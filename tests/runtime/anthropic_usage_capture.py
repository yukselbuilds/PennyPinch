"""Exercise the desktop usage boundary with the pinned, owned Windows engine."""

import argparse
import asyncio
from concurrent.futures import ThreadPoolExecutor
from http.server import ThreadingHTTPServer
import json
from pathlib import Path
import platform
import runpy
import subprocess
import sys
import threading
import time
import uuid


ROOT = Path(__file__).resolve().parents[2]
probe_path = ROOT / "bench/optimization/anthropic_usage.py"
probe = runpy.run_path(str(probe_path))
ccr, replay, surface = (probe[name] for name in ("ccr", "replay", "surface"))
require = replay.require
SSE_CASES = {"sse-a": (31, 700, 0, 7), "sse-b": (42, 0, 200, 9)}
USAGE_PATH = "/__headroom_desktop/provider-usage"


def sse_event(kind, **fields):
    return b"event: " + kind.encode() + b"\r\ndata: " + replay.encoded({"type": kind, **fields}) + b"\r\n\r\n"


def sse_frames(model, counters):
    usage = dict(zip(probe["FIELDS"], counters, strict=True))
    output = usage.pop("output_tokens")
    return [
        sse_event("message_start", message={"id": "msg-usage-sse", "type": "message", "role": "assistant",
                                          "model": model, "content": [], "usage": {**usage, "output_tokens": 1}}),
        sse_event("content_block_start", index=0, content_block={"type": "text", "text": ""}),
        sse_event("content_block_delta", index=0, delta={"type": "text_delta", "text": "FIXTURE_OK"}),
        sse_event("content_block_stop", index=0),
        sse_event("message_delta", delta={"stop_reason": "end_turn"}, usage={"output_tokens": output}),
        sse_event("message_stop"),
    ]


class CaptureFixture(probe["UsageFixture"]):
    def respond(self, body, name):
        key = self.headers["X-Proof-Case"]
        case = key.rsplit("-t", 1)[0]
        if case == "retry" and len(self.server.captured[key]) == 1:
            failure = replay.encoded({"type": "error", "error": {"type": "rate_limit_error", "message": "fixture"}})
            self.send_response(429)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(failure)))
            self.send_header("Retry-After", "0")
            self.end_headers()
            self.wfile.write(failure)
            return
        if case not in SSE_CASES:
            return super().respond(body, name)
        require(body["stream"] is True, "fixture_sse_request")
        self.server.concurrent_streams.wait(timeout=10)
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Connection", "close")
        self.end_headers()
        for event in sse_frames(body["model"], SSE_CASES[case]):
            self.wfile.write(event)
            self.wfile.flush()
        self.close_connection = True


def usage_response(engine, token):
    headers = {} if token is None else {"X-Headroom-Desktop-Control": token}
    status, _, data = probe["exchange"](engine.port, "GET", USAGE_PATH, headers)
    return status, json.loads(data)


def records(engine):
    status, value = usage_response(engine, engine.control_token)
    require(status == 200 and set(value) == {"schema_version", "runtime_generation", "surface", "records"}
            and value["schema_version"] == 2 and value["runtime_generation"] == engine.generation
            and value["surface"] == "provider_usage", "usage_snapshot_contract")
    result = {}
    for row in value["records"]:
        require(row["request_id"] not in result, "usage_conflicting_scope")
        result[row["request_id"]] = row
    return result


def await_records(engine, identities):
    deadline = time.monotonic() + 2
    while True:
        available = records(engine)
        if identities <= available.keys():
            return available
        require(time.monotonic() < deadline, "usage_record_missing")
        time.sleep(0.01)


def check_record(row, upstream, client_status):
    require(set(row) == {"surface", "request_id", "client_status", "truncated", "attempts", "usage"}
            and row["surface"] == "anthropic_messages"
            and row["client_status"] == client_status and row["truncated"] is False
            and len(row["attempts"]) == len(upstream), "usage_attempt_coverage")
    for seen, expected in zip(row["attempts"], upstream, strict=True):
        require(set(seen) == {"status", "usage"} and seen["status"] == expected["status"]
                and probe["usage_fields"](seen["usage"] or {}) == expected["usage"], "usage_attempt_counter")
    require(row["usage"] == probe["usage_sum"]([item["usage"] for item in upstream]), "usage_scope_sum")
    return {key: value for key, value in row.items() if key != "request_id"}


def check_native_reader(binary, engine):
    frame = {"port": engine.port, "pid": engine.pids[-1], "generation": engine.generation,
             "secret": engine.control_token, "directory": str(engine.directory.parent)}
    test = "runtime::metrics_read::tests::owned_anthropic_usage_reaches_activity_detail"
    outcome = subprocess.run([str(binary), "--exact", test, "--ignored", "--test-threads=1"],
                             input=replay.encoded(frame), capture_output=True, cwd=ROOT,
                             timeout=30, creationflags=subprocess.CREATE_NO_WINDOW)
    require(outcome.returncode == 0 and b"1 passed" in outcome.stdout, "native_usage_projection_failed")


def exercise(engine, server, plan, requests, result, native_reader):
    require(usage_response(engine, None) == (404, {"error": "not_found"})
            and usage_response(engine, "0" * 32) == (404, {"error": "not_found"})
            and not records(engine), "usage_private_access")
    name = plan["surface"]
    timeout = plan["thresholds"]["request_timeout_seconds"]
    for case in probe["CASES"]:
        for turn, request in enumerate(requests[:-1], 1):
            key = f"{case}-t{turn}"
            surface["http_request"](engine.port, request, key, name, timeout)
            captured = server.captured.pop(key, [])
            require(len(captured) == 1 and captured[0]["auth_preserved"], "usage_warmup_route")
        key = f"{case}-t{len(requests)}"
        seen = set(probe["request_logs"](engine))
        client = probe["observe_client"](engine, requests[-1], case, key)
        upstream = server.captured.pop(key, [])
        require(len(upstream) == len(server.attempts[case]) == 2
                and all(item["auth_preserved"] for item in upstream), "usage_ccr_route")
        identities = set(probe["request_logs"](engine)) - seen
        require(len(identities) == 1, "usage_stats_join_not_unique")
        data = await_records(engine, identities)[next(iter(identities))]
        result[case] = check_record(data, server.attempts[case], client["status"])
        if case == "complete" and native_reader is not None:
            check_native_reader(native_reader, engine)
            result[case]["native_detail_projection"] = "pass"

    first = requests[0]
    seen = set(probe["request_logs"](engine))
    surface["http_request"](engine.port, first, "retry-t1", name, timeout)
    captured = server.captured.pop("retry-t1")
    require(len(captured) == 2 and all(item["auth_preserved"] for item in captured), "usage_retry_route")
    identities = set(probe["request_logs"](engine)) - seen
    require(len(identities) == 1, "usage_stats_join_not_unique")
    retry = await_records(engine, identities)[next(iter(identities))]
    result["retry"] = check_record(retry, [
        {"status": 429, "usage": probe["usage_fields"]({})},
        {"status": 200, "usage": probe["usage_fields"]({"input_tokens": 1, "output_tokens": 1})},
    ], 200)

    seen = set(probe["request_logs"](engine))
    with ThreadPoolExecutor(max_workers=2) as pool:
        futures = [pool.submit(surface["http_request"], engine.port, first, f"{case}-t1", name, timeout)
                   for case in SSE_CASES]
        for future in futures:
            future.result()
    logs = {key: value for key, value in probe["request_logs"](engine).items() if key not in seen}
    require(len(logs) == 2, "usage_stats_join_parallel")
    captured_records = await_records(engine, logs.keys())
    for case, values in SSE_CASES.items():
        outbound = server.captured.pop(f"{case}-t1")
        require(len(outbound) == 1 and outbound[0]["auth_preserved"], "usage_parallel_route")
        ids = [key for key, log in logs.items() if log.get("output_tokens") == values[-1]]
        require(len(ids) == 1, "usage_parallel_sample_identity")
        expected = dict(zip(probe["FIELDS"], values, strict=True))
        result[case] = check_record(captured_records[ids[0]], [{"status": 200, "usage": expected}], 200)
    require(not server.captured, "usage_extra_upstream_calls")


async def parser_boundaries(control, parse_events):
    frames = sse_frames("fixture", SSE_CASES["sse-a"])

    async def inspect(raw):
        async def chunks():
            for position in range(0, len(raw), 71):
                yield raw[position:position + 71]
        attempt = {"usage": None}
        observed = [chunk async for chunk in control._observe_anthropic_sse(chunks(), attempt, parse_events)]
        require(b"".join(observed) == raw, "usage_stream_bytes_changed")
        return attempt["usage"]

    expected = dict(zip(probe["FIELDS"], SSE_CASES["sse-a"], strict=True))
    require(await inspect(b"".join(frames)) == expected, "usage_split_frame_counters")
    require(await inspect(b"".join(frames[:-1])) is None, "usage_missing_terminal")
    malformed = b"event: message_delta\ndata: {\"type\":[]}\n\n"
    require(await inspect(b"".join([frames[0], malformed, frames[-1]])) is None, "usage_invalid_frame")
    require(await inspect(b"a" * (control.MAX_USAGE_EVENT_BYTES + 1)) is None, "usage_frame_limit")
    trace = control.UsageTrace()
    for _ in range(control.MAX_USAGE_ATTEMPTS):
        trace.begin_attempt()["usage"] = expected
    require(trace.begin_attempt() is None and trace.truncated
            and all(value is None for value in trace.snapshot(200)["usage"].values()), "usage_attempt_limit")


def main():
    require(Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "fixture_workspace")
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("--native-reader", type=Path)
    native_reader = args.parse_args().native_reader
    if native_reader is not None:
        native_reader = native_reader.resolve(strict=True)
        require(native_reader.is_relative_to(ROOT / "work/caches/cargo-target")
                and native_reader.suffix == ".exe", "native_reader_path")
    plan = json.loads(ccr["PLAN"].read_bytes())
    requests = ccr["workloads"](plan)["long"][:plan["short_turns"] + 1]
    manifest_path = ROOT / "runtime/manifest.json"
    controller = replay.load_module("capture_controller", ROOT / "runtime/desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(manifest_path)
    require(replay.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "fixture_python_pin")
    cache = controller.verify_tokenizer_cache(payload)
    runner = payload / manifest["engine"]["runner"]
    supervisor = replay.load_module("capture_supervisor", runner)
    vendor = payload / "Lib/site-packages/headroom"
    control = replay.load_module("capture_control", ROOT / "runtime/optimization_control.py")
    source_map = json.loads((ROOT / "runtime/optimization-sources.json").read_bytes())
    parser = replay.load_module("capture_sse", vendor / "proxy/sse_byte_buffer_policy.py")
    run = ROOT / "work/proofs/optimization/desktop-anthropic-usage" / str(uuid.uuid4())
    run.mkdir(parents=True)
    overlay = replay.prepare_fixture_runtime(run, payload)
    inputs = [Path(__file__), Path(sys.executable), runner, ccr["PLAN"], probe_path,
              *[probe_path.with_name(name) for name in ("ccr_policy.py", "fixtures.py", "replay.py", "surfaces.py")],
              *[ROOT / "runtime" / path.name for path in overlay], *overlay,
              *[vendor / path for path in source_map], *[path for path in cache.iterdir() if path.is_file()]]
    if native_reader is not None:
        inputs += [native_reader, ROOT / "src-tauri/src/runtime/metrics_read.rs",
                   ROOT / "src-tauri/src/metrics/normalize.rs", ROOT / "src-tauri/src/metrics/activity.rs",
                   ROOT / "src-tauri/src/metrics/store.rs"]
    before = replay.fingerprints(inputs)
    asyncio.run(parser_boundaries(control, parser.parse_sse_events_from_byte_buffer))
    server = ThreadingHTTPServer(("127.0.0.1", 0), CaptureFixture)
    server.capture_lock, server.captured = threading.Lock(), {}
    server.log_turn = len(requests)
    server.original_log = ccr["log_block"](requests[-1])["content"]
    server.attempts = {case: [] for case in probe["CASES"]}
    server.concurrent_streams = threading.Barrier(2)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    result, stops, failure = {}, [], None
    try:
        engine = probe["UsageEngine"](controller, supervisor, payload, run / "engine", server.server_port, plan)
        try:
            engine.start()
            exercise(engine, server, plan, requests, result, native_reader)
        finally:
            engine.close()
            stops.append(engine.stopped)
        restarted = probe["UsageEngine"](controller, supervisor, payload, run / "engine-restarted", server.server_port, plan)
        try:
            restarted.start()
            require(restarted.generation != engine.generation and not records(restarted)
                    and usage_response(restarted, engine.control_token) == (404, {"error": "not_found"}),
                    "usage_generation_isolation")
        finally:
            restarted.close()
            stops.append(restarted.stopped)
    except Exception as error:
        failure = type(error).__name__
        if isinstance(error, AssertionError) and str(error).isascii() and str(error).replace("_", "").isalnum():
            failure = str(error)
    finally:
        server.shutdown()
        server.server_close()
    after = replay.fingerprints(inputs)
    network = replay.fixture_network_evidence(run)
    passed = failure is None and before == after and network["huggingface_hub_payload_bytes"] == 0
    path = run / "result.json"
    replay.write_new(path, {"passed": passed, "failure": failure, "os": platform.platform(),
                            "scenarios": result, "stops": stops, "fixture_network": network,
                            "before": before, "after": after})
    print(json.dumps({"passed": passed, "failure": failure, "scenarios": len(result), "proof": path.relative_to(ROOT).as_posix()}))
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
