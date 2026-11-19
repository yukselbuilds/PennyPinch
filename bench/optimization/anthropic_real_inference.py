"""Match real local Anthropic Messages inference usage to an owned Headroom record."""
import argparse
import ctypes
from hashlib import file_digest
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
LOCAL_CHAT = PROGRAM.with_name("chat_real_inference.py")
CAPTURE = ROOT / "tests/runtime/anthropic_usage_capture.py"
PATH = "/v1/messages"
MODEL = "smollm2-135m-local-messages"
LIMIT = 24
EXPECTED = {"answer": 4}
SYNTHETIC = "headroom-local-fixture-only"


def bodies(response_format):
    common = {"model": MODEL, "max_tokens": LIMIT, "temperature": 0,
              "extra_params": {"response_format": response_format}}
    return {
        "json": {**common, "stream": False, "messages": [
            {"role": "user", "content": "Return a JSON object with answer equal to 2 plus 2."}]},
        "sse": {**common, "stream": True, "messages": [
            {"role": "user", "content": "Return a JSON object with answer equal to 9 minus 5."}]},
    }


def install_guard(port, run, expected):
    import httpx

    run = Path(run)
    target = f"http://127.0.0.1:{port}{PATH}"
    send = httpx.AsyncClient.send

    async def bounded(client, request, **options):
        if request.method == "POST" and str(request.url) == target and len(request.content) <= 16384:
            try:
                body = json.loads(request.content)
            except (ValueError, RecursionError):
                body = None
            if isinstance(body, dict):
                case = "sse" if body.get("stream") is True else "json"
                changed = [field for field, value in expected[case].items() if body.get(field) != value]
                if body.keys() - expected[case].keys():
                    changed.append("extra_fields")
                if changed:
                    (run / f"{case}-changed-fields.json").write_text(json.dumps(changed), encoding="ascii")
                elif (request.headers.get("x-api-key") == SYNTHETIC
                      and request.headers.get("anthropic-version") == "2023-06-01"):
                    marker = run / f"{case}-sent.txt"
                    try:
                        with marker.open("x", encoding="ascii") as stream:
                            stream.write("1\n")
                    except FileExistsError:
                        pass
                    else:
                        return await send(client, request, **{**options, "follow_redirects": False})
        return httpx.Response(403, request=request, json={"error": {"message": "Model proof scope"}})

    httpx.AsyncClient.send = bounded


def inspect_reply(raw, stream, usage_fields):
    if stream:
        data = [line[5:].strip() for line in raw.splitlines() if line.startswith(b"data:")]
        events = [json.loads(item) for item in data]
        types = [part["type"] for part in events]
        assert types and types[0] == "message_start" and types[-1] == "message_stop", "message_terminal_missing"
        assert types.count("message_start") == types.count("message_stop") == 1 and "error" not in types, "message_scope_changed"
        message = events[0]["message"]
        usage = dict(message["usage"])
        deltas = [part for part in events if part["type"] == "message_delta"]
        assert deltas, "message_delta_missing"
        for delta in deltas:
            usage.update(delta.get("usage") or {})
        stop = deltas[-1]["delta"]["stop_reason"]
        text = "".join(part["delta"]["text"] for part in events if part["type"] == "content_block_delta"
                       and part["delta"].get("type") == "text_delta")
    else:
        message = json.loads(raw)
        usage, stop = message["usage"], message["stop_reason"]
        text = "".join(block["text"] for block in message["content"] if block["type"] == "text")
    assert message["type"] == "message" and message["role"] == "assistant" and message["model"] == MODEL, "message_identity"
    assert stop in ("end_turn", "stop_sequence", "max_tokens") and text.strip(), "message_incomplete"
    counters = usage_fields(usage)
    assert (counters["input_tokens"] is not None and counters["input_tokens"] > 0
            and counters["output_tokens"] is not None and 0 < counters["output_tokens"] <= LIMIT), "usage_range"
    try:
        answer = json.loads(text)
        exact = answer == EXPECTED and type(answer["answer"]) is int
    except (TypeError, ValueError, RecursionError):
        exact = False
    return {"usage": counters, "finish": stop, "answer_exact": exact}


def checked_assets(llama, replay):
    root = llama["PROOFS"] / "assets"
    assert root.resolve(strict=True) == root, "asset_scope"
    archive = root / llama["ARCHIVE"]
    weights = root / llama["WEIGHTS"]
    for path in (archive, weights):
        assert replay.sha(path) == llama["PINS"][path.name], "asset_pin_changed"
    binary = root / "llama-b12359-bin-win-cpu-x64/llama-server.exe"
    installed = [binary, *sorted(binary.parent.glob("*.dll"))]
    with ZipFile(archive) as package:
        contents = {PurePosixPath(name).name: name for name in package.namelist()
                    if name.endswith((".dll", "/llama-server.exe")) or name == "llama-server.exe"}
        assert contents.keys() == {path.name for path in installed}, "binary_set_changed"
        for path in installed:
            assert path.resolve(strict=True) == path, "binary_path_changed"
            with package.open(contents[path.name]) as source:
                assert file_digest(source, "sha256").hexdigest() == replay.sha(path), "binary_pin_changed"
    return binary, weights, [*installed, archive, weights]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true", required=True)
    parser.parse_args()
    assert os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_workspace"
    llama = runpy.run_path(str(LOCAL_CHAT))
    capture = runpy.run_path(str(CAPTURE))
    probe, replay = capture["probe"], capture["replay"]
    plan_path = capture["ccr"]["PLAN"]
    plan = json.loads(plan_path.read_bytes())
    runtime = ROOT / "runtime"
    controller = replay.load_module("real_messages_controller", runtime / "desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(runtime / "manifest.json")
    assert replay.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "python_pin_changed"
    runner = payload / manifest["engine"]["runner"]
    supervisor = replay.load_module("real_messages_supervisor", runner)
    binary, weights, assets = checked_assets(llama, replay)
    requests = bodies(llama["BODY"]["response_format"])
    run = ROOT / "work/proofs/optimization/anthropic-real-inference" / uuid.uuid4().hex
    assert run.resolve().is_relative_to(ROOT), "proof_scope"
    run.mkdir(parents=True)
    registration = {"os": platform.platform(), "engine": manifest["engine"]["version"], "provider": "llama.cpp b12359",
                    "model": MODEL, "transports": list(requests), "request_limit": 2, "output_token_limit": LIMIT,
                    "scope": "loopback Anthropic Messages JSON and SSE with synthetic input and real CPU inference"}
    replay.write_new(run / "registration.json", registration)
    staged = replay.prepare_fixture_runtime(run, payload)
    helper = run / "fixture-runtime/desktop_helper.py"
    source = helper.read_bytes()
    anchor = b'"from headroom.cli import main; main()"'
    assert source.count(anchor) == 1, "engine_bootstrap_changed"
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    guard = f"import runpy; runpy.run_path({str(PROGRAM)!r})['install_guard']({port!r}, {str(run)!r}, {requests!r}); "
    helper.write_bytes(source.replace(anchor, repr(guard).encode("ascii") + b" " + anchor, 1))
    patched = json.loads((runtime / "optimization-sources.json").read_bytes())
    inputs = [PROGRAM, LOCAL_CHAT, CAPTURE, Path(sys.executable), payload / "python313.dll", runner, plan_path,
              payload / "runtime-manifest.json", run / "registration.json", *assets, *staged, *[runtime / path.name for path in staged],
              *[PROGRAM.with_name(name) for name in ("replay.py", "fixtures.py", "surfaces.py", "anthropic_usage.py", "ccr_policy.py")],
              *[payload / "Lib/site-packages/headroom" / name for name in patched]]
    before = replay.fingerprints(inputs)
    job = supervisor.KillOnCloseJob()
    provider, engine, failure = None, None, None
    results, seen = {}, set()
    iphlpapi = ctypes.WinDLL("iphlpapi")
    try:
        assert replay.SENTINEL == SYNTHETIC, "synthetic_auth_changed"
        cwd = run / "model-server"
        cwd.mkdir()
        env = supervisor.safe_environment(cwd, cwd, cwd, port)
        command = [str(binary), "--model", str(weights), "--alias", MODEL, "--host", "127.0.0.1", "--port", str(port),
                   "--no-webui", "--ctx-size", "512", "--parallel", "1", "--n-gpu-layers", "0", "--threads", "2",
                   "--n-predict", str(LIMIT), "--jinja", "--api-key", SYNTHETIC]
        provider = subprocess.Popen(command, cwd=cwd, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                                    stderr=subprocess.DEVNULL, creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            job.assign(provider.pid)
        except OSError:
            provider.kill()
            raise
        llama["ready"](port, provider, supervisor, iphlpapi)
        engine = probe["UsageEngine"](controller, supervisor, payload, run / "engine", port, plan)
        engine.start()
        assert not capture["records"](engine) and not probe["request_logs"](engine), "engine_not_empty"
        headers = {"Content-Type": "application/json", "x-api-key": SYNTHETIC, "anthropic-version": "2023-06-01"}
        for case, body in requests.items():
            status, media, raw = probe["exchange"](engine.port, "POST", PATH, headers, replay.encoded(body))
            row = results[case] = {"status": status}
            assert status == 200 and media.startswith("text/event-stream" if body["stream"] else "application/json"), "inference_route_failed"
            row.update(inspect_reply(raw, body["stream"], probe["usage_fields"]))
            deadline = time.monotonic() + 3
            while True:
                logged = set(probe["request_logs"](engine)) - seen
                matched = {key: record for key, record in capture["records"](engine).items() if key not in seen}
                if (logged and matched.keys() == logged) or time.monotonic() >= deadline:
                    break
                time.sleep(0.05)
            assert len(matched) == 1 and matched.keys() == logged, "inference_usage_join"
            identity, record = next(iter(matched.items()))
            row["usage_record"] = capture["check_record"](record, [{"status": status, "usage": row["usage"]}], status)
            seen.add(identity)
    except Exception as error:
        failure = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)) else type(error).__name__
    finally:
        try:
            if engine is not None:
                engine.close()
        except Exception:
            failure = failure or "engine_shutdown_failed"
        finally:
            job.close()
            if provider is not None:
                provider.wait(timeout=10)
    after = replay.fingerprints(inputs)
    sent = {case: int((run / f"{case}-sent.txt").is_file()) for case in requests}
    provider_closed = provider is not None and provider.poll() is not None and not supervisor.listener_pids(port, iphlpapi)
    passed = failure is None and before == after and provider_closed and sum(sent.values()) == len(seen) == 2
    result = {**registration, "passed": passed, "failure": failure, "results": results, "upstream": sent,
              "provider_closed": provider_closed, "engine_stop": engine.stopped if engine is not None else None,
              "matched_inputs": len(before) if before == after else 0}
    proof = run / "result.json"
    replay.write_new(proof, {**result, "before": before, "after": after})
    print(json.dumps({**result, "proof": proof.relative_to(ROOT).as_posix()}))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
