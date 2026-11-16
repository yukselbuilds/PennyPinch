"""Chat Completions usage through the real pinned desktop engine and native reader."""
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
probe, base = capture["probe"], capture["replay"]
surfaces = probe["surface"]
require = base.require
PATH = "/v1/chat/completions"
SURFACE = "openai_chat_completions"
MODEL = "o4-mini"
SECOND_MODEL = "o3-mini"
FIELDS = probe["FIELDS"]
MARKER = re.compile(r"(?:hash=|<<ccr:)([a-fA-F0-9]{12,24})(?=[^a-fA-F0-9]|$)")
CASES = ("json", "zero", "retry", "ccr-json", "ccr-error", "sse-disabled", "sse-cut")
PARALLEL = {"sse-a": (700, 600, 0, 7), "sse-b": (800, 0, None, 9)}


def wire_usage(counts):
    if counts is None:
        return None
    prompt, cached, written, completion = counts
    details = {"cached_tokens": cached}
    if written is not None:
        details["cache_write_tokens"] = written
    return {"prompt_tokens": prompt, "completion_tokens": completion,
            "total_tokens": prompt + completion, "prompt_tokens_details": details}


def model(case):
    return SECOND_MODEL if case == "sse-b" else MODEL


def frames(case, counts, *, done=True):
    common = {"object": "chat.completion.chunk", "id": f"chatcmpl-{case}", "model": model(case), "created": 1}
    rows = [{**common, "choices": [{"index": 0, "delta": {"role": "assistant", "content": "FIXTURE_OK λ"}, "finish_reason": None}], "usage": None},
            {**common, "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}], "usage": None}]
    if counts is not None:
        rows.append({**common, "choices": [], "usage": wire_usage(counts)})
    return b"".join(b"data: " + base.encoded(row) + b"\r\n\r\n" for row in rows) + (b"data: [DONE]\r\n\r\n" if done else b"")


class Fixture(capture["CaptureFixture"]):
    def do_POST(self):
        require(self.path == PATH, "chat_upstream_path")
        size = int(self.headers.get("Content-Length", "0"))
        require(0 < size <= 2 * 1024 * 1024, "chat_body_limit")
        body = json.loads(self.rfile.read(size))
        case = self.headers["X-Proof-Case"]
        with self.server.capture_lock:
            calls = self.server.calls.setdefault(case, [])
            index = len(calls)
            call = {}
            calls.append(call)

        require(body["model"] == model(case) and body["reasoning_effort"] == "high"
                and self.headers.get("Authorization") == "Bearer " + base.SENTINEL, "chat_settings_forwarded")
        streaming = case.startswith("sse-")
        require(body.get("stream", False) is streaming, "chat_upstream_stream_mode")
        if streaming:
            require(body["stream_options"]["include_usage"] is (case != "sse-disabled"), "chat_stream_options_preserved")
        status, counts = 200, (13, 0, None, 2)
        message = {"role": "assistant", "content": "FIXTURE_OK λ"}
        if case.startswith("ccr-"):
            if index == 0:
                for item in body["messages"]:
                    if item["role"] == "tool" and (match := MARKER.search(item["content"])):
                        break
                else:
                    raise AssertionError("chat_ccr_marker_missing")
                self.server.original[case] = base.fixtures.output("repetitive_logs", int(item["tool_call_id"].rsplit("-", 1)[1]))
                counts = (1000, 900, 100, 6)
                message = {"role": "assistant", "content": None, "tool_calls": [{"id": "call-retrieve", "type": "function",
                    "function": {"name": "headroom_retrieve", "arguments": json.dumps({"hash": match[1]})}}]}
            else:
                retrieved = [json.loads(item["content"]) for item in body["messages"]
                             if item["role"] == "tool" and item.get("tool_call_id") == "call-retrieve"]
                require(len(retrieved) == 1 and retrieved[0]["original_content"] == self.server.original[case], "chat_ccr_original_changed")
                call["original_retrieved"] = True
                counts = (1400, 1100, 0, 23)
                if case == "ccr-error":
                    status, counts = 400, None
        elif case == "zero":
            counts = (0, 0, 0, 0)
        elif case == "retry" and index == 0:
            status, counts = 429, None
        elif case in PARALLEL:
            counts = PARALLEL[case]
            self.server.parallel.wait(timeout=10)
        elif case == "sse-disabled":
            counts = None

        observed = dict(zip(FIELDS, counts, strict=True)) if counts is not None and case != "sse-cut" else None
        call.update(status=status, usage=observed)
        self.send_response(status)
        self.send_header("Connection", "close")
        if streaming:
            self.send_header("Content-Type", "text/event-stream")
            self.end_headers()
            data = frames(case, counts, done=case != "sse-cut")
            for start in range(0, len(data), 71):
                self.wfile.write(data[start:start + 71])
                self.wfile.flush()
        else:
            data = base.encoded({"object": "chat.completion", "id": f"chatcmpl-{case}-{index}", "model": model(case),
                "created": 1, "choices": [{"index": 0, "message": message,
                    "finish_reason": "tool_calls" if "tool_calls" in message else "stop"}],
                "usage": wire_usage(counts)} if status == 200 else {"error": {"message": "fixture", "type": "fixture_error"}})
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Retry-After", "0")
            self.end_headers()
            self.wfile.write(data)
        self.close_connection = True


def client(engine, case, long):
    headers = {"Content-Type": "application/json", "Authorization": "Bearer " + base.SENTINEL,
               "X-Proof-Case": case, "x-headroom-session-id": case}
    stream = case.startswith("sse-") or case == "ccr-sse"
    prompt = {"role": "user", "content": f"Report FIXTURE_OK for fixture {case}."}
    body = {"model": model(case), "reasoning_effort": "high", "stream": stream,
            "messages": [prompt]}
    if case.startswith("ccr-"):
        body = {**long, **body, "messages": [prompt, *long["messages"][1:]]}
    if stream:
        body["stream_options"] = {"include_usage": case != "sse-disabled"}
    status, media, raw = probe["exchange"](engine.port, "POST", PATH, headers, base.encoded(body))
    if case == "ccr-error":
        # The direct JSON path can fall back to the initial tool call.
        require(status == 200 and b"headroom_retrieve" in raw and b"FIXTURE_OK" not in raw, "chat_ccr_failure_passthrough")
    else:
        require(status == 200 and b"FIXTURE_OK" in raw, "chat_client_answer")
    if case in PARALLEL or case == "sse-disabled":
        require(media.startswith("text/event-stream") and raw == frames(case, PARALLEL.get(case)), "chat_stream_bytes_changed")
    return status


def check(row, calls, status, attempts):
    require(len(calls) == attempts and row["surface"] == SURFACE and row["client_status"] == status
            and not row["truncated"], "chat_record_scope")
    require(row["attempts"] == [{key: call[key] for key in ("status", "usage")} for call in calls], "chat_raw_usage_mismatch")
    counts = {name: sum(values) if all(value is not None for value in values) else None for name in FIELDS
              if (values := [(call["usage"] or {}).get(name) for call in calls])}
    counts["total_input_tokens"] = counts["input_tokens"]
    counts["total_tokens"] = (counts["input_tokens"] + counts["output_tokens"]
                             if None not in (counts["input_tokens"], counts["output_tokens"]) else None)
    require(row["usage"] == counts, "chat_usage_aggregate_mismatch")
    return {"client_status": status, "attempts": attempts, "upstream_statuses": [call["status"] for call in calls],
            "usage": counts, "original_retrieved": any(call.get("original_retrieved") for call in calls)}


def main():
    require(Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_fixture_workspace")
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("--native-reader", type=Path, required=True)
    args.add_argument("--case", choices=(*CASES, "ccr-sse"))
    options = args.parse_args()
    binary = options.native_reader.resolve(strict=True)
    require(binary.is_relative_to(ROOT / "work/caches/cargo-target") and binary.suffix == ".exe", "native_reader_path")
    plan_path = capture["ccr"]["PLAN"]
    plan = json.loads(plan_path.read_bytes())
    controller = base.load_module("chat_usage_controller", ROOT / "runtime/desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(ROOT / "runtime/manifest.json")
    require(base.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "fixture_python_pin")
    supervisor = base.load_module("chat_usage_supervisor", payload / manifest["engine"]["runner"])
    run = ROOT / "work/proofs/optimization/desktop-chat-usage" / str(uuid.uuid4())
    run.mkdir(parents=True)
    overlay = base.prepare_fixture_runtime(run, payload)
    sources = [Path(__file__), Path(sys.executable), binary, plan_path, payload / "runtime-manifest.json", *overlay,
               *[ROOT / "runtime" / path.name for path in overlay], ROOT / "src-tauri/src/runtime/metrics_read.rs",
               ROOT / "src-tauri/src/metrics/normalize.rs", ROOT / "src-tauri/src/metrics/store.rs",
               ROOT / "tests/runtime/anthropic_usage_capture.py",
               *[ROOT / "bench/optimization" / name for name in ("fixtures.py", "surfaces.py", "replay.py", "anthropic_usage.py", "ccr_policy.py")]]
    vendor = payload / "Lib/site-packages/headroom"
    source_map = json.loads((ROOT / "runtime/optimization-sources.json").read_bytes())
    sources += [vendor / name for name in [*source_map, "providers/route_specs.py", "proxy/provider_usage.py",
                                          "proxy/outcome.py", "cli/main.py", "settings_store.py", "paths.py"]]
    long = surfaces["conversation"]("chat_http_sse", "repetitive_logs", MODEL, 1)[-1]
    server = ThreadingHTTPServer(("127.0.0.1", 0), Fixture)
    server.capture_lock, server.calls, server.original = threading.Lock(), {}, {}
    server.parallel = threading.Barrier(2)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    result = {"passed": False, "os": platform.platform(), "scenarios": {}, "stops": [], "failure": None}
    engine = probe["UsageEngine"](controller, supervisor, payload, run / "engine", server.server_port, plan)
    settings = engine.directory / "workspace/settings.json"
    if options.case == "ccr-sse":
        value = json.loads(settings.read_bytes())
        value["ccr_buffered_chat_streaming"] = True
        settings.write_bytes(base.encoded(value))
    sources.append(settings)
    before = base.fingerprints(sources)
    stage = "start"
    try:
        try:
            engine.start()
            require(capture["usage_response"](engine, None) == (404, {"error": "not_found"}) and not capture["records"](engine), "chat_usage_private")
            for case in ((options.case,) if options.case else CASES):
                stage = case
                seen = set(probe["request_logs"](engine))
                previously_observed = set(capture["records"](engine))
                status = client(engine, case, long)
                ids = set(probe["request_logs"](engine)) - seen
                if case == "sse-cut":
                    # The vendor counts failed streams without a /stats request row.
                    records = capture["records"](engine)
                    added = records.keys() - previously_observed
                    require(not ids and len(added) == 1, "chat_cut_private_record_only")
                    row = records[next(iter(added))]
                else:
                    require(len(ids) == 1, "chat_request_id_unique")
                    row = capture["await_records"](engine, ids)[next(iter(ids))]
                result["scenarios"][case] = check(row, server.calls[case], status, 2 if case.startswith("ccr-") or case == "retry" else 1)
                result["scenarios"][case]["stats_row"] = bool(ids)
                if case == "json" and not options.case:
                    stage = "local_response_cache"
                    seen = set(probe["request_logs"](engine))
                    client(engine, case, long)
                    cached = {key: row for key, row in probe["request_logs"](engine).items() if key not in seen}
                    require(len(cached) == 1 and all(row.get("cache_hit") for row in cached.values())
                            and len(server.calls[case]) == 1 and not (cached.keys() & capture["records"](engine).keys()),
                            "chat_local_cache_no_model_send")
                    result["scenarios"][stage] = {"cached": True, "model_sends": 0, "provider_usage_record": False}
                if case == "ccr-json":
                    stage = "native_projection"
                    frame = {"port": engine.port, "pid": engine.pids[-1], "generation": engine.generation,
                             "secret": engine.control_token, "directory": str(run)}
                    test = subprocess.run([str(binary), "--exact", "runtime::metrics_read::tests::owned_chat_usage_reaches_activity_detail",
                        "--ignored", "--test-threads=1"], input=base.encoded(frame), capture_output=True, cwd=ROOT, timeout=30,
                        creationflags=subprocess.CREATE_NO_WINDOW)
                    require(test.returncode == 0 and b"1 passed" in test.stdout, "chat_native_projection_failed")
                    result["scenarios"][case]["native_detail_projection"] = "pass"
            if not options.case:
                stage = "parallel_sse"
                seen = set(probe["request_logs"](engine))
                with ThreadPoolExecutor(max_workers=2) as pool:
                    statuses = dict(zip(PARALLEL, pool.map(lambda case: client(engine, case, long), PARALLEL), strict=True))
                ids = set(probe["request_logs"](engine)) - seen
                require(len(ids) == len(PARALLEL), "chat_parallel_ids")
                rows = capture["await_records"](engine, ids)
                for case, counts in PARALLEL.items():
                    matched = [rows[item] for item in ids if rows[item]["usage"]["input_tokens"] == counts[0]]
                    require(len(matched) == 1, "chat_parallel_counter_ownership")
                    result["scenarios"][case] = check(matched[0], server.calls[case], statuses[case], 1)
        finally:
            engine.close()
            result["stops"].append(engine.stopped)
        if not options.case:
            stage = "restart"
            restarted = probe["UsageEngine"](controller, supervisor, payload, run / "restarted", server.server_port, plan)
            try:
                restarted.start()
                require(restarted.generation != engine.generation and not capture["records"](restarted)
                        and capture["usage_response"](restarted, engine.control_token) == (404, {"error": "not_found"}), "chat_generation_isolation")
            finally:
                restarted.close()
                result["stops"].append(restarted.stopped)
    except Exception as error:
        reason = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)) else type(error).__name__
        result["failure"] = {"stage": stage, "code": reason, "upstream_statuses": {case: [call.get("status") for call in calls]
                             for case, calls in server.calls.items()}}
    finally:
        server.shutdown()
        server.server_close()
    after = base.fingerprints(sources)
    network = base.fixture_network_evidence(run)
    result.update(passed=result["failure"] is None and before == after and network["huggingface_hub_payload_bytes"] == 0,
                  before=before, after=after, fixture_network=network)
    proof = run / "result.json"
    base.write_new(proof, result)
    print(json.dumps({"passed": result["passed"], "failure": result["failure"], "scenarios": len(result["scenarios"]),
                      "proof": proof.relative_to(ROOT).as_posix()}))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
