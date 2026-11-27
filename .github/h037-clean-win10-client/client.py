from hashlib import sha256
import json
import os
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from openai_fixture import LocalOpenAICompatFixture, OUTPUT_MARKER


MODEL = "headroom-opencode-fixture"
SENTINEL = "headroom-local-fixture-only"
PROMPT = "Return the synthetic fixture marker. Do not use tools."
EXE_SHA256 = "2359208e2f81539fd16eb755a4d8a747e87c6fd5cd59a49bb552f8a610d33eb9"


def publish(path, value):
    temporary = path.with_suffix(".partial")
    temporary.write_text(json.dumps(value), encoding="utf-8")
    temporary.replace(path)


def prepare(work, profile):
    client = work / "opencode.exe"
    with client.open("rb") as source:
        assert sha256(source.read()).hexdigest() == EXE_SHA256, "cli_digest_mismatch"
    folder = work / "config/opencode"
    folder.mkdir(parents=True)
    config = {
        "model": {"providerID": "lab", "model": "slot"},
        "update": "disable", "share": "disabled", "plugins": [],
        "warming": False, "snapshots": False, "compaction": {"auto": False},
        "agents": {"title": {"disabled": True}},
        "permissions": [
            {"action": "*", "resource": "*", "effect": "deny"},
            {"action": "headroom_retrieve", "resource": "*", "effect": "allow"},
        ],
        "policies": [
            *({"action": action, "resource": "*", "effect": "deny"} for action in
              ("provider.use", "tool.use", "integration.connect", "integration.use",
               "mcp.use", "skill.use", "plugin.use", "websearch.use")),
            {"action": "provider.use", "resource": "lab", "effect": "allow"},
            {"action": "integration.use", "resource": "lab", "effect": "allow"},
            {"action": "mcp.use", "resource": "headroom", "effect": "allow"},
            {"action": "tool.use", "resource": "headroom_retrieve:*", "effect": "allow"},
        ],
        "providers": {"lab": {
            "package": "@opencode/ai/providers/openai-compatible",
            "env": ["OPENAI_API_KEY"],
            "settings": {"baseURL": "https://api.openai.com/v1", "apiKey": "{env:OPENAI_API_KEY}"},
            "models": {"slot": {"modelID": MODEL, "name": "Synthetic test",
                                "capabilities": {"tools": False, "reasoning": {"supported": False}}}},
        }},
    }
    original = ("// Example OpenCode settings\n" + json.dumps(config, indent=2) + "\n").encode()
    (folder / "opencode.jsonc").write_bytes(original)
    fixture = LocalOpenAICompatFixture(SENTINEL)
    fixture.start()
    try:
        settings = profile / "runtime/workspace/settings.json"
        settings.parent.mkdir(parents=True)
        publish(settings, {"openai_base_url": f"http://127.0.0.1:{fixture.port}/v1"})
        publish(work / "fixture.json", {"port": fixture.port, "original_sha256": sha256(original).hexdigest(),
                                       "opencode_sha256": EXE_SHA256})
        if fixture.state.terminal_sent.wait(timeout=410):
            rows = [row for row in fixture.state.snapshot() if row["method"] == "POST"]
            publish(work / "upstream.json", {"posts": len(rows), "correct_request": len(rows) == 1 and
                    rows[0]["path"] == "/v1/chat/completions" and rows[0]["model"] == MODEL and rows[0]["stream"]
                    and rows[0]["authorization_matches_sentinel"], "stream_ended": not fixture.state.handler_error})
        fixture.thread.join(timeout=45)
    finally:
        fixture.close()


def request(work):
    executable = work / "opencode.exe"
    env = {key: os.environ[key] for key in ("SystemRoot", "WINDIR", "SystemDrive", "COMSPEC") if key in os.environ}
    env["PATH"] = os.pathsep.join((str(work), str(Path(env["SystemRoot"]) / "System32")))
    for key, name in {
        "HOME": "home", "USERPROFILE": "home", "APPDATA": "roam", "LOCALAPPDATA": "local", "TEMP": "temp", "TMP": "temp",
        "ProgramData": "data", "XDG_CONFIG_HOME": "config", "XDG_DATA_HOME": "xdgdata",
        "XDG_CACHE_HOME": "cache", "XDG_STATE_HOME": "state",
    }.items():
        folder = work / name
        folder.mkdir(exist_ok=True)
        env[key] = str(folder)
    home = Path(env["HOME"])
    env.update(HOMEDRIVE=home.drive, HOMEPATH=str(home)[len(home.drive):],
               OPENCODE_DISABLE_MODELS_FETCH="1", OPENCODE_DISABLE_AUTOUPDATE="1",
               OPENCODE_DISABLE_PROJECT_CONFIG="1", OPENAI_API_KEY=SENTINEL,
               GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=str(home / ".gitconfig"))
    sink = json.loads((work / "fixture.json").read_text())["port"]
    env.update(HTTP_PROXY=f"http://127.0.0.1:{sink}", HTTPS_PROXY=f"http://127.0.0.1:{sink}", NO_PROXY="127.0.0.1,localhost")
    workspace = work / "workspace"
    workspace.mkdir(exist_ok=True)
    facts = {}
    try:
        result = subprocess.run([str(executable), "run", "--standalone", "--format", "json", "--model", "lab/slot",
                                 "--title", "PennyPinch clean installer check", PROMPT],
                                cwd=workspace, env=env, stdin=subprocess.DEVNULL, capture_output=True, timeout=95,
                                creationflags=subprocess.CREATE_NO_WINDOW)
        facts.update(exit=result.returncode, stderr_bytes=len(result.stderr))
        events = [json.loads(line) for line in result.stdout.splitlines() if line.strip()]
        kinds = [row.get("type") for row in events]
        answer = "".join(row.get("part", {}).get("text", "") for row in events if row.get("type") == "text")
        facts.update(steps=kinds.count("step_finish"), marker_exact=answer == OUTPUT_MARKER,
                     unexpected_event=bool(set(kinds).intersection(("error", "retry", "tool_use", "background_wait"))))
    except (subprocess.TimeoutExpired, ValueError, OSError) as error:
        facts["error"] = type(error).__name__
    publish(work / "cli.json", facts)


action, directory, *rest = sys.argv[1:]
work = Path(directory).resolve(strict=True)
if action == "serve":
    prepare(work, Path(rest[0]))
elif action == "request":
    request(work)
else:
    raise ValueError("unexpected_probe_mode")
