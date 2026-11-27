from hashlib import file_digest
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import threading
import time
import tomllib
import uuid


CODEX_SHA256 = "9d563ee4e0441ff9d59d612013e0460ddf8e5709735c2d9f6b98b7ef8ca4eb42"
DEVICE_URL = "https://auth.openai.com/codex/device"
ANSI = re.compile(r"\x1b\[[\d;]*[A-Za-z]")
MAX_OUTPUT = 512 * 1024


def atomic_json(path, payload):
    temp = path.with_suffix(".tmp")
    temp.write_text(json.dumps(payload), encoding="utf-8")
    temp.replace(path)


def call(argv, timeout, *, data=None):
    return subprocess.run(argv, input=data, cwd=workdir, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          timeout=timeout, creationflags=subprocess.CREATE_NO_WINDOW)


def login():
    code_seen = threading.Event()
    fail = []
    child = subprocess.Popen([str(client), "login", "--device-auth"], cwd=workdir, stdout=subprocess.PIPE,
                             stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL, creationflags=subprocess.CREATE_NO_WINDOW)

    def scan():
        saw_url = False
        expects_code = False
        length = 0
        try:
            for part in iter(child.stdout.readline, b""):
                length += len(part)
                if length > MAX_OUTPUT:
                    raise ValueError
                line = ANSI.sub("", part.decode("utf-8", "replace")).strip()
                if line == DEVICE_URL:
                    saw_url = True
                elif saw_url and line.startswith("2. Enter this one-time code"):
                    expects_code = True
                elif expects_code and line:
                    expects_code = False
                    if not re.fullmatch("[A-Z0-9][A-Z0-9-]{4,27}", line) or code_seen.is_set():
                        raise ValueError
                    encrypted = call([str(windows_ps), "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
                                      str(recipe / "encrypt-challenge.ps1")], 15, data=line.encode("ascii"))
                    cipher = encrypted.stdout.decode("ascii", "strict")
                    if encrypted.returncode or not re.fullmatch("[A-Za-z0-9+/]{512}", cipher):
                        raise ValueError
                    atomic_json(work / "device.json", {"challenge_id": public["challenge_id"], "ciphertext": cipher})
                    code_seen.set()
        except (ValueError, TypeError, OSError, subprocess.TimeoutExpired):
            fail.append("challenge_capture_failed")

    thread = threading.Thread(target=scan, daemon=True)
    thread.start()
    try:
        deadline = time.monotonic() + 850
        while child.poll() is None and not fail and time.monotonic() < deadline:
            time.sleep(.25)
        if child.poll() is None:
            child.kill()
        exit_code = child.wait(timeout=10)
        thread.join(10)
        status = call([str(client), "login", "status"], 20)
        text = (status.stdout + status.stderr).decode("utf-8", "replace").casefold()
        active = "logged in using chatgpt" in text and status.returncode == 0
        if fail or exit_code or not code_seen.is_set() or not active:
            raise RuntimeError(fail[0] if fail else "device_auth_not_completed")
    finally:
        if child.poll() is None:
            child.kill()
            child.wait(timeout=10)
        child.stdout.close()


def run_live():
    if any(name in os.environ for name in ("OPENAI_API_KEY", "CODEX_API_KEY", "OPENAI_BASE_URL", "CHATGPT_BASE_URL")):
        raise RuntimeError("unexpected_provider_override")
    with client.open("rb") as source:
        assert file_digest(source, "sha256").hexdigest() == CODEX_SHA256, "cli_digest_mismatch"
    original = config.read_bytes()
    expected = {"cli_auth_credentials_store": "file", "web_search": "disabled"}
    current = tomllib.loads(original.decode("utf-8"))
    expected["openai_base_url"] = f"http://127.0.0.1:{port}/v1"
    assert current == expected, "unexpected_saved_cli_configuration"
    assert not auth.exists() and (not ledger.exists() or ledger.stat().st_size == 0), "profile_is_not_fresh"
    login()
    atomic_json(work / "login-complete.json", {"auth": "chatgpt"})
    marker = "PENNYPINCH_CLEAN_WIN10_" + uuid.uuid4().hex[:12].upper()
    result = call([str(client), "--no-daemon", "-a", "never", "exec", "--ephemeral", "--json", "--color", "never",
                   "--sandbox", "read-only", "--skip-git-repo-check", "-C", str(workdir),
                   f"Reply with exactly {marker} and no other text. Use no tools or subagents."], 110)
    assert 0 < len(result.stdout) <= MAX_OUTPUT, "cli_output_invalid_size"
    events = [json.loads(line) for line in result.stdout.splitlines() if line]
    terminal = [row for row in events if row.get("type") == "turn.completed"]
    items = [row["item"] for row in events if row.get("type") == "item.completed"]
    answers = [item.get("text", "").strip() for item in items if item.get("type") == "agent_message"]
    assert result.returncode == 0 and len(terminal) == 1 and answers == [marker], "live_turn_not_complete"
    assert not any(row.get("type") in ("error", "turn.failed") for row in events), "live_turn_reported_error"
    assert all(item.get("type") in ("agent_message", "reasoning") for item in items), "unexpected_tool_or_item"
    usage = terminal[0]["usage"]
    assert type(usage["input_tokens"]) is int and usage["input_tokens"] > 0 and usage["output_tokens"] > 0, "provider_usage_unavailable"
    end = time.monotonic() + 12
    matched = []
    while time.monotonic() < end:
        if ledger.is_file():
            raw = ledger.read_bytes()
            assert len(raw) <= MAX_OUTPUT, "proxy_receipt_invalid_size"
            rows = [json.loads(line) for line in raw.splitlines()]
            matched = [row for row in rows if row.get("source") == "proxy" and row.get("client") == "codex"
                       and row.get("after") == usage["input_tokens"]]
            if matched:
                break
        time.sleep(.25)
    assert len(matched) == 1 and config.read_bytes() == original, "live_turn_not_correlated_to_engine"
    model = matched[0]["model"]
    assert re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,79}", model), "unknown_reported_model"
    return {"live_turns": 1, "turn_completed": True, "marker_exact": True, "matched_proxy_rows": 1,
            "reported_model": model, "usage_inputs": usage["input_tokens"], "usage_outputs": usage["output_tokens"]}


work = Path(sys.argv[1]).resolve(strict=True)
profile = Path(sys.argv[2]).resolve(strict=True)
port = int(sys.argv[3])
recipe = Path(__file__).resolve().parent
client = work / "codex.exe"
config = work / "profile/config.toml"
auth = config.with_name("auth.json")
ledger = profile / "runtime/workspace/savings_events.jsonl"
workdir = work / "empty-workspace"
workdir.mkdir()
windows_ps = Path(os.environ["SystemRoot"]) / "System32/WindowsPowerShell/v1.0/powershell.exe"
public = json.loads((recipe / "device-public.json").read_bytes())
assert Path(os.environ["CODEX_HOME"]).resolve() == config.parent and not auth.exists()
report = {"status": "fail"}
try:
    answers = run_live()
    report = {"status": "pass", **answers}
except (AssertionError, KeyError, ValueError, OSError, RuntimeError, subprocess.TimeoutExpired) as error:
    label = str(error) if isinstance(error, (AssertionError, RuntimeError)) else ""
    report["error_code"] = label if re.fullmatch("[a-z_]{1,60}", label) else "live_client_probe_failed"
finally:
    if auth.is_file():
        try:
            call([str(client), "logout"], 20)
        except (OSError, subprocess.TimeoutExpired):
            pass
        auth.unlink(missing_ok=True)
    report["temporary_auth_removed"] = not auth.exists()
    if not report["temporary_auth_removed"]:
        report["status"] = "fail"
    atomic_json(work / "live-result.json", report)
