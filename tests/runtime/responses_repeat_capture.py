"""Verify repeated Responses results at a local Windows upstream."""
import argparse
from contextlib import nullcontext
from copy import deepcopy
import json
from http.server import ThreadingHTTPServer
import os
from pathlib import Path
import platform
import re
import runpy
import site
import sys
import threading
import uuid


ROOT = Path(__file__).resolve().parents[2]
BENCH = ROOT / "bench/optimization"
dedup = runpy.run_path(str(BENCH / "dedup.py"))
surface, base, require = (dedup[name] for name in ("surface", "base", "require"))
usage = runpy.run_path(str(BENCH / "anthropic_usage.py"))
PLAN, TIMEOUT = dedup["PLAN"], 30


def set_enabled(engine, enabled, revision):
    request = {"schema_version": 1, "request_id": str(uuid.uuid4()), "runtime_generation": engine.generation,
               "action": "set", "enabled": enabled, "expected_revision": revision}
    status, _, raw = usage["exchange"](engine.port, "POST", "/__headroom_desktop/control",
                                       {"x-headroom-desktop-control": engine.control_token}, base.encoded(request))
    state = json.loads(raw)
    require(status == 200 and state["status"] == "ok" and state["enabled"] is enabled
            and state["revision"] == revision + 1, "pause_transition_failed")
    return state["revision"]


def send_http(engine, body, headers):
    status, _, raw = usage["exchange"](engine.port, "POST", "/v1/responses", headers, base.encoded(body))
    events = [json.loads(line[6:]) for line in raw.splitlines() if line.startswith(b"data: {")]
    delta = "".join(event["delta"] for event in events if event["type"] == "response.output_text.delta")
    require(status == 200 and delta == "FIXTURE_OK"
            and any(event["type"] == "response.completed" for event in events), "http_stream_changed")


def exercise(engine, server, wire, disabled, count, rows):
    from websockets.sync.client import connect

    samples = dedup["workloads"]()
    cases = ["exact"] if disabled else ["exact", "ambiguous", "changed_arguments", "changed_failure", "source",
                                        "numeric", "no_session", "cross_session", "collision", "stateful"]
    if wire == "http" and not disabled:
        cases.append("long_log")
    for case in cases:
        key = f"{wire}-{case}"
        tool, values = samples.get(case, samples["exact"])
        if case == "exact" and not disabled:
            values = (values[0], values[1], values[1], None)
        elif case != "long_log":
            values = values[:2]
        requests = list(dedup["turns"]("responses", case, tool, values))
        if case == "exact" and not disabled:
            reused = deepcopy(requests[-1][0])
            reused["input"].append(dict(requests[0][0]["input"][-2]))
            requests.append((reused, None))
        headers = {"Authorization": "Bearer " + base.SENTINEL, "Content-Type": "application/json", "X-Proof-Case": key}
        if case != "no_session":
            headers["x-headroom-session-id"] = key
        if case == "changed_arguments":
            requests[-1][0]["input"][-2]["arguments"] = '{"fixture":"different"}'
        if case == "collision":
            requests[-1][0]["input"].append(dict(requests[0][0]["input"][-2]))
        if case == "stateful":
            for body, _ in requests:
                body["previous_response_id"] = "resp-owned-fixture"

        def socket(address, scope_headers):
            return connect(f"ws://127.0.0.1:{engine.port}/v1/responses",
                           additional_headers={**scope_headers, "X-Proof-Case": address},
                           compression=None, proxy=None, max_size=2 * 1024 * 1024,
                           open_timeout=TIMEOUT, close_timeout=5, legacy=True)

        previous = None
        ws = wire == "websocket"
        with socket(key, headers) if ws else nullcontext() as connection:
            for turn, (body, identity) in enumerate(requests):
                paused = case == "exact" and turn == 2 and not disabled
                if paused or (case == "exact" and turn == 3 and not disabled):
                    engine.control_revision = set_enabled(engine, not paused, engine.control_revision)
                scope_changed = case == "cross_session" and turn == 1
                capture_key = f"{key}-t{turn}"
                out_headers = {**headers, "X-Proof-Case": capture_key}
                if scope_changed:
                    out_headers["x-headroom-session-id"] += "-other"
                if ws:
                    frame = {**body, "type": "response.create"}
                    if scope_changed:
                        other = key + "-other"
                        with socket(other, out_headers) as changed:
                            surface["websocket_request"](changed, frame, TIMEOUT)
                        capture_key = other + "-t0"
                    else:
                        surface["websocket_request"](connection, frame, TIMEOUT)
                else:
                    send_http(engine, body, out_headers)
                with server.capture_lock:
                    captured = server.captured.pop(capture_key, [])
                require(len(captured) == 1 and captured[0]["auth_preserved"], f"{wire}_{case}_upstream")
                sent = captured[0]["body"]
                base.validate_body(body, sent, previous, False)
                expected = {call: value for _, call, value in dedup["output_items"](body, "responses")}
                actual = {call: value for _, call, value in dedup["output_items"](sent, "responses")}
                ccr_count = 0
                if case == "long_log":
                    checks = dedup["proof_for_outputs"](engine, body, sent, "responses", count)
                    require(all(row["exact"] and not row["pointers"] for row in checks.values())
                            and checks[identity]["ccr_exact"], "ccr_round_trip_changed")
                    ccr_count = sum(row["ccr_exact"] for row in checks.values())
                else:
                    require(actual == expected, f"{wire}_{case}_output_changed")
                # The WebSocket frame marker is transport metadata, outside the input comparison.
                payload = {k: v for k, v in sent.items() if not (ws and k == "type")}
                before, after = count(base.encoded(body).decode()), count(base.encoded(payload).decode())
                output_before = count(body["input"][-1]["output"]) if case == "exact" and turn == 1 else None
                output_after = count(actual[identity]) if output_before is not None else None
                if output_before is not None:
                    require(after == before and output_after == output_before, f"{wire}_repeat_count_changed")
                rows.append({"wire": wire, "dedupe": not disabled, "case": case, "turn": turn,
                             "ccr_exact": ccr_count, "paused": paused,
                             "call_id_reuse": case == "exact" and turn == 4,
                             "input_tokens": before, "forwarded_tokens": after,
                             "new_result_tokens": output_before, "forwarded_result_tokens": output_after})
                previous = sent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true", required=True)
    parser.parse_args()
    require(Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_workspace")
    runtime = ROOT / "runtime"
    controller = base.load_module("repeat_controller", runtime / "desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(runtime / "manifest.json")
    require(base.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "pinned_python_required")
    runner = payload / manifest["engine"]["runner"]
    supervisor = base.load_module("repeat_supervisor", runner)
    cache = controller.verify_tokenizer_cache(payload)
    os.environ["TIKTOKEN_CACHE_DIR"] = str(cache)
    packages = payload / "Lib/site-packages"
    site.addsitedir(str(packages))
    import tiktoken

    tokenizer = tiktoken.get_encoding("o200k_base")
    count = lambda value: len(tokenizer.encode(value, disallowed_special=()))
    run = ROOT / "work/proofs/optimization/responses-repeat" / str(uuid.uuid4())
    run.mkdir(parents=True)
    inputs = [Path(__file__), Path(sys.executable), runner, payload / "runtime-manifest.json",
              *[BENCH / name for name in ("replay.py", "surfaces.py", "dedup.py", "fixtures.py", "anthropic_usage.py", "ccr_policy.py")],
              *[packages / "headroom" / name for name in json.loads((runtime / "optimization-sources.json").read_bytes())],
              *list(cache.glob("*")), *list((packages / "tiktoken").glob("*.pyd"))]
    for label in ("default", "disabled"):
        directory = run / label
        directory.mkdir()
        overlay = base.prepare_fixture_runtime(directory, payload)
        inputs += [*overlay, *[runtime / path.name for path in overlay]]
        if label == "disabled":
            helper = directory / "fixture-runtime/desktop_helper.py"
            data = helper.read_bytes()
            anchor = b'    "HF_HUB_OFFLINE": "1",'
            require(data.count(anchor) == 1 and b'"HEADROOM_DEDUPE"' not in data, "dedup_fixture_scope")
            eol = b"\r\n" if b"\r\n" in data else b"\n"
            helper.write_bytes(data.replace(anchor, b'    "HEADROOM_DEDUPE": "0",' + eol + anchor, 1))
    before = base.fingerprints(inputs)
    server = ThreadingHTTPServer(("127.0.0.1", 0), dedup["Fixture"])
    server.capture_lock, server.captured = threading.Lock(), {}
    threading.Thread(target=server.serve_forever, daemon=True).start()
    rows, stops, failure = [], [], None
    try:
        for disabled in (False, True):
            label = "disabled" if disabled else "default"
            engine = usage["UsageEngine"](controller, supervisor, payload, run / label / "engine", server.server_port, PLAN)
            try:
                engine.start()
                require(dedup["engine_dedupe"](engine) != disabled, "effective_dedup_state")
                engine.control_revision = 0
                for wire in ("http", "websocket"):
                    exercise(engine, server, wire, disabled, count, rows)
            finally:
                engine.close()
                stops.append(engine.stopped)
        require(len(rows) == 52 and sum(row["paused"] for row in rows) == 2
                and sum(row["call_id_reuse"] for row in rows) == 2
                and any(row["ccr_exact"] for row in rows), "coverage_changed")
    except Exception as error:
        failure = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z_]+", str(error)) else type(error).__name__
    finally:
        server.shutdown()
        server.server_close()
    after = base.fingerprints(inputs)
    status = "pass" if failure is None and before == after else "fail"
    base.write_new(run / "result.json", {"status": status, "failure": failure, "os": platform.platform(),
                   "engine": manifest["engine"]["version"], "inputs_before": before, "inputs_after": after,
                   "rows": rows, "stops": stops})
    print(json.dumps({"status": status, "failure": failure, "rows": len(rows), "result": str(run / "result.json")}))
    return status != "pass"


if __name__ == "__main__":
    raise SystemExit(main())
