"""Paired local probe of the bundled cross-turn dedup switch."""
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
surface = runpy.run_path(str(HERE / "surfaces.py"))
base = surface["replay"]
ROOT = base.ROOT
require = base.require
SURFACES = ("responses", "chat_http_sse", "anthropic_http_sse")
PAIRS = (("default", "disabled"), ("disabled", "default"))
PLAN = {"thresholds": {"startup_timeout_seconds": 120, "request_timeout_seconds": 30}}
POINTER = re.compile(r"\[↑([1-9]\d{0,4})L same as msg (\d{1,4})(?: ([+-]\d{1,5})L)?: (.+)\]")
NUMBER_PREFIX = re.compile(r"^[1-9]\d{0,17}(?::|\t)")
CCR = re.compile(r"(?:Retrieve (?:more|original): hash=|<<ccr:)([a-fA-F0-9]{12,24})(?=[^a-fA-F0-9]|$)")
FAILURE = "stderr: FIXTURE_FAILURE expected=-17.2500 actual=0.000000000000001"


class Fixture(surface["SurfaceHandler"]):
    def do_POST(self):
        if self.path == "/v1/responses":
            return base.FixtureHandler.do_POST(self)
        return super().do_POST()


def workloads():
    notes = "\n".join(f"Key {n}: owned diagnostic sample with distinct fixture value {n * 17}." for n in range(8))
    log = base.fixtures.output("repetitive_logs", 1)
    code = base.fixtures.output("source_read", 1)
    numbers = "[\n" + ",\n".join(f'  {{"row":{i},"big":18446744073709551617,"fraction":0.1234567890123456789012345}}'
                                  for i in range(8)) + "\n]\n"
    group = lambda tag: "\n".join([f"Diagnostics section record {tag}", *[f"{tag} field {n}: fixture detail number {n + 10}" for n in range(5)]])
    return {
        "exact": ("inspect_workspace", (notes, notes, None)),
        "long_log": ("inspect_workspace", (log, log)),
        "changed_failure": ("inspect_workspace", ("exit_code=0\n" + notes, "exit_code=23\n" + notes + "\n" + FAILURE)),
        "source": ("Read", (code, code)),
        "numeric": ("Read", (numbers, numbers)),
        "ambiguous": ("inspect_workspace", (group("alpha") + "\n" + group("beta"), group("beta"))),
        "other_session": ("inspect_workspace", (notes,)),
    }


def turns(wire, case, tool, values):
    responses = wire == "responses"
    anthropic = wire == "anthropic_http_sse"
    key = "input" if responses else "messages"
    args = {"fixture": case}
    schema = {"type": "object", "properties": {"fixture": {"type": "string"}}, "required": ["fixture"]}
    spec = {"name": tool, "description": "Read owned fixture output", "parameters": schema}
    message = lambda role, text: {"role": role, "content": [{"type": "text", "text": text}] if anthropic else text}
    body = {"model": "claude-sonnet-4-20250514" if anthropic else "o4-mini", "stream": True,
            key: [message("user", f"Read the synthetic {case} fixture.")]}
    if responses:
        body.update(tools=[{"type": "function", **spec}], reasoning={"effort": "high"})
    elif anthropic:
        body.update(tools=[{"name": tool, "description": spec["description"], "input_schema": schema}],
                    max_tokens=2048, thinking={"type": "enabled", "budget_tokens": 1024})
    else:
        body.update(tools=[{"type": "function", "function": spec}], reasoning_effort="high")
    for number, value in enumerate(values, 1):
        identity = f"call-{case}-{number}"
        if value is None:
            body[key].extend([message("assistant", "FIXTURE_OK"), message("user", "Confirm without another tool call.")])
        elif responses:
            body[key].extend([{"type": "function_call", "call_id": identity, "name": tool, "arguments": base.encoded(args).decode()},
                              {"type": "function_call_output", "call_id": identity, "output": value}])
        elif anthropic:
            result = {"type": "tool_result", "tool_use_id": identity, "content": value}
            if number == 2 and case == "changed_failure":
                result["is_error"] = True
            body[key].extend([{"role": "assistant", "content": [{"type": "tool_use", "id": identity, "name": tool, "input": args}]},
                              {"role": "user", "content": [result]}])
        else:
            body[key].extend([{"role": "assistant", "content": None, "tool_calls": [{"id": identity, "type": "function", "function": {
                "name": tool, "arguments": base.encoded(args).decode()}}]}, {"role": "tool", "tool_call_id": identity, "content": value}])
        yield deepcopy(body), identity if value is not None else None


def output_items(body, wire):
    if wire == "responses":
        return [(i, item["call_id"], item["output"]) for i, item in enumerate(body["input"]) if item.get("type") == "function_call_output"]
    if wire == "chat_http_sse":
        return [(i, item["tool_call_id"], item["content"]) for i, item in enumerate(body["messages"]) if item["role"] == "tool"]
    return [(i, b["tool_use_id"], b["content"]) for i, item in enumerate(body["messages"])
            for b in item["content"] if b["type"] == "tool_result"]


def pointer_anchor(lines):
    anchor = next((NUMBER_PREFIX.sub("", line, count=1).strip() for line in lines if line.strip()), "")
    return repr(anchor if len(anchor) <= 20 else anchor[:17] + "...")


def expand(text, ordinal, earlier):
    parts, defined = [], 0
    for line in text.split("\n"):
        match = POINTER.fullmatch(line)
        if not match:
            parts.append(line)
            continue
        length, reference = map(int, match.groups()[:2])
        if match[3]:
            return None, "renumbered_reference"
        if reference >= ordinal:
            return None, "reference_is_not_earlier"
        candidates = set()
        for original in earlier.get(reference, []):
            lines = original.split("\n")
            for offset in range(len(lines) - length + 1):
                span = lines[offset:offset + length]
                if not any(POINTER.fullmatch(part) for part in span) and pointer_anchor(span) == match[4]:
                    candidates.add("\n".join(span))
        if len(candidates) != 1:
            return None, "ambiguous_reference" if candidates else "missing_reference"
        parts.append(candidates.pop())
        defined += 1
    return "\n".join(parts), "expanded" if defined else "direct"


def proof_for_outputs(engine, original, forwarded, wire, count):
    wanted = {identity: text for _, identity, text in output_items(original, wire)}
    earlier, checks = {}, {}
    for index, identity, text in output_items(forwarded, wire):
        require(isinstance(text, str), "non_text_output")
        pointers = sum(bool(POINTER.fullmatch(line)) for line in text.split("\n"))
        recovered, anchor = expand(text, index, earlier)
        earlier.setdefault(index, []).append(text)
        markers = set(CCR.findall(recovered or ""))
        exact = recovered == wanted[identity]
        retrieved = False
        if not exact and len(markers) == 1:
            try:
                base.retrieval(engine.port, next(iter(markers)), wanted[identity], PLAN["thresholds"]["request_timeout_seconds"])
                exact = retrieved = True
            except (ValueError, AssertionError):
                pass
        checks[identity] = {"tokens": count(text), "pointers": pointers, "anchor": anchor,
                            "exact": exact, "raw_visible": text == wanted[identity], "ccr_exact": retrieved,
                            "failure_visible": FAILURE in text if FAILURE in wanted[identity] else None,
                            "exit_visible": next((line in text.split("\n") for line in wanted[identity].split("\n")
                                                  if line.startswith("exit_code=")), None),
                            "text_sha256": hashlib.sha256(text.encode("utf-8")).hexdigest()}
    return checks


def engine_dedupe(engine):
    # Use the constructed router's startup field. The requested env value alone is not evidence.
    logs = [p for p in (engine.directory / "workspace/logs").glob("*.log") if p.is_file()]
    values = {m for p in logs for m in re.findall(r"Savings profile: coding .*? dedupe=(True|False)\b", p.read_text(encoding="utf-8", errors="replace"))}
    require(len(values) == 1, "effective_dedup_missing")
    return values == {"True"}


def exercise(engine, server, pair, arm, count):
    timeout = PLAN["thresholds"]["request_timeout_seconds"]
    for wire in SURFACES:
        for case, (tool, values) in workloads().items():
            previous = None
            for number, (body, new_call) in enumerate(turns(wire, case, tool, values), 1):
                key = f"{pair}-{arm}-{wire}-{case}-t{number}"
                require(len(base.encoded(body)) < 1024 * 1024, "fixture_size")
                outcome = (base.request(engine.port, body, key, timeout) if wire == "responses" else
                           surface["http_request"](engine.port, body, key, wire, timeout))
                with server.capture_lock:
                    captured = server.captured.pop(key, [])
                require(outcome["status"] == 200 and len(captured) == 1 and captured[0]["auth_preserved"], "owned_upstream_response")
                candidate = captured[0]["body"]
                if wire == "responses":
                    base.validate_body(body, candidate, None, False)
                else:
                    surface["validate"](body, candidate, None, wire, False)
                items = "input" if wire == "responses" else "messages"
                prefix = previous is None or (candidate[items][:len(previous[items])] == previous[items] and candidate["tools"] == previous["tools"])
                checks = proof_for_outputs(engine, body, candidate, wire, count)
                yield {"pair": pair, "arm": arm, "surface": wire, "case": case, "turn": number,
                       "request_input_tokens": count(base.encoded(body).decode()), "upstream_request_tokens": count(base.encoded(candidate).decode()),
                       "prefix_and_tools_preserved": prefix, "new_call": new_call, "checks": checks, "elapsed_ms": outcome["total_ms"]}
                previous = candidate


def compare(rows):
    grouped = {}
    for row in rows:
        grouped.setdefault((row["pair"], row["surface"], row["case"], row["turn"]), {})[row["arm"]] = row
    result = []
    for (pair, wire, case, turn), group in sorted(grouped.items()):
        dedup, disabled = group["default"], group["disabled"]
        require(disabled["request_input_tokens"] == dedup["request_input_tokens"] and disabled["new_call"] == dedup["new_call"], "paired_input_mismatch")
        old, new = disabled["upstream_request_tokens"], dedup["upstream_request_tokens"]
        new_call = dedup["new_call"]
        before, after = disabled["checks"].get(new_call), dedup["checks"].get(new_call)
        risks = []
        if not dedup["prefix_and_tools_preserved"]:
            risks.append("prefix_changed")
        if any(not check["exact"] for check in dedup["checks"].values()):
            risks.append("exact_recovery_unproven")
        if case in ("source", "numeric") and after and not after["raw_visible"]:
            risks.append("protected_output_changed")
        if case in ("changed_failure", "ambiguous", "other_session") and after and after["pointers"]:
            risks.append("fold_outside_exact_repeat_scope")
        if after and after["failure_visible"] is False:
            risks.append("failure_not_visible")
        if after and after["exit_visible"] is False:
            risks.append("exit_not_visible")
        result.append({"pair": pair, "surface": wire, "case": case, "turn": turn, "disabled": old, "default": new,
                       "saved_tokens": old - new, "saved_percent": 100 * (old - new) / old,
                       "new_output_tokens": [before["tokens"], after["tokens"]] if after else None,
                       "disabled_exact": all(check["exact"] for check in disabled["checks"].values()), "risks": risks})
    return result


def main():
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("--run", action="store_true", required=True)
    args.parse_args()
    require(Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom") and os.name == "nt", "canonical_windows_workspace")
    controller = base.load_module("dedup_controller", ROOT / "runtime/desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(ROOT / "runtime/manifest.json")
    require(base.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "pinned_python_required")
    supervisor_path = payload / manifest["engine"]["runner"]
    supervisor = base.load_module("dedup_supervisor", supervisor_path)
    cache = controller.verify_tokenizer_cache(payload)
    os.environ["TIKTOKEN_CACHE_DIR"] = str(cache)
    site.addsitedir(str(payload / "Lib/site-packages"))
    import tiktoken

    tokenizer = tiktoken.get_encoding("o200k_base")
    count = lambda text: len(tokenizer.encode(text, disallowed_special=()))
    run = ROOT / "work/proofs/optimization/duplicate-tool-results" / str(uuid.uuid4())
    run.mkdir(parents=True)
    vendor = payload / "Lib/site-packages/headroom"
    mapped = json.loads((ROOT / "runtime/optimization-sources.json").read_bytes())
    paths = [Path(__file__), HERE / "replay.py", HERE / "surfaces.py", HERE / "fixtures.py", Path(sys.executable), supervisor_path,
             payload / "runtime-manifest.json", *[p for p in cache.iterdir() if p.is_file()],
             *list((payload / "Lib/site-packages/tiktoken").glob("*.pyd")),
             *[vendor / name for name in set(mapped) | {"agent_savings.py", "config.py", "transforms/content_router.py", "transforms/cross_turn_dedup.py"}]]
    profiles = {}
    for pair, order in enumerate(PAIRS, 1):
        for arm in order:
            directory = run / f"pair-{pair}-{arm}"
            directory.mkdir()
            overlay = base.prepare_fixture_runtime(directory, payload)
            paths += [*overlay, *[ROOT / "runtime" / path.name for path in overlay]]
            if arm == "disabled":
                helper = directory / "fixture-runtime/desktop_helper.py"
                data = helper.read_bytes()
                anchor = b'    "HF_HUB_OFFLINE": "1",'
                require(data.count(anchor) == 1 and b'"HEADROOM_DEDUPE"' not in data, "dedup_fixture_scope")
                newline = b'\r\n' if b'\r\n' in data else b'\n'
                helper.write_bytes(data.replace(anchor, b'    "HEADROOM_DEDUPE": "0",' + newline + anchor, 1))
            profiles[pair, arm] = directory
    before = base.fingerprints(paths)
    server = ThreadingHTTPServer(("127.0.0.1", 0), Fixture)
    server.capture_lock, server.captured = threading.Lock(), {}
    threading.Thread(target=server.serve_forever, daemon=True).start()
    rows, starts, stops, comparison = [], [], [], []
    failure = None
    try:
        for (pair, arm), directory in profiles.items():
            engine = base.Engine(controller, supervisor, payload, directory / "engine", server.server_port, PLAN)
            try:
                ready = engine.start()
                effective = engine_dedupe(engine)
                require(effective == (arm == "default"), "effective_dedup_state")
                starts.append({"pair": pair, "arm": arm, "effective_dedup": effective, **ready})
                rows.extend(exercise(engine, server, pair, arm, count))
            finally:
                engine.close()
                stops.append(engine.stopped)
            print(json.dumps({"pair": pair, "arm": arm, "rows": len(rows), "stop": engine.stopped["shutdown"]}), flush=True)
        comparison = compare(rows)
        require(len(rows) == 168 and len(comparison) == 84, "pair_coverage")
    except Exception as exc:
        failure = str(exc) if isinstance(exc, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(exc)) else type(exc).__name__
    finally:
        server.shutdown()
        server.server_close()
    after = base.fingerprints(paths)
    passed = not failure and before == after
    report = run / "result.json"
    base.write_new(report, {"evaluation_complete": passed, "before": before, "after": after, "failure": failure,
                           "os": platform.platform(), "engine": manifest["engine"]["version"], "tokenizer": "o200k_base",
                           "starts": starts, "stops": stops, "fixture_network": [base.fixture_network_evidence(p) for p in profiles.values()],
                           "rows": rows, "comparison": comparison})
    print(json.dumps({"evaluation_complete": passed, "failure": failure, "rows": len(rows), "inputs": len(before), "proof": report.relative_to(ROOT).as_posix()}))
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
