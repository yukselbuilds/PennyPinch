"""Verify Headroom's usage counters with two anonymous AI Horde requests."""
import argparse
import http.client
import json
import os
from pathlib import Path
import platform
import re
import runpy
import sys
import time
import uuid


PROGRAM = Path(__file__).resolve()
ROOT = PROGRAM.parents[2]
CAPTURE = ROOT / "tests/runtime/chat_usage_capture.py"
BASE = "https://oai.aihorde.net"
PATH = "/v1/chat/completions"
MODEL = "koboldcpp/Llama-3.2-1B-Instruct"
ANONYMOUS = "Bearer 0000000000"
TIMEOUT = 75
MAX_COMPLETION_TOKENS = 32
EXPECTED = {"json": "7", "sse": "4"}
PROVIDER_ERRORS = ("UnknownChatModel", "model_not_found", "no_worker_supports_features", "no_worker_supports_limit",
                   "no_eligible_worker", "queue_unavailable", "UnsupportedChatParameter", "FrontAtCapacity")


def install_guard(directory, bodies):
    import httpx

    send = httpx.AsyncClient.send
    directory = Path(directory)

    async def bounded(client, request, **options):
        body = None
        if (request.method == "POST" and str(request.url) == BASE + PATH and len(request.content) <= 4096
                and request.headers.get("authorization") == ANONYMOUS
                and not any(name in request.headers for name in ("cookie", "proxy-authorization", "x-api-key"))):
            try:
                body = json.loads(request.content)
            except (ValueError, RecursionError):
                pass
        for mode, expected in bodies.items():
            if body != expected:
                continue
            counter = directory / f"{mode}-sent.txt"
            try:
                with counter.open("x", encoding="ascii") as stream:
                    stream.write("1\n")
            except FileExistsError:
                break
            request.headers = httpx.Headers({"Host": request.url.host, "Authorization": ANONYMOUS,
                "Content-Type": "application/json", "Content-Length": str(len(request.content)),
                "Accept": "text/event-stream" if mode == "sse" else "application/json",
                "User-Agent": "Headroom-Protocol-Test", "X-Title": "Headroom Protocol Test"})
            response = await send(client, request, **{**options, "follow_redirects": False})
            (directory / f"{mode}-status.txt").write_text(str(response.status_code), encoding="ascii")
            return response
        return httpx.Response(403, request=request, json={"error": {"message": "Benchmark request outside scope"}})

    httpx.AsyncClient.send = bounded


def request(port, body):
    client = http.client.HTTPConnection("127.0.0.1", port, timeout=TIMEOUT)
    deadline = time.monotonic() + TIMEOUT
    try:
        client.request("POST", PATH, json.dumps(body).encode(), {"Content-Type": "application/json",
            "Authorization": ANONYMOUS, "X-Title": "Headroom Protocol Test"})
        reply = client.getresponse()
        content = bytearray()
        while part := reply.read1(8192):
            assert time.monotonic() < deadline and len(content) + len(part) <= 128 * 1024, "response_bound"
            content.extend(part)
        return reply.status, reply.getheader("Content-Type", ""), bytes(content)
    finally:
        client.close()


def inspect_reply(mode, raw, diagnostics):
    if mode == "sse":
        lines = [line[5:].strip() for line in raw.splitlines() if line.startswith(b"data:")]
        assert len(lines) > 1 and lines[-1] == b"[DONE]", "stream_terminal_missing"
        frames = [json.loads(line) for line in lines[:-1]]
        assert all(frame.get("object") == "chat.completion.chunk" for frame in frames), "sse_reply_shape"
        assert [frame for frame in frames if frame.get("usage") is not None] == [frames[-1]] and not frames[-1]["choices"], "stream_usage_terminal"
        choices = [choice for frame in frames for choice in frame["choices"]]
        text = "".join(choice["delta"].get("content") or "" for choice in choices)
    else:
        reply = json.loads(raw)
        assert reply.get("object") == "chat.completion", "json_reply_shape"
        frames, choices = [reply], reply["choices"]
        choice, = choices
        text = choice["message"]["content"] or ""
    first_id = frames[0].get("id")
    assert isinstance(first_id, str) and first_id and all(frame.get("id") == first_id for frame in frames), "reply_identity_changed"
    assert all(frame.get("model") == MODEL for frame in frames), "provider_model_changed"
    recognized_finish = ("stop", "length", "content_filter", "tool_calls")
    finishes = [reason if reason in recognized_finish else "unknown" for choice in choices
                if (reason := choice.get("finish_reason")) is not None]
    diagnostics.update(finish_reasons=finishes, answer_exact=text.strip() == EXPECTED[mode])
    assert finishes in (["stop"], ["length"]) and text.strip(), "answer_not_completed"
    usage = frames[-1]["usage"]
    prompt, completion = usage.get("prompt_tokens"), usage.get("completion_tokens")
    assert type(prompt) is int and prompt > 0 and type(completion) is int and 0 < completion <= MAX_COMPLETION_TOKENS, "invalid_token_counts"
    assert type(usage.get("total_tokens")) is int and usage["total_tokens"] == prompt + completion, "invalid_token_total"
    details = usage.get("prompt_tokens_details")
    assert details is None or isinstance(details, dict), "invalid_cache_fields"
    details = details or {}
    cached, written = details.get("cached_tokens"), details.get("cache_write_tokens")
    assert all(value is None or type(value) is int and 0 <= value <= prompt for value in (cached, written)), "invalid_cache_fields"
    assert (cached or 0) + (written or 0) <= prompt, "invalid_cache_total"
    counters = {"input_tokens": prompt, "output_tokens": completion,
                "cache_read_input_tokens": cached, "cache_creation_input_tokens": written}
    estimates = [frame["x_horde"].get("estimated_usage") for frame in frames if isinstance(frame.get("x_horde"), dict)]
    estimated = True if True in estimates else False if False in estimates else None
    return counters, estimated


def compare_counts(engine, capture, probe, check, seen, usage, status):
    deadline = time.monotonic() + 3
    while True:
        rows = {key: row for key, row in capture["records"](engine).items() if key not in seen}
        logged = set(probe["request_logs"](engine)) - seen
        if (rows and rows.keys() == logged) or time.monotonic() >= deadline:
            break
        time.sleep(0.05)
    assert len(rows) == 1 and rows.keys() == logged, "request_id_join"
    request_id, row = next(iter(rows.items()))
    return request_id, check(row, [{"status": 200, "usage": usage}], status, 1)


def failure_code(error):
    if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)):
        return str(error)
    return type(error).__name__


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--live", action="store_true", required=True)
    parser.parse_args()
    assert os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_workspace"
    chat = runpy.run_path(str(CAPTURE))
    capture, replay, probe = (chat[name] for name in ("capture", "base", "probe"))
    runtime = ROOT / "runtime"
    controller = replay.load_module("remote_chat_controller", runtime / "desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(runtime / "manifest.json")
    assert replay.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "pinned_python_required"
    runner = payload / manifest["engine"]["runner"]
    supervisor = replay.load_module("remote_chat_supervisor", runner)
    plan_path = capture["ccr"]["PLAN"]
    plan = json.loads(plan_path.read_bytes())
    run = ROOT / "work/proofs/optimization/chat-remote-usage" / uuid.uuid4().hex
    assert run.resolve().is_relative_to(ROOT), "proof_scope"
    run.mkdir(parents=True)
    common = {"model": MODEL, "max_completion_tokens": MAX_COMPLETION_TOKENS, "temperature": 0,
              "horde": {"availability_mode": "fail_fast", "include_reasoning": False}}
    bodies = {
        "json": {**common, "stream": False, "messages": [{"role": "user", "content": "What is 3 + 4? Reply with only the digit."}]},
        "sse": {**common, "stream": True, "stream_options": {"include_usage": True},
                "messages": [{"role": "user", "content": "What is 2 + 2? Reply with only the digit."}]},
    }
    registration = {"os": platform.platform(), "engine": manifest["engine"]["version"], "provider": "AI Horde",
                    "mode": "anonymous_free", "model": MODEL, "transports": list(bodies), "maximum_requests": 2,
                    "maximum_completion_tokens": MAX_COMPLETION_TOKENS, "timeout_seconds": TIMEOUT,
                    "provider_json_schema_requested": False,
                    "scope": "two synthetic prompts through a single owned engine; no paired savings measurement"}
    replay.write_new(run / "registration.json", registration)
    staged = replay.prepare_fixture_runtime(run, payload)
    helper = run / "fixture-runtime/desktop_helper.py"
    source = helper.read_bytes()
    anchor = b'"from headroom.cli import main; main()"'
    assert source.count(anchor) == 1, "engine_bootstrap_changed"
    guard = f"import runpy; runpy.run_path({str(PROGRAM)!r})['install_guard']({str(run)!r}, {bodies!r}); "
    helper.write_bytes(source.replace(anchor, repr(guard).encode("ascii") + b" " + anchor, 1))
    engine = probe["UsageEngine"](controller, supervisor, payload, run / "engine", 1, plan)
    settings = engine.directory / "workspace/settings.json"
    settings.write_bytes(replay.encoded({"openai_base_url": BASE + "/v1", "anthropic_base_url": "http://127.0.0.1:1"}))
    source_map = json.loads((runtime / "optimization-sources.json").read_bytes())
    inputs = [PROGRAM, CAPTURE, Path(sys.executable), runner, payload / "python313.dll",
              payload / "runtime-manifest.json", plan_path, settings, run / "registration.json",
              *staged, *[runtime / path.name for path in staged], ROOT / "tests/runtime/anthropic_usage_capture.py",
              *[PROGRAM.with_name(name) for name in ("replay.py", "fixtures.py", "surfaces.py", "anthropic_usage.py", "ccr_policy.py")],
              *[payload / "Lib/site-packages/headroom" / name for name in source_map]]
    before = replay.fingerprints(inputs)
    results, seen, failure = {}, set(), None
    try:
        engine.start()
        assert not capture["records"](engine) and not probe["request_logs"](engine), "engine_not_empty"
        for mode, body in bodies.items():
            row = results[mode] = {}
            try:
                status, media, raw = request(engine.port, body)
                row["client_status"] = status
                if status != 200:
                    try:
                        code = json.loads(raw)["error"]["code"]
                    except (ValueError, TypeError, KeyError, RecursionError):
                        code = None
                    row["known_error"] = code if code in PROVIDER_ERRORS else "unclassified"
                mime = "text/event-stream" if mode == "sse" else "application/json"
                assert status == 200 and media.startswith(mime), "live_route_response"
                usage, estimated = inspect_reply(mode, raw, row)
                assert (run / f"{mode}-status.txt").read_text(encoding="ascii") == "200", "upstream_status_mismatch"
                identity, record = compare_counts(engine, capture, probe, chat["check"], seen, usage, status)
                row.update(request_id=identity, provider_usage_estimated=estimated, usage_record=record)
            except Exception as error:
                row["failure"] = failure_code(error)
                failure = failure or row["failure"]
            finally:
                seen.update(probe["request_logs"](engine))
    except Exception as error:
        failure = failure or failure_code(error)
    finally:
        try:
            engine.close()
        except Exception:
            failure = failure or "engine_shutdown_failed"
    after = replay.fingerprints(inputs)
    attempts = {mode: int((run / f"{mode}-sent.txt").is_file()) for mode in bodies}
    passed = failure is None and before == after and sum(attempts.values()) == len(seen) == 2
    upstream_statuses = {mode: int(path.read_text(encoding="ascii")) for mode in bodies
                         if (path := run / f"{mode}-status.txt").is_file()}
    proof = run / "result.json"
    result = {**registration, "passed": passed, "failure": failure, "transport_results": results,
              "remote_attempts": attempts, "upstream_statuses": upstream_statuses, "engine_stop": engine.stopped}
    replay.write_new(proof, {**result, "before": before, "after": after})
    print(json.dumps({**result, "proof": proof.relative_to(ROOT).as_posix()}))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
