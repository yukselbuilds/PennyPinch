"""Compare pinned Anthropic CCR injection policies with an owned loopback fixture."""

import argparse
from copy import deepcopy
import hashlib
from http.server import ThreadingHTTPServer
import json
import os
from pathlib import Path
import platform
import re
import runpy
import site
import sys
import threading
import uuid


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
PLAN = HERE / "ccr-policy-plan.json"
surface = runpy.run_path(str(HERE / "surfaces.py"))
replay = surface["replay"]
CCR_HASH = re.compile(r"(?:Retrieve more: hash=|<<ccr:)([a-fA-F0-9]{12,24})(?=[^a-fA-F0-9]|$)")
LOG_CALL = "call-repetitive_logs-7"


class CcrFixtureHandler(surface["SurfaceHandler"]):
    def respond(self, body, name):
        if body["stream"]:
            return super().respond(body, name)
        data = replay.encoded({
            "id": "msg-fixture", "type": "message", "role": "assistant", "model": body["model"],
            "content": [{"type": "text", "text": "FIXTURE_OK"}],
            "usage": {"input_tokens": 1, "output_tokens": 1},
            "stop_reason": "end_turn", "stop_sequence": None,
        })
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(data)


def continue_after_fixture_reply(body, tool_exchange):
    body = deepcopy(body)
    body["messages"].extend([
        {"role": "assistant", "content": [{"type": "text", "text": "FIXTURE_OK"}]},
        {"role": "user", "content": [{"type": "text", "text": "Inspect the next synthetic fixture result."}]},
        *deepcopy(tool_exchange),
    ])
    return body


def workloads(plan):
    name, model = plan["surface"], plan["fixture_model"]
    snapshots = surface["conversation"](name, "short_json", model, plan["short_turns"])
    short = [snapshots[0]]
    # The cache tracker includes the completed SSE reply in its next-turn prefix.
    for snapshot in snapshots[1:]:
        short.append(continue_after_fixture_reply(short[-1], snapshot["messages"][-2:]))
    read_tool = surface["conversation"](name, "numeric_read", model, 1)[0]["tools"][0]
    long = deepcopy(short)
    # Use substantial stable input before the cache breakpoint, without any real project content.
    context = "\n".join(f"Unit {index:04d} synthetic baseline input." for index in range(160))
    for body in long:
        body["tools"].insert(0, deepcopy(read_tool))
        body["messages"][0]["content"][0]["text"] = context
    for case, tool, call_id in (
        ("repetitive_logs", "inspect_workspace", LOG_CALL),
        ("numeric_read", "Read", "call-numeric_read-8"),
    ):
        tool_exchange = [
            {"role": "assistant", "content": [{"type": "tool_use", "id": call_id, "name": tool, "input": {}}]},
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": call_id,
                                         "content": replay.fixtures.output(case, len(long) + 4)}]},
        ]
        long.append(continue_after_fixture_reply(long[-1], tool_exchange))
    return {"short": short, "long": long, "fresh": short[:1]}


def log_block(body):
    return next(
        block for message in body["messages"] for block in message["content"]
        if block.get("tool_use_id") == LOG_CALL
    )


def digest(body):
    return hashlib.sha256(replay.encoded(body)).hexdigest()


def exercise(engine, server, plan, mode, sessions, ccr_tool, count, rows):
    name = plan["surface"]
    timeout = plan["thresholds"]["request_timeout_seconds"]
    for session, requests in sessions.items():
        previous = None
        for turn, original in enumerate(requests, 1):
            key = f"{session}-t{turn}"
            timing = surface["http_request"](engine.port, original, key, name, timeout)
            with server.capture_lock:
                captures = server.captured.pop(key, [])
            replay.require(len(captures) == 1 and captures[0]["auth_preserved"], "fixture_upstream_identity")
            forwarded = captures[0]["body"]
            has_log = session == "long" and turn > plan["short_turns"]
            expected = deepcopy(original)
            if mode == "eager" or has_log:
                expected["tools"].append(ccr_tool)
            expected["tools"].sort(key=lambda item: item["name"])

            restored = False
            if has_log:
                raw = log_block(original)["content"]
                compressed = log_block(forwarded)["content"]
                marker = CCR_HASH.search(compressed)
                replay.require(marker and count(compressed) < count(raw), "fixture_log_not_compressed")
                replay.retrieval(engine.port, marker.group(1), raw, timeout)
                restored = True
                log_block(expected)["content"] = compressed
                # CCR can require a buffered upstream reply. The client still receives SSE.
                expected["stream"] = False
            replay.require(forwarded == expected, "fixture_body_changed_outside_expected_fields")
            if previous is not None:
                replay.require(forwarded["messages"][:len(previous["messages"])] == previous["messages"],
                               "fixture_message_prefix_changed")
            tools_changed = previous is not None and previous["tools"] != forwarded["tools"]
            replay.require(tools_changed == (mode == "lazy" and session == "long"
                                              and turn == plan["short_turns"] + 1),
                           "fixture_tool_schema_transition")
            previous = forwarded
            rows.append({"session": session, "turn": turn, "original_tokens": count(original),
                         "forwarded_tokens": count(forwarded), "input_sha256": digest(original),
                         "forwarded_sha256": digest(forwarded), "tools_sha256": digest(forwarded["tools"]),
                         "tools_changed": tools_changed, "ccr_retrieval_verified": restored,
                         "upstream_stream": forwarded["stream"], "fixture_timing": timing})


def run_experiment(run, plan, sessions, payload, controller, supervisor, ccr_tool, count, inputs):
    arms = []
    for pair, modes in enumerate(plan["pairs"], 1):
        for mode in modes:
            directory = run / f"pair-{pair}-{mode}"
            directory.mkdir()
            inputs.extend(replay.prepare_fixture_runtime(directory, payload, ccr_injection=mode))
            arms.append({"pair": pair, "mode": mode, "rows": []})
    before = replay.fingerprints(inputs)
    server = ThreadingHTTPServer(("127.0.0.1", 0), CcrFixtureHandler)
    server.capture_lock, server.captured = threading.Lock(), {}
    threading.Thread(target=server.serve_forever, daemon=True).start()
    failure = None
    try:
        for arm in arms:
            directory = run / f"pair-{arm['pair']}-{arm['mode']}"
            engine = replay.Engine(controller, supervisor, payload, directory / "engine", server.server_port, plan)
            engine.environment["HEADROOM_DESKTOP_PROTECTED_TOOLS"] = json.dumps(plan["protected_tools"])
            try:
                arm["startup"] = engine.start()
                exercise(engine, server, plan, arm["mode"], sessions, ccr_tool, count, arm["rows"])
            finally:
                engine.close()
                arm["shutdown"] = engine.stopped
                arm["fixture_network"] = replay.fixture_network_evidence(directory)
            replay.require(arm["fixture_network"]["huggingface_hub_payload_bytes"] == 0, "fixture_model_download")
        for pair in range(len(plan["pairs"])):
            eager, lazy = sorted(arms[2 * pair:2 * pair + 2], key=lambda arm: arm["mode"])
            for baseline, candidate in zip(eager["rows"], lazy["rows"], strict=True):
                replay.require(baseline["input_sha256"] == candidate["input_sha256"], "fixture_unpaired_input")
                if candidate["session"] in ("short", "fresh"):
                    replay.require(candidate["forwarded_tokens"] == candidate["original_tokens"]
                                   and baseline["forwarded_tokens"] > candidate["forwarded_tokens"],
                                   "fixture_short_policy_benefit")
    except Exception as error:
        failure = type(error).__name__
        if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)):
            failure = str(error)
    finally:
        server.shutdown()
        server.server_close()
    after = replay.fingerprints(inputs)
    return {"passed": failure is None and before == after, "failure": failure, "plan": plan,
            "arms": arms, "before": before, "after": after, "os": platform.platform(),
            "python": platform.python_version(), "provider_usage": "not_tested", "task_quality": "not_tested"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true", required=True)
    parser.parse_args()
    replay.require(ROOT == Path.cwd().resolve() == Path(r"D:\Projeler\Headroom"), "fixture_workspace")
    plan = json.loads(PLAN.read_text(encoding="utf-8"))
    manifest_path = ROOT / "runtime/manifest.json"
    controller = replay.load_module("ccr_policy_controller", ROOT / "runtime/desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(manifest_path)
    python_path = Path(sys.executable)
    replay.require(replay.sha(python_path) == manifest["engine"]["python_sha256"], "fixture_python_pin")
    runner = payload / manifest["engine"]["runner"]
    supervisor = replay.load_module("ccr_policy_supervisor", runner)
    cache = controller.verify_tokenizer_cache(payload)
    os.environ["TIKTOKEN_CACHE_DIR"] = str(cache)
    site_packages = payload / "Lib/site-packages"
    site.addsitedir(str(site_packages))
    vendor = site_packages / "headroom"
    definition = vendor / "ccr/tool_injection.py"
    ccr_tool = replay.load_module("ccr_policy_tool_definition", definition).create_ccr_tool_definition("anthropic")
    import tiktoken

    tokenizer = tiktoken.get_encoding(plan["token_encoding"])

    def count(value):
        return len(tokenizer.encode(replay.encoded(value).decode("utf-8"), disallowed_special=()))

    overlay_sources = ["desktop_controller.py", "desktop_helper.py", "cache_control_shim.py",
                       "optimization_control.py", "optimization-sources.json"]
    inputs = [
        Path(__file__), PLAN, manifest_path, python_path, runner,
        *[ROOT / "runtime" / name for name in overlay_sources],
        *[HERE / name for name in ("surfaces.py", "replay.py", "fixtures.py")],
        definition, vendor / "proxy/handlers/anthropic.py", vendor / "proxy/handlers/streaming.py",
        vendor / "proxy/helpers.py", vendor / "transforms/content_router.py",
        vendor / "proxy/tool_injection_policy.py", vendor / "proxy/tool_injection_config.py",
        vendor / "cache/prefix_tracker.py", *[path for path in cache.iterdir() if path.is_file()],
        *[path for path in (site_packages / "tiktoken").iterdir() if path.suffix in (".py", ".pyd")],
    ]
    run = ROOT / "work/proofs/optimization/ccr-policy" / str(uuid.uuid4())
    run.mkdir(parents=True)
    result = run_experiment(run, plan, workloads(plan), payload, controller, supervisor, ccr_tool, count, inputs)
    proof = run / "result.json"
    replay.write_new(proof, result)
    print(json.dumps({"passed": result["passed"], "failure": result["failure"],
                      "requests": sum(len(arm["rows"]) for arm in result["arms"]),
                      "proof": proof.relative_to(ROOT).as_posix()}))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
