"""Validate a real local model's Chat Completions SSE counters through Headroom."""
import ctypes
from hashlib import file_digest
import http.client
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import runpy
import socket
import subprocess
import sys
import time
import uuid
from zipfile import ZipFile


PROGRAM = Path(__file__).resolve()
ROOT = PROGRAM.parents[2]
PROOFS = ROOT / "work/proofs/optimization/chat-real-inference"
CAPTURE = ROOT / "tests/runtime/chat_usage_capture.py"
PATH = "/v1/chat/completions"
MODEL = "smollm2-135m-local-chat"
ARCHIVE = "llama-b12359-bin-win-cpu-x64.zip"
WEIGHTS = "SmolLM2-135M-Instruct-Q4_K_M.gguf"
PINS = {
    ARCHIVE: "837c1655e87df3b66989f4be33a792a1399c785bf259038dc75c900096683e69",
    WEIGHTS: "2e8040ceae7815abe0dcb3540b9995eaa1fa0d2ca9e797d0a635ae4433c68c2d",
}
BODY = {
    "model": MODEL, "stream": True, "stream_options": {"include_usage": True},
    "messages": [{"role": "user", "content": "Return a JSON object with answer equal to 2 plus 2."}],
    "max_completion_tokens": 24, "temperature": 0,
    "response_format": {"type": "json_schema", "json_schema": {"name": "answer", "strict": True,
        "schema": {"type": "object", "properties": {"answer": {"type": "integer", "enum": [4]}},
                   "required": ["answer"], "additionalProperties": False}}},
}


def inspect_stream(raw):
    lines = [line[5:].strip() for line in raw.splitlines() if line.startswith(b"data:")]
    assert lines and lines[-1] == b"[DONE]", "stream_terminal_missing"
    events = [json.loads(line) for line in lines[:-1]]
    assert events and all(row.get("object") == "chat.completion.chunk" for row in events), "changed_stream_shape"
    identities = {row.get("id") for row in events}
    assert len(identities) == 1 and all(isinstance(value, str) and value for value in identities), "changed_stream_identity"
    choices = [choice for event in events for choice in event["choices"]]
    assert [choice["finish_reason"] for choice in choices if choice.get("finish_reason")] == ["stop"], "answer_not_completed"
    content = "".join((choice.get("delta") or {}).get("content") or "" for choice in choices)
    answer = json.loads(content)
    assert answer == {"answer": 4} and type(answer["answer"]) is int, "answer_mismatch"
    usage_frames = [row for row in events if row.get("usage") is not None]
    assert usage_frames == [events[-1]] and not events[-1]["choices"], "usage_terminal_mismatch"
    usage = events[-1]["usage"]
    prompt, completion = usage.get("prompt_tokens"), usage.get("completion_tokens")
    assert type(prompt) is int and prompt > 0 and type(completion) is int and completion > 0, "invalid_token_counts"
    assert type(usage.get("total_tokens")) is int and usage["total_tokens"] == prompt + completion, "invalid_token_total"
    details = usage.get("prompt_tokens_details")
    assert details is None or isinstance(details, dict), "invalid_cache_fields"
    details = details or {}
    cached, written = details.get("cached_tokens"), details.get("cache_write_tokens")
    assert all(value is None or type(value) is int and 0 <= value <= prompt for value in (cached, written)), "invalid_cache_fields"
    assert (cached or 0) + (written or 0) <= prompt, "invalid_cache_total"
    return {"input_tokens": prompt, "cache_read_input_tokens": cached,
            "cache_creation_input_tokens": written, "output_tokens": completion}


def install_guard(port, counter):
    import httpx

    target = f"http://127.0.0.1:{port}{PATH}"
    send = httpx.AsyncClient.send
    allowed = False

    async def bounded(client, request, **options):
        nonlocal allowed
        if not allowed and request.method == "POST" and str(request.url) == target and len(request.content) <= 16384:
            body = json.loads(request.content)
            if isinstance(body, dict) and all(body.get(name) == value for name, value in BODY.items()):
                allowed = True
                with Path(counter).open("xb") as stream:
                    stream.write(b"1\n")
                return await send(client, request, **{**options, "follow_redirects": False})
        return httpx.Response(403, request=request, json={"error": {"message": "Protocol proof scope"}})

    httpx.AsyncClient.send = bounded


def ready(port, process, supervisor, iphlpapi):
    end = time.monotonic() + 25
    while time.monotonic() < end:
        assert process.poll() is None, "model_server_exited"
        listeners = supervisor.listener_pids(port, iphlpapi)
        assert not listeners or listeners == {"127.0.0.1": process.pid}, "model_listener_owner_changed"
        if listeners:
            conn = http.client.HTTPConnection("127.0.0.1", port, timeout=1)
            try:
                conn.request("GET", "/health")
                if conn.getresponse().status == 200:
                    return
            except OSError:
                pass
            finally:
                conn.close()
        time.sleep(0.1)
    raise AssertionError("model_server_not_ready")


def main():
    assert os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_workspace"
    chat = runpy.run_path(str(CAPTURE))
    capture, replay, probe = (chat[name] for name in ("capture", "base", "probe"))
    assert PATH == chat["PATH"], "chat_route"
    plan_path = capture["ccr"]["PLAN"]
    plan = json.loads(plan_path.read_bytes())
    runtime = ROOT / "runtime"
    controller = replay.load_module("real_chat_controller", runtime / "desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(runtime / "manifest.json")
    assert replay.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "python_pin_changed"
    runner = payload / manifest["engine"]["runner"]
    supervisor = replay.load_module("real_chat_supervisor", runner)
    assets = PROOFS / "assets"
    assert assets.resolve(strict=True) == assets, "asset_scope"
    for name, wanted in PINS.items():
        assert replay.sha(assets / name) == wanted, "asset_pin_changed"
    binary = assets / "llama-b12359-bin-win-cpu-x64/llama-server.exe"
    binaries = [binary, *sorted(binary.parent.glob("*.dll"))]
    with ZipFile(assets / ARCHIVE) as archive:
        archived = {PurePosixPath(name).name: name for name in archive.namelist()
                    if name.endswith((".dll", "/llama-server.exe")) or name == "llama-server.exe"}
        assert archived.keys() == {path.name for path in binaries}, "server_binary_set_changed"
        for path in binaries:
            assert path.resolve(strict=True) == path, "server_binary_path_changed"
            with archive.open(archived[path.name]) as source:
                assert file_digest(source, "sha256").hexdigest() == replay.sha(path), "server_binary_changed"
    weights = assets / WEIGHTS
    run = PROOFS / "runs" / uuid.uuid4().hex
    assert run.resolve().is_relative_to(PROOFS), "proof_scope"
    run.mkdir(parents=True)
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    os.environ["LITELLM_LOCAL_MODEL_COST_MAP"] = "True"
    registration = {
        "os": platform.platform(), "engine": manifest["engine"]["version"], "provider": "llama.cpp b12359",
        "model": MODEL, "request_limit": 1, "max_output_tokens": BODY["max_completion_tokens"],
        "scope": "actual local CPU inference, synthetic prompt and constrained JSON answer",
    }
    replay.write_new(run / "registration.json", registration)
    staged = replay.prepare_fixture_runtime(run, payload)
    helper = run / "fixture-runtime/desktop_helper.py"
    source = helper.read_bytes()
    anchor = b'"from headroom.cli import main; main()"'
    assert source.count(anchor) == 1, "engine_bootstrap_changed"
    counter = run / "upstream-attempts.txt"
    guard = f"import runpy; runpy.run_path({str(PROGRAM)!r})['install_guard']({port!r}, {str(counter)!r}); "
    helper.write_bytes(source.replace(anchor, repr(guard).encode("ascii") + b" " + anchor, 1))
    source_map = json.loads((runtime / "optimization-sources.json").read_bytes())
    inputs = [PROGRAM, CAPTURE, Path(sys.executable), payload / "python313.dll", runner, weights, assets / ARCHIVE, plan_path,
              run / "registration.json", runtime / "manifest.json", payload / "runtime-manifest.json",
              *binaries, *staged, *[runtime / path.name for path in staged],
              ROOT / "tests/runtime/anthropic_usage_capture.py",
              *[ROOT / "bench/optimization" / name for name in ("replay.py", "fixtures.py", "surfaces.py", "anthropic_usage.py", "ccr_policy.py")],
              *[payload / "Lib/site-packages/headroom" / name for name in source_map]]
    before = replay.fingerprints(inputs)
    provider_job = supervisor.KillOnCloseJob()
    provider, engine, failure, client, record = None, None, None, {}, None
    iphlpapi = ctypes.WinDLL("iphlpapi")
    try:
        cwd = run / "model-server"
        cwd.mkdir()
        env = supervisor.safe_environment(cwd, cwd, cwd, port)
        command = [str(binary), "--model", str(weights), "--alias", MODEL, "--host", "127.0.0.1", "--port", str(port),
                   "--no-webui", "--ctx-size", "512", "--parallel", "1", "--n-gpu-layers", "0", "--threads", "2",
                   "--n-predict", str(registration["max_output_tokens"]), "--jinja"]
        provider = subprocess.Popen(command, cwd=cwd, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                    stderr=subprocess.DEVNULL, creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            provider_job.assign(provider.pid)
        except OSError:
            provider.kill()
            raise
        ready(port, provider, supervisor, iphlpapi)
        engine = probe["UsageEngine"](controller, supervisor, payload, run / "engine", port, plan)
        engine.start()
        assert not capture["records"](engine) and not probe["request_logs"](engine), "engine_not_empty"
        headers = {"Content-Type": "application/json", "Authorization": "Bearer " + replay.SENTINEL}
        status, media, raw = probe["exchange"](engine.port, "POST", PATH, headers, replay.encoded(BODY))
        client = {"status": status, "sse": media.startswith("text/event-stream")}
        assert status == 200 and client["sse"], "inference_route_failed"
        stream_usage = inspect_stream(raw)
        client.update(answer_exact=True, usage=stream_usage)
        end = time.monotonic() + 3
        while True:
            rows = capture["records"](engine)
            logged = set(probe["request_logs"](engine))
            if (rows and rows.keys() == logged) or time.monotonic() >= end:
                break
            time.sleep(0.05)
        assert len(rows) == 1 and rows.keys() == logged, "inference_usage_join"
        record = chat["check"](next(iter(rows.values())), [{"status": 200, "usage": stream_usage}], status, 1)
        assert counter.read_bytes() == b"1\n", "unexpected_upstream_attempt_count"
    except Exception as error:
        failure = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)) else type(error).__name__
    finally:
        try:
            if engine is not None:
                engine.close()
        except Exception:
            failure = failure or "engine_shutdown_failed"
        finally:
            provider_job.close()
            if provider is not None:
                provider.wait(timeout=10)
    provider_closed = provider is not None and provider.poll() is not None and not supervisor.listener_pids(port, iphlpapi)
    after = replay.fingerprints(inputs)
    passed = failure is None and before == after and provider_closed and engine.stopped["owned_port_closed"]
    result = {**registration, "passed": passed, "failure": failure, "client": client, "usage_record": record,
              "loopback_model_requests": int(counter.is_file()), "remote_model_requests": 0,
              "provider_closed": provider_closed, "engine_stop": engine.stopped if engine else None, "before": before, "after": after}
    proof = run / "result.json"
    replay.write_new(proof, result)
    print(json.dumps({"passed": passed, "failure": failure, "usage": record and record["usage"],
                      "proof": proof.relative_to(ROOT).as_posix()}))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
