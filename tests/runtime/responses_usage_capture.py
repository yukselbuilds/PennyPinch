"""Owned Responses HTTP/SSE usage through the real pinned engine; loopback only."""
import argparse
from concurrent.futures import ThreadPoolExecutor
from http.server import ThreadingHTTPServer
import json
from pathlib import Path
import platform
import re
import runpy
import subprocess
import sys
import threading
import uuid

ROOT = Path(__file__).resolve().parents[2]
capture = runpy.run_path(str(ROOT / "tests/runtime/anthropic_usage_capture.py"))
probe = capture["probe"]
replay, ccr = capture["replay"], capture["ccr"]
require = replay.require
FIELDS = probe["FIELDS"]
CASES = ("complete", "missing_write", "missing_usage", "failed_continuation", "zero", "retry")
STREAMS = {"sse-a": (700, 600, 0, 7), "sse-b": (800, 0, 200, 9)}
MARKER = re.compile(r"(?:hash=|<<ccr:)([a-fA-F0-9]{12,24})(?=[^a-fA-F0-9]|$)")


def response(body, counters, output=None):
    usage = None
    if counters is not None:
        total, cached, written, generated = counters
        details = {"cached_tokens": cached}
        if written is not None:
            details["cache_write_tokens"] = written
        usage = {"input_tokens": total, "input_tokens_details": details,
                 "output_tokens": generated, "total_tokens": total + generated}
    return {"id": "resp-fixture", "object": "response", "model": body["model"], "status": "completed",
            "output": output if output is not None else [{"type": "message", "id": "msg-fixture", "role": "assistant",
                "status": "completed", "content": [{"type": "output_text", "text": "FIXTURE_OK", "annotations": []}]}], "usage": usage}


def normalized(counters):
    return dict(zip(FIELDS, counters, strict=True)) if counters is not None else None


class Fixture(capture["CaptureFixture"]):
    def do_POST(self):
        require(self.path == "/v1/responses", "responses_fixture_path")
        size = int(self.headers.get("Content-Length", "0"))
        require(0 < size <= 2 * 1024 * 1024, "responses_fixture_body_limit")
        body = json.loads(self.rfile.read(size))
        case = self.headers["X-Proof-Case"]
        with self.server.capture_lock:
            calls = self.server.calls.setdefault(case, [])
            index = len(calls)
            row = {"status": 200, "usage": None,
                   "auth_preserved": self.headers.get("Authorization") == "Bearer " + replay.SENTINEL,
                   "model_preserved": body["model"] == "gpt-6.1-sol",
                   "reasoning_preserved": body.get("reasoning") == {"effort": "xhigh"}}
            calls.append(row)
        status, counters, output = 200, (13, 0, None, 2), None
        if case in STREAMS:
            require(body["stream"] is True, "responses_sse_route")
            self.server.concurrent_streams.wait(timeout=10)
            counters = STREAMS[case]
        elif case in CASES[:4]:
            if index == 0:
                candidates = [(item, MARKER.search(item.get("output", ""))) for item in body["input"]
                              if item.get("type") == "function_call_output" and isinstance(item.get("output"), str)]
                candidates = [(item, match) for item, match in candidates if match]
                require(bool(candidates), "responses_ccr_marker_missing")
                item, match = candidates[0]
                self.server.original[case] = replay.fixtures.output("repetitive_logs", int(item["call_id"].rsplit("-", 1)[1]))
                output = [{"type": "function_call", "id": "fc-fixture", "call_id": "call-retrieve",
                           "name": "headroom_retrieve", "arguments": json.dumps({"hash": match[1]})}]
                counters = (1000, 900, 100, 6)
            else:
                retrieved = [json.loads(item["output"]) for item in body["input"]
                             if item.get("type") == "function_call_output" and item.get("call_id") == "call-retrieve"]
                require(len(retrieved) == 1 and retrieved[0]["original_content"] == self.server.original[case],
                        "responses_ccr_original_changed")
                row["original_retrieved"] = True
                counters = (1400, 1100, None if case == "missing_write" else 0, 23)
                if case == "missing_usage":
                    counters = None
                if case == "failed_continuation":
                    status, counters = 400, None
        elif case == "zero":
            counters = (0, 0, 0, 0)
        elif case == "retry" and index == 0:
            status, counters = 429, None
        row.update(status=status, usage=normalized(counters))
        self.send_response(status)
        self.send_header("Connection", "close")
        if case in STREAMS:
            self.send_header("Content-Type", "text/event-stream")
            self.end_headers()
            frames = [capture["sse_event"]("response.created", response=response(body, None)),
                      capture["sse_event"]("response.completed", response=response(body, counters))]
            for frame in frames:
                for position in range(0, len(frame), 71):
                    self.wfile.write(frame[position:position + 71])
                    self.wfile.flush()
        else:
            data = replay.encoded(response(body, counters, output) if status == 200 else {"error": {"message": "fixture"}})
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Retry-After", "0")
            self.end_headers()
            self.wfile.write(data)
        self.close_connection = True


def client(engine, body, case):
    headers = {"Content-Type": "application/json", "Authorization": "Bearer " + replay.SENTINEL,
               "X-Proof-Case": case, "x-headroom-session-id": case}
    if case in STREAMS:
        headers["X-Headroom-Bypass"] = "true"
    status, _, raw = probe["exchange"](engine.port, "POST", "/v1/responses", headers, replay.encoded(body))
    if case != "failed_continuation":
        require(status == 200 and b"FIXTURE_OK" in raw, "responses_client_answer_changed")
    else:
        value = json.loads(raw)
        print(json.dumps({"failure_case_http_status": status,
                          "response_keys": sorted(value)[:6] if isinstance(value, dict) else [],
                          "completed_answer": isinstance(value, dict) and value.get("status") == "completed"}), flush=True)
        require(status >= 400 and isinstance(value, dict) and value.get("error") is not None
                and b"FIXTURE_OK" not in raw and value.get("status") != "completed",
                "responses_failed_continuation_hidden")
    return status


def check(row, calls, status):
    require(row["surface"] == "openai_responses" and row["client_status"] == status and not row["truncated"],
            "responses_record_scope")
    require(len(row["attempts"]) == len(calls) and all(
        call["auth_preserved"] and call["model_preserved"] and call["reasoning_preserved"] for call in calls),
        "responses_attempt_scope")
    require(row["attempts"] == [{"status": call["status"], "usage": call["usage"]} for call in calls],
            "responses_raw_counters")
    usage = {}
    for field in FIELDS:
        values = [(call["usage"] or {}).get(field) if 200 <= call["status"] < 300 else None for call in calls]
        usage[field] = sum(values) if all(value is not None for value in values) else None
    usage["total_input_tokens"] = usage["input_tokens"]
    usage["total_tokens"] = (usage["input_tokens"] + usage["output_tokens"]
                              if usage["input_tokens"] is not None and usage["output_tokens"] is not None else None)
    require(row["usage"] == usage, "responses_aggregate_counters")
    return {"attempts": len(calls), "usage": usage, "client_status": status,
            "original_retrieved": any(call.get("original_retrieved") for call in calls)}


def main():
    require(Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "fixture_workspace")
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("--native-reader", type=Path, required=True)
    args.add_argument("--case", choices=CASES)
    options = args.parse_args()
    binary = options.native_reader.resolve(strict=True)
    cases = (options.case,) if options.case else CASES
    streams = {} if options.case else STREAMS
    require(binary.is_relative_to(ROOT / "work/caches/cargo-target") and binary.suffix == ".exe", "native_reader_path")
    plan = json.loads(ccr["PLAN"].read_bytes())
    controller = replay.load_module("responses_capture_controller", ROOT / "runtime/desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(ROOT / "runtime/manifest.json")
    require(replay.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "fixture_python_pin")
    supervisor = replay.load_module("responses_capture_supervisor", payload / manifest["engine"]["runner"])
    run = ROOT / "work/proofs/optimization/desktop-responses-usage/runs" / str(uuid.uuid4())
    run.mkdir(parents=True)
    overlay = replay.prepare_fixture_runtime(run, payload)
    sources = [Path(__file__), Path(sys.executable), binary, ccr["PLAN"], payload / "runtime-manifest.json",
               ROOT / "tests/runtime/anthropic_usage_capture.py", ROOT / "src-tauri/src/runtime/metrics_read.rs",
               ROOT / "src-tauri/src/metrics/normalize.rs", ROOT / "src-tauri/src/metrics/store.rs", *overlay,
               *[ROOT / "runtime" / path.name for path in overlay],
               *[ROOT / "bench/optimization" / name for name in ("fixtures.py", "surfaces.py", "replay.py", "anthropic_usage.py", "ccr_policy.py")]]
    source_map = json.loads((ROOT / "runtime/optimization-sources.json").read_bytes())
    sources += [payload / "Lib/site-packages/headroom" / name for name in source_map]
    before = replay.fingerprints(sources)
    short = {"model": "gpt-6.1-sol", "reasoning": {"effort": "xhigh"}, "stream": False,
             "input": [{"role": "user", "content": "Report FIXTURE_OK."}]}
    long = replay.fixtures.conversation("repetitive_logs", "gpt-6.1-sol", "xhigh", 1)[-1]
    long["stream"] = False
    server = ThreadingHTTPServer(("127.0.0.1", 0), Fixture)
    server.capture_lock, server.calls, server.original = threading.Lock(), {}, {}
    server.concurrent_streams = threading.Barrier(2)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    scenarios, stops, failure = {}, [], None
    engine = probe["UsageEngine"](controller, supervisor, payload, run / "engine", server.server_port, plan)
    try:
        try:
            engine.start()
            require(capture["usage_response"](engine, None) == (404, {"error": "not_found"}) and not capture["records"](engine),
                    "responses_usage_private_access")
            for case in cases:
                seen = set(probe["request_logs"](engine))
                status = client(engine, long if case in CASES[:4] else short, case)
                ids = set(probe["request_logs"](engine)) - seen
                require(len(ids) == 1, "responses_request_id_not_unique")
                row = capture["await_records"](engine, ids)[next(iter(ids))]
                scenarios[case] = check(row, server.calls[case], status)
                if case == "complete":
                    frame = {"port": engine.port, "pid": engine.pids[-1], "generation": engine.generation,
                             "secret": engine.control_token, "directory": str(run)}
                    result = subprocess.run([str(binary), "--exact", "runtime::metrics_read::tests::owned_responses_usage_reaches_activity_detail",
                        "--ignored", "--test-threads=1"], input=replay.encoded(frame), capture_output=True, cwd=ROOT, timeout=30,
                        creationflags=subprocess.CREATE_NO_WINDOW)
                    require(result.returncode == 0 and b"1 passed" in result.stdout, "responses_native_projection_failed")
                    scenarios[case]["native_detail_projection"] = "pass"
            seen = set(probe["request_logs"](engine))
            with ThreadPoolExecutor(max_workers=2) as pool:
                futures = {case: pool.submit(client, engine, {**short, "stream": True}, case) for case in streams}
                statuses = {case: future.result() for case, future in futures.items()}
            ids = set(probe["request_logs"](engine)) - seen
            require(len(ids) == len(streams), "responses_parallel_identity")
            rows = capture["await_records"](engine, ids)
            for case, counts in streams.items():
                matched = [rows[item] for item in ids if rows[item]["usage"]["input_tokens"] == counts[0]]
                require(len(matched) == 1, "responses_parallel_counters")
                scenarios[case] = check(matched[0], server.calls[case], statuses[case])
        finally:
            engine.close()
            stops.append(engine.stopped)
        restarted = probe["UsageEngine"](controller, supervisor, payload, run / "engine-restarted", server.server_port, plan)
        try:
            restarted.start()
            require(restarted.generation != engine.generation and not capture["records"](restarted)
                    and capture["usage_response"](restarted, engine.control_token) == (404, {"error": "not_found"}),
                    "responses_usage_generation_isolation")
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
    after = replay.fingerprints(sources)
    network = replay.fixture_network_evidence(run)
    passed = failure is None and before == after and network["huggingface_hub_payload_bytes"] == 0
    output = run / "result.json"
    replay.write_new(output, {"passed": passed, "failure": failure, "os": platform.platform(), "surface": "Responses HTTP/SSE",
        "scenarios": scenarios, "stops": stops, "fixture_network": network, "before": before, "after": after})
    print(json.dumps({"passed": passed, "failure": failure, "scenarios": len(scenarios), "proof": output.relative_to(ROOT).as_posix()}))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
