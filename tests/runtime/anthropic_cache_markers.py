"""Compare Anthropic cache directives and isolated provider counters on Windows."""
import argparse
from copy import deepcopy
from http.server import ThreadingHTTPServer
import json
from pathlib import Path
import platform
import re
import runpy
import sys
import threading
import time
import uuid


ROOT = Path(__file__).resolve().parents[2]
capture = runpy.run_path(str(Path(__file__).with_name("anthropic_usage_capture.py")))
replay, surface, probe, require = (capture[name] for name in ("replay", "surface", "probe", "require"))
PLAN = {"thresholds": {"startup_timeout_seconds": 120}}
EPHEMERAL = {"type": "ephemeral"}
LONG = {**EPHEMERAL, "ttl": "1h"}
CASES = {
    "legacy": (0, 2, 19, 20, 22, 2),
    "client_blocks": (0, 2, 19, 20, 22, 2),
    "budget": (0, 2, 2, 2),
    "native": (0, 2, 2),
    "native_paused": (0, 2, 2),
    "native_and_blocks": (0, 2),
}


def usage_values(turn):
    # Independent from the directives. In particular zero and nonzero reads coexist with either placement.
    return (62, 1024, 0, 12) if turn % 2 else (61, 0, 1024, 11)


class Fixture(surface["SurfaceHandler"]):
    def respond(self, body, _surface):
        turn = int(self.headers["X-Proof-Case"].rsplit("-t", 1)[1])
        counts = usage_values(turn)
        if body["stream"]:
            content, mime = b"".join(capture["sse_frames"](body["model"], counts)), "text/event-stream"
        else:
            result = {"type": "message", "id": f"msg-marker-{turn}", "role": "assistant", "model": body["model"],
                      "content": [{"type": "text", "text": "FIXTURE_OK"}], "stop_reason": "end_turn",
                      "stop_sequence": None, "usage": dict(zip(probe["FIELDS"], counts, strict=True))}
            content, mime = replay.encoded(result), "application/json"
        self.send_response(200)
        self.send_header("Content-Type", mime)
        self.send_header("Content-Length", str(len(content)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(content)


def policy_requests(case, widths):
    def text(label, n):
        return {"type": "text", "text": f"Owned fixture {label} position {n}."}

    messages = [{"role": "user", "content": [text(case, n) for n in range(5)]}]
    for turn, width in enumerate(widths):
        prior = len(messages) - 1
        if width:
            messages.extend([{"role": "assistant", "content": [text(f"reply {turn}", 0)]},
                             {"role": "user", "content": [text(f"step {turn}", n) for n in range(1, width)]}])
        system = [text("system", n) for n in range(4)]
        tools = [{"name": "inspect_workspace", "description": "Inspect fixture", "input_schema": {"type": "object"}}]
        body = {"model": "claude-sonnet-4-20250514", "stream": turn % 2 == 1, "max_tokens": 2048,
                "thinking": {"type": "enabled", "budget_tokens": 1024}, "messages": deepcopy(messages),
                "system": system, "tools": tools}
        if case == "client_blocks":
            body["messages"][-1]["content"][-1]["cache_control"] = EPHEMERAL
            if turn:
                body["messages"][prior]["content"][-1]["cache_control"] = EPHEMERAL
        if case == "budget" and turn:
            for item in system[:4 if turn == 2 else 3]:
                item["cache_control"] = EPHEMERAL
            if turn == 3:
                tools[0]["cache_control"] = LONG
        if case == "native_and_blocks":
            system[0]["cache_control"] = LONG
            body["messages"][0]["content"][0]["cache_control"] = LONG
        if (case in {"native", "native_paused"} and turn == 1) or case == "native_and_blocks":
            body["cache_control"] = LONG
        yield body


def message_blocks(body):
    return [block for message in body["messages"] for block in message["content"]]


def markers(body):
    return [(index, block["cache_control"]) for index, block in enumerate(message_blocks(body)) if "cache_control" in block]


def prefix_without_directives(body):
    return [{**message, "content": [{key: value for key, value in block.items() if key != "cache_control"}
                                     for block in message["content"]]} for message in body["messages"]]


def set_enabled(engine, revision, enabled):
    value = {"schema_version": 1, "request_id": str(uuid.uuid4()), "runtime_generation": engine.generation,
             "action": "set", "expected_revision": revision, "enabled": enabled}
    status, _, response = probe["exchange"](engine.port, "POST", "/__headroom_desktop/control",
                                            {"X-Headroom-Desktop-Control": engine.control_token}, replay.encoded(value))
    state = json.loads(response)
    require(status == 200 and state["status"] == "ok" and state["enabled"] is enabled
            and state["revision"] == revision + 1, "cache_fixture_pause_transition_failed")
    return state["revision"]


def exercise(engine, server, baseline, rows):
    seen, revision = set(), 0
    for case, widths in CASES.items():
        previous = None
        for turn, body in enumerate(policy_requests(case, widths)):
            label = f"{case}-t{turn}"
            if case == "native_paused" and turn:
                revision = set_enabled(engine, revision, enabled=turn != 1)
            headers = {"Content-Type": "application/json", "x-api-key": replay.SENTINEL,
                       "Authorization": "Bearer " + replay.SENTINEL, "anthropic-version": "2023-06-01",
                       "X-Proof-Case": label, "x-headroom-session-id": "cache-markers-" + case}
            status, mime, response = probe["exchange"](engine.port, "POST", "/v1/messages", headers, replay.encoded(body))
            require(status == 200 and b"FIXTURE_OK" in response
                    and (mime.startswith("text/event-stream") and b"message_stop" in response if body["stream"]
                         else mime.startswith("application/json")), f"{case}_{turn}_client_output")
            with server.capture_lock:
                upstream = server.captured.pop(label, [])
            require(len(upstream) == 1 and upstream[0]["auth_preserved"], f"{case}_{turn}_one_upstream")
            sent = upstream[0]["body"]
            for field in ("model", "thinking", "system", "cache_control", "stream"):
                require(sent.get(field) == body.get(field), f"{case}_{turn}_{field}_changed")
            tool_list = sent.get("tools", [])
            ccr_tools = [tool for tool in tool_list if tool.get("name") == "headroom_retrieve"]
            require(len(ccr_tools) <= 1 and all("cache_control" not in tool for tool in ccr_tools)
                    and [tool for tool in tool_list if tool not in ccr_tools] == body["tools"], f"{case}_{turn}_tools_changed")
            require(prefix_without_directives(sent) == prefix_without_directives(body), f"{case}_{turn}_messages_changed")
            if previous:
                require(prefix_without_directives(sent)[:len(previous["messages"])] == prefix_without_directives(previous),
                        f"{case}_{turn}_sent_prefix_content_changed")

            client_marks, sent_marks = markers(body), markers(sent)
            last = len(message_blocks(sent)) - 1
            full_budget = case == "budget" and turn in (2, 3)
            native_fallback_conflict = case in {"native", "native_paused"} and turn == 1
            if client_marks or (case == "native_paused" and turn == 1) or (not baseline and (full_budget or native_fallback_conflict)):
                expected = client_marks
            else:
                expected = [(last, EPHEMERAL)]
            require(sent_marks == expected, f"{case}_{turn}_incorrect_marker_placement")
            prior_in_window = None
            if case in {"legacy", "client_blocks"} and turn:
                previous_write = markers(previous)[-1][0]
                require(last - previous_write == widths[turn], "text_block_distance_changed")
                prior_in_window = any(0 <= index - previous_write < 20 for index, _ in sent_marks)
                require(prior_in_window == (case == "client_blocks" or widths[turn] < 20), "lookback_coverage_changed")

            deadline = time.monotonic() + 2
            available = capture["records"](engine)
            while not (available.keys() - seen) and time.monotonic() < deadline:
                time.sleep(0.01)
                available = capture["records"](engine)
            identities = available.keys() - seen
            require(len(identities) == 1, "private_usage_missing_or_ambiguous")
            request_id = next(iter(identities))
            counts = dict(zip(probe["FIELDS"], usage_values(turn), strict=True))
            expected_attempt = {"status": 200, "usage": counts}
            checked = capture["check_record"](available[request_id], [expected_attempt], status)
            seen.add(request_id)
            rows.append({"case": case, "turn": turn, "stream": body["stream"], "width": widths[turn],
                         "client_message_markers": client_marks, "wire_message_markers": sent_marks,
                         "client_system_and_tool_markers": sum("cache_control" in item for item in body["system"] + body["tools"]),
                         "provider_automatic_cache": body.get("cache_control"), "prior_write_in_documented_window": prior_in_window,
                         "old_wire_prefix_exact": previous is None or sent["messages"][:len(previous["messages"])] == previous["messages"],
                         "documented_budget_conflict": full_budget and bool(sent_marks),
                         "native_with_legacy_marker": native_fallback_conflict and bool(sent_marks),
                         "provider_fixture_usage": checked["usage"]})
            previous = sent


def main():
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("mode", choices=("baseline", "candidate"))
    mode = args.parse_args().mode
    require(Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_workspace")
    runtime = ROOT / "runtime"
    controller = replay.load_module("marker_controller", runtime / "desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(runtime / "manifest.json")
    require(replay.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "pinned_python_required")
    runner = payload / manifest["engine"]["runner"]
    supervisor = replay.load_module("marker_supervisor", runner)
    cache = controller.verify_tokenizer_cache(payload)
    run = ROOT / "work/proofs/optimization/anthropic-cache-markers" / (mode + "-" + uuid.uuid4().hex)
    run.mkdir(parents=True)
    overlay = replay.prepare_fixture_runtime(run, payload)
    vendor = payload / "Lib/site-packages/headroom"
    sources = json.loads((runtime / "optimization-sources.json").read_bytes())
    inputs = [Path(__file__), Path(__file__).with_name("anthropic_usage_capture.py"), runner,
              Path(sys.executable), payload / "runtime-manifest.json", *overlay, *[runtime / path.name for path in overlay],
              *[vendor / name for name in set(sources) | {"proxy/helpers.py"}],
              *[path for path in cache.iterdir() if path.is_file()],
              *[ROOT / "bench/optimization" / name for name in ("anthropic_usage.py", "ccr_policy.py", "surfaces.py", "replay.py", "fixtures.py")]]
    before = replay.fingerprints(inputs)
    server = ThreadingHTTPServer(("127.0.0.1", 0), Fixture)
    server.capture_lock, server.captured = threading.Lock(), {}
    threading.Thread(target=server.serve_forever, daemon=True).start()
    rows, stopped, failure = [], None, None
    try:
        engine = probe["UsageEngine"](controller, supervisor, payload, run / "engine", server.server_port, PLAN)
        try:
            engine.start()
            exercise(engine, server, mode == "baseline", rows)
        finally:
            engine.close()
            stopped = engine.stopped
        require(len(rows) == sum(map(len, CASES.values())) and not server.captured, "fixture_coverage")
    except Exception as error:
        failure = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)) else type(error).__name__
    finally:
        server.shutdown()
        server.server_close()
    after = replay.fingerprints(inputs)
    network = replay.fixture_network_evidence(run)
    passed = failure is None and before == after and network["huggingface_hub_payload_bytes"] == 0
    proof = run / "result.json"
    replay.write_new(proof, {"passed": passed, "mode": mode, "failure": failure, "rows": rows,
                            "stop": stopped, "os": platform.platform(), "engine": manifest["engine"]["version"],
                            "fixture_network": network, "before": before, "after": after})
    print(json.dumps({"passed": passed, "failure": failure, "rows": len(rows), "proof": proof.relative_to(ROOT).as_posix()}))
    return not passed


if __name__ == "__main__":
    raise SystemExit(main())
