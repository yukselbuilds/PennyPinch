"""Compare an exact local JSONL query with full Read through the owned engine."""
import argparse
from dataclasses import replace
from http.server import ThreadingHTTPServer
import json
import os
from pathlib import Path
import platform
import re
import runpy
import site
import struct
import subprocess
import sys
import threading
import time
import uuid


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
replay = runpy.run_path(str(HERE.parent / "optimization" / "replay.py"))
load_module, encoded, require = (replay[name] for name in ("load_module", "encoded", "require"))
query = load_module("targeted_query_proof", HERE / "targeted_query.py")
PLAN = {"thresholds": {"startup_timeout_seconds": 120, "request_timeout_seconds": 30}}
TARGET = "row_0777"
READ_ARGS = {"resource": "owned_report"}
READ = {"type": "function", "name": "Read", "description": "Read the full owned report.",
        "parameters": {"type": "object", "properties": {"resource": {"type": "string", "enum": ["owned_report"]}},
                       "required": ["resource"], "additionalProperties": False}}
LOOKUP = {"type": "function", "name": "select_report_record", "description": "Find an exact record and count rows in an owned JSONL report.",
          "parameters": {"type": "object", "properties": {"archive_id": {"type": "string", "pattern": "^[a-f0-9]{32}$"},
                                                         "record_id": {"type": "string", "pattern": "^[a-zA-Z0-9_-]{1,64}$"}},
                         "required": ["archive_id", "record_id"], "additionalProperties": False}}
PRODUCER = """import struct, sys
data = sys.stdin.buffer
size, exit_code = struct.unpack('>II', data.read(8))
sys.stdout.buffer.write(data.read(size))
sys.stderr.buffer.write(data.read())
sys.exit(exit_code)
"""


def report_rows():
    return [f'{{"id":"row_{i:04}","int":18446744073709551617,"decimal":0.1234567890123456789012345,"detail":"Tamam — λ"}}'.encode("utf-8")
            + (b"" if i == 999 else b"\r\n" if i % 2 else b"\n") for i in range(1000)]


def capture(engine, tool, call_id, stdout, stderr=b"", exit_code=0):
    value = struct.pack(">II", len(stdout), exit_code) + stdout + stderr
    child = subprocess.run([sys.executable, "-I", "-B", "-c", PRODUCER], input=value, capture_output=True,
                           cwd=engine.directory / "workspace", env=engine.environment,
                           creationflags=subprocess.CREATE_NO_WINDOW, timeout=10)
    require((child.stdout, child.stderr, child.returncode) == (stdout, stderr, exit_code), "captured_channels_changed")
    return query.ToolOutput(tool, call_id, child.stdout, child.stderr, child.returncode)


def read_result(output, stdout=None, **metadata):
    return {"stdout": (output.stdout if stdout is None else stdout).decode("utf-8"),
            "stderr": output.stderr.decode("utf-8"), "exit_code": output.exit_code,
            "source_tool": output.tool, "source_call_id": output.call_id, **metadata}


def request_body(task, archive_id=None):
    items = [{"type": "message", "role": "user", "content": [{"type": "input_text", "text": task}]}]
    tools = [READ]
    if archive_id is not None:
        items.insert(0, {"type": "message", "role": "developer", "content": [
            {"type": "input_text", "text": "Authorized report for this session. Archive ID " + archive_id}]})
        tools.append(LOOKUP)
    return {"model": "o4-mini", "reasoning": {"effort": "high"}, "stream": True, "input": items, "tools": tools}


def append_result(body, name, call_id, arguments, value):
    return {**body, "input": [*body["input"],
                             {"type": "function_call", "name": name, "call_id": call_id, "arguments": encoded(arguments).decode()},
                             {"type": "function_call_output", "call_id": call_id, "output": encoded(value).decode()}]}


def forward(engine, server, name, payload, previous=None):
    status = replay["request"](engine.port, payload, name, 30)["status"]
    with server.capture_lock:
        observed = server.captured.pop(name, [])
    require(status == 200 and len(observed) == 1 and observed[0]["auth_preserved"], "single_owned_upstream_required")
    body = observed[0]["body"]
    replay["validate_body"](payload, body, previous, True)
    require(body["tools"] == payload["tools"] and body["input"] == payload["input"], "forwarded_query_or_full_read_changed")
    return body


def expect_error(callback, error_type, code):
    try:
        callback()
    except error_type as error:
        require(error.args == (code,), "wrong_failure_contract")
    else:
        require(False, "failure_contract_missing")


def boundary_checks(archive, owner, original, args, row):
    foreign = object()
    for denied in (lambda: archive.select(foreign, args),
                   lambda: archive.read_original(foreign, args["archive_id"]),
                   lambda: query.Archive().select(owner, args),
                   lambda: archive.select(owner, {**args, "archive_id": "0" * 32})):
        expect_error(denied, LookupError, "archive_unavailable")
    for change in ({"owner": "other"}, {"path": "outside-report.jsonl"}, {"record_id": "../other"}):
        expect_error(lambda: archive.select(owner, {**args, **change}), ValueError, "invalid_query")
    empty_report = b'{"id":"different"}\r\n'
    cases = {
        "missing_record": (empty_report, "record_missing"),
        "duplicate_record": (row + row, "ambiguous_jsonl"),
        "duplicate_json_key": (row + b'{"id":"x","id":"y"}\n', "invalid_jsonl"),
        "number_as_id": (b'{"id":1}\n', "invalid_jsonl"),
        "nonfinite_number": (b'{"id":"bad","value":NaN}\n', "invalid_jsonl"),
        "binary_tail": (row + b"\xff\xfe", "invalid_jsonl"),
        "row_limit": (row + b'{"id":"wide","text":"' + b"a" * query.MAX_ROW_BYTES + b'"}\n', "query_capacity"),
        "record_limit": (row + b"".join(f'{{"id":"i{i}"}}\n'.encode() for i in range(query.MAX_ROWS)), "query_capacity"),
    }
    for label, (value, reason) in cases.items():
        case = replace(original, stdout=value)
        handle = archive.keep(owner, case)
        selection = archive.select(owner, {**args, "archive_id": handle})
        require(selection == query.Selection(reason) and archive.read_original(owner, handle) == case, label)

    too_large = replace(original, stdout=b"x" * (query.MAX_CAPTURE_BYTES + 1))
    expect_error(lambda: archive.keep(owner, too_large), ValueError, "archive_capacity")
    byte_archive = query.Archive()
    maximum = replace(original, stdout=b"x" * query.MAX_CAPTURE_BYTES)
    entries = [byte_archive.keep(owner, maximum) for _ in range(query.MAX_ARCHIVE_BYTES // query.MAX_CAPTURE_BYTES)]
    expect_error(lambda: byte_archive.keep(owner, original), ValueError, "archive_capacity")
    require(all(byte_archive.read_original(owner, ref) == maximum for ref in entries), "archive_capacity_evicted_data")
    entry_archive = query.Archive()
    empty = replace(original, stdout=b"")
    for _ in range(query.MAX_CAPTURES):
        entry_archive.keep(owner, empty)
    expect_error(lambda: entry_archive.keep(owner, empty), ValueError, "archive_capacity")
    return {**{label: reason for label, (_, reason) in cases.items()}, "scope_and_generation": "pass",
            "arguments": "pass", "archive_byte_and_entry_limits": "pass"}


def protected_outputs(engine, server, archive, owner, args, record, raw_report):
    expected = [
        ("failed_stdout", query.REPORT_TOOL, b'{"id":"row_0777","message":"FAIL expected=-17.2500"}\r\n', b"", 1),
        ("successful_stderr", query.REPORT_TOOL, record, "Uyarı λ CRLF\r\n".encode(), 0),
        ("source_read", "Read", "# Türkçe kaynak satırı\r\nanswer = 18446744073709551617\r\n".encode() * 120, b"", 0),
        ("numeric_read", "Read", raw_report, b"", 0),
    ]
    results = []
    for label, tool, stdout, stderr, code in expected:
        call_id = "call-" + label
        output = capture(engine, tool, call_id, stdout, stderr, code)
        handle = archive.keep(owner, output)
        require(archive.select(owner, {**args, "archive_id": handle}) == query.Selection("full_output_required"),
                "protected_output_selected")
        original = archive.read_original(owner, handle)
        require(original == output, "original_metadata_changed")
        schema = {**READ, "name": tool}
        body = {**request_body("Use the authorized output in full."), "tools": [schema]}
        body = append_result(body, tool, call_id, READ_ARGS, read_result(original))
        forwarded = forward(engine, server, label + "-t0", body)
        wire = json.loads(forwarded["input"][-1]["output"])
        require(wire == read_result(output) and forwarded["input"][-1]["call_id"] == call_id, "protected_envelope_changed")
        results.append({"case": label, "exit_code": wire["exit_code"], "stdout_bytes": len(stdout),
                        "stderr_bytes": len(stderr), "raw_exact": True, "call_id_preserved": True})
    return results


def exercise(engine, server, count):
    archive, owner = query.Archive(), object()
    rows = report_rows()
    raw = b"".join(rows)
    original = capture(engine, query.REPORT_TOOL, "call-owned-report", raw)
    handle = archive.keep(owner, original)
    args = {"archive_id": handle, "record_id": TARGET}
    started = time.perf_counter()
    selected = archive.select(owner, args)
    selection_ms = (time.perf_counter() - started) * 1000
    require(selected == query.Selection("selected", rows[777], len(rows)), "target_record_or_count_changed")
    require(rows[777].strip() not in encoded(args), "query_argument_contains_expected_row")
    require(archive.read_original(owner, handle) == original, "raw_archive_round_trip_changed")

    task = f"Return the exact report row for {TARGET} and the total record count. Preserve every digit."
    full_open = request_body(task)
    full_next = append_result(full_open, "Read", "call-read", READ_ARGS, read_result(original))
    select_open = request_body(task, handle)
    select_next = append_result(select_open, LOOKUP["name"], "call-select", args,
                                read_result(original, stdout=selected.row, archive_id=handle, record_count=selected.count))
    full_again = append_result(select_next, "Read", "call-read-after-select", READ_ARGS,
                               read_result(archive.read_original(owner, handle)))

    full_begin = forward(engine, server, "full-t0", full_open)
    full_wire = forward(engine, server, "full-t1", full_next, full_begin)
    select_begin = forward(engine, server, "targeted-t0", select_open)
    select_wire = forward(engine, server, "targeted-t1", select_next, select_begin)
    again_wire = forward(engine, server, "targeted-t2", full_again, select_wire)
    full_value = json.loads(full_wire["input"][-1]["output"])
    select_value = json.loads(select_wire["input"][-1]["output"])
    require(full_value["stdout"].encode().splitlines(keepends=True)[777] == select_value["stdout"].encode()
            and select_value["record_count"] == len(full_value["stdout"].splitlines()) == 1000,
            "paired_wire_oracle_failed")

    def sum_requests(values):
        return sum(count(encoded(body).decode()) for body in values)

    full_tokens = sum_requests((full_begin, full_wire))
    select_tokens = sum_requests((select_begin, select_wire))
    including_readback = select_tokens + sum_requests((again_wire,))
    require(select_tokens < full_tokens < including_readback, "full_wire_benefit_bounds")
    return {"report_bytes": len(raw), "query_ms": selection_ms,
            "full_input_json_tokens": full_tokens, "targeted_input_json_tokens": select_tokens,
            "targeted_then_full_input_json_tokens": including_readback,
            "full_tool_output_json_tokens": count(full_wire["input"][-1]["output"]),
            "targeted_tool_output_json_tokens": count(select_wire["input"][-1]["output"]),
            "full_arguments_tokens": count(encoded(READ_ARGS).decode()),
            "targeted_arguments_tokens": count(encoded(args).decode()),
            "full_tool_schema_tokens": count(encoded(full_open["tools"]).decode()),
            "targeted_tool_schema_tokens": count(encoded(select_open["tools"]).decode()),
            "selected_row_and_count_exact": True,
            "protected": protected_outputs(engine, server, archive, owner, args, rows[777], raw),
            "boundaries": boundary_checks(archive, owner, original, args, rows[777])}


def main():
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("--run", action="store_true", required=True)
    args.parse_args()
    require(os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_workspace")
    runtime = ROOT / "runtime"
    controller = load_module("targeted_query_controller", runtime / "desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(runtime / "manifest.json")
    require(replay["sha"](Path(sys.executable)) == manifest["engine"]["python_sha256"], "pinned_python_required")
    runner = payload / manifest["engine"]["runner"]
    supervisor = load_module("targeted_query_supervisor", runner)
    cache = controller.verify_tokenizer_cache(payload)
    os.environ["TIKTOKEN_CACHE_DIR"] = str(cache)
    packages = payload / "Lib/site-packages"
    site.addsitedir(str(packages))
    import tiktoken

    tokenizer = tiktoken.get_encoding("o200k_base")
    count = lambda value: len(tokenizer.encode(value, disallowed_special=()))
    run = ROOT / "work/proofs/optimization/targeted-query" / uuid.uuid4().hex
    run.mkdir(parents=True)
    staged_runtime = replay["prepare_fixture_runtime"](run, payload)
    expected_sources = json.loads((runtime / "optimization-sources.json").read_bytes())
    inputs = [Path(__file__), HERE / "targeted_query.py", HERE.parent / "optimization/replay.py",
              HERE.parent / "optimization/fixtures.py", Path(sys.executable), runner,
              payload / "runtime-manifest.json", payload / "python313.dll", *staged_runtime,
              *[runtime / path.name for path in staged_runtime],
              *[packages / "headroom" / name for name in expected_sources],
              *[path for path in cache.iterdir() if path.is_file()], *list((packages / "tiktoken").glob("*.pyd"))]
    before = replay["fingerprints"](inputs)
    server = ThreadingHTTPServer(("127.0.0.1", 0), replay["FixtureHandler"])
    server.capture_lock, server.captured = threading.Lock(), {}
    threading.Thread(target=server.serve_forever, daemon=True).start()
    runs, stops, failure = [], [], None
    try:
        for number in range(3):
            engine = replay["Engine"](controller, supervisor, payload, run / f"engine-{number}", server.server_port, PLAN)
            try:
                engine.start()
                runs.append(exercise(engine, server, count))
                require(not server.captured, "unattributed_upstream_request")
            finally:
                engine.close()
                stops.append(engine.stopped)
    except Exception as error:
        failure = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)) else type(error).__name__
    finally:
        server.shutdown()
        server.server_close()
    after = replay["fingerprints"](inputs)
    network = replay["fixture_network_evidence"](run)
    passed = failure is None and before == after and network["huggingface_hub_payload_bytes"] == 0
    proof = run / "result.json"
    replay["write_new"](proof, {"passed": passed, "failure": failure, "runs": runs, "stops": stops,
                                "os": platform.platform(), "engine": manifest["engine"]["version"],
                                "fixture_network": network, "before": before, "after": after})
    print(json.dumps({"passed": passed, "failure": failure, "repeats": len(runs), "proof": str(proof.relative_to(ROOT))}))
    return not passed


if __name__ == "__main__":
    raise SystemExit(main())
