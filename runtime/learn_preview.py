"""Summarize one user-selected tool transcript without invoking an analyzer."""

import hashlib
import json
import re
import sys
from collections import Counter
from pathlib import Path

MAX_BYTES = 16 * 1024 * 1024
MAX_RECORDS = 64_000
MAX_REPLAY_OUTPUT = 1024 * 1024
LOG_LINE = re.compile(r"^(?:INFO|DEBUG|TRACE|WARN|WARNING|ERROR|FATAL|SUMMARY)\b")
# Codex puts the process status before the output. A string printed by the
# command itself is not evidence of a failed process.
CODEX_EXIT = re.compile(
    r"\A(?:Chunk ID: [^\r\n]*\r?\n)?(?:Wall time: [^\r\n]*\r?\n)?"
    r"(?:Process exited with code |Exit code: )(-?[0-9]{1,12})(?:\r?\n|$)"
)

SUGGESTION = {
    "file_not_found": "path",
    "is_directory": "path",
    "module_not_found": "dependency",
    "command_not_found": "dependency",
    "permission_denied": "permission",
    "timeout": "timeout",
}
UNRELATED_ERRORS = {"no_matches", "user_rejected", "sibling_error"}


def object_or_empty(value):
    return value if isinstance(value, dict) else {}


def codex_events(record):
    if record.get("type") != "response_item":
        return
    item = object_or_empty(record.get("payload"))
    kind = item.get("type")
    if kind in ("function_call", "custom_tool_call"):
        args = item.get("arguments", item.get("input"))
        if isinstance(args, str) and kind == "function_call":
            try:
                args = json.loads(args)
            except (ValueError, RecursionError):
                args = None
        yield "call", item.get("call_id"), item.get("name"), args
    elif kind in ("function_call_output", "custom_tool_call_output"):
        output = item.get("output")
        framed = output
        if isinstance(output, str) and output.startswith("{"):
            try:
                framed = json.loads(output)
            except (ValueError, RecursionError):
                pass
        if isinstance(framed, dict):
            status = framed.get("exit_code")
            output = framed.get("output")
        else:
            match = CODEX_EXIT.match(output) if isinstance(output, str) else None
            status = int(match[1]) if match else None
        failed = status != 0 if type(status) is int else None
        yield "result", item.get("call_id"), failed, output
    elif kind == "message" and item.get("role") == "user":
        yield "boundary", None, None, None


def claude_events(record):
    kind = record.get("type")
    content = object_or_empty(record.get("message")).get("content")
    if kind not in ("assistant", "user"):
        return
    if not isinstance(content, list):
        if kind == "user":
            yield "boundary", None, None, None
        return
    for item in content:
        item = object_or_empty(item)
        block = item.get("type")
        if kind == "assistant" and block == "tool_use":
            yield "call", item.get("id"), item.get("name"), item.get("input")
        elif kind == "user" and block == "tool_result":
            result = item.get("content")
            if isinstance(result, list):
                result = "\n".join(
                    part["text"] for part in result
                    if isinstance(part, dict) and isinstance(part.get("text"), str)
                )
            yield "result", item.get("tool_use_id"), item.get("is_error", False), result
        elif kind == "user" and block == "text":
            yield "boundary", None, None, None


def read_runs(records, source):
    """Only compare consecutive explicit failures of an identical invocation."""
    from headroom.learn._shared import classify_error
    from headroom.learn.models import ToolCall

    events = codex_events if source == "codex" else claude_events
    pending, seen = {}, set()
    run, signature = [], None
    overlapping = False
    for line_number, record in records:
        scope = record.get("sessionId", "") if source == "claude_code" else ""
        if source == "claude_code" and (not isinstance(scope, str) or not scope):
            continue
        for kind, call_id, value, content in events(record):
            if kind == "boundary":
                if run:
                    yield run, False
                    run, signature = [], None
                continue
            if not isinstance(call_id, str) or not call_id:
                continue
            key = (scope, call_id)
            if key in seen:
                continue
            if kind == "call":
                if key in pending:
                    continue
                if pending and run:
                    yield run, False
                    run, signature = [], None
                # Concurrent commands have no reliable ordering for a retry.
                overlapping = bool(pending)
                cwd = record.get("cwd") if source == "claude_code" else None
                pending[key] = (value, content, cwd)
                continue
            call = pending.pop(key, None)
            if call:
                seen.add(key)
            if overlapping:
                call = None
                overlapping = bool(pending)
            command = None
            error = None
            if call and isinstance(call[0], str) and isinstance(call[1], dict):
                name, args, cwd = call
                command = (key[0], cwd, name, json.dumps(args, sort_keys=True))
                if value is True:
                    error = classify_error(content if isinstance(content, str) else "")
            identical = command is not None and signature is not None and command == signature
            if run and (not identical or error != run[0].error_category):
                yield run, identical and value is False
                run, signature = [], None
            if error is not None and error.value not in UNRELATED_ERRORS:
                signature = command
                if name in ("Bash", "bash") and not isinstance(args.get("command", ""), str):
                    continue
                run.append(ToolCall(name, call_id, args, "", True, error, msg_index=line_number))
    if run:
        yield run, False


def read_transcript(raw):
    if len(raw) > MAX_BYTES:
        raise OverflowError()
    lines = raw.decode("utf-8-sig").split("\n", MAX_RECORDS + 1)
    if len(lines) - int(lines[-1] == "") > MAX_RECORDS:
        raise OverflowError()
    records = [(number, json.loads(line)) for number, line in enumerate(lines, 1) if line.strip()]
    if any(not isinstance(record, dict) for _, record in records):
        raise ValueError()
    codex_sessions = {
        object_or_empty(record.get("payload")).get("id")
        for _, record in records
        if record.get("type") == "session_meta"
        and isinstance(object_or_empty(record.get("payload")).get("id"), str)
    }
    claude = any(
        record.get("type") in ("assistant", "user")
        and isinstance(record.get("sessionId"), str)
        and isinstance(record.get("message"), dict)
        for _, record in records
    )
    if len(codex_sessions) > 1 or bool(codex_sessions) == claude:
        raise ValueError()
    return records, "codex" if codex_sessions else "claude_code"


def preview(raw):
    try:
        records, source = read_transcript(raw)

        # Load the exact runtime's detector without its scanners or LLM analyzer.
        from headroom.learn.loops import DEFAULT_MIN_OCCURRENCES, detect_loops
        from headroom.learn.models import SessionData

        calls, recoveries = Counter(), Counter()
        first_lines = {}
        for run, recovered in read_runs(records, source):
            if len(run) < DEFAULT_MIN_OCCURRENCES:
                continue
            loops = detect_loops([SessionData("preview", tool_calls=run)])
            if loops:
                category = SUGGESTION.get(run[0].error_category.value, "failure")
                calls[category] += loops[0].count
                recoveries[category] += int(recovered)
                first_lines.setdefault(category, loops[0].msg_indices[0])
        return {"status": "ready", "source": source, "suggestions": [
            {"kind": kind, "failures": count, "recoveries": recoveries[kind], "first_line": first_lines[kind]}
            for kind, count in sorted(calls.items())
        ]}
    except OverflowError:
        return {"status": "too_large"}
    except (ValueError, RecursionError):
        return {"status": "invalid_file"}


def replay_items(records, source):
    codex_scope = next((object_or_empty(record.get("payload")).get("id") for _, record in records
                        if record.get("type") == "session_meta"), None)
    for _, record in records:
        if source == "codex":
            if record.get("type") != "response_item":
                continue
            item = object_or_empty(record.get("payload"))
            kind = item.get("type")
            if kind not in ("function_call", "function_call_output", "custom_tool_call", "custom_tool_call_output"):
                continue
            yield codex_scope, item.get("call_id"), kind.endswith("output"), item
        elif record.get("type") in ("assistant", "user"):
            for item in object_or_empty(record.get("message")).get("content", []):
                if isinstance(item, dict) and item.get("type") in ("tool_use", "tool_result"):
                    result = item["type"] == "tool_result"
                    yield record.get("sessionId"), item.get("tool_use_id" if result else "id"), result, item


def local_replay(raw):
    """Try one reviewed transformation, then compare its inverse before counting bytes."""
    try:
        records, source = read_transcript(raw)
        from headroom.transforms.lossless_compaction import collapse_runs, expand_runs

        calls, outputs, ambiguous = {}, {}, set()
        unowned = 0
        for scope, identity, result, item in replay_items(records, source):
            if not isinstance(scope, str) or not scope.strip() or not isinstance(identity, str) or not identity:
                unowned += int(result)
                continue
            key = scope, identity
            if result and key not in calls:
                ambiguous.add(key)
            entries = outputs if result else calls
            if key in entries and entries[key] != item:
                ambiguous.add(key)
            entries[key] = item

        report = {"rule": "repeated_log_lines_v1", "checked": unowned, "protected": 0, "ambiguous": unowned,
                  "eligible": 0, "compacted": 0, "before_bytes": 0, "after_bytes": 0,
                  "original_sha256": hashlib.sha256(raw).hexdigest(), "round_trip_exact": True}
        for key, item in outputs.items():
            report["checked"] += 1
            owner = calls.get(key)
            if key in ambiguous or owner is None:
                report["ambiguous"] += 1
                continue
            if owner.get("name") == "exec_command":
                try:
                    arguments = json.loads(owner.get("arguments", ""))
                except (TypeError, ValueError):
                    arguments = None
                if (not isinstance(arguments, dict) or not isinstance(arguments.get("cmd"), str)
                        or not isinstance(arguments.get("workdir"), str) or not arguments["workdir"]):
                    report["ambiguous"] += 1
                    continue
            # The explicit dictionary is a harness envelope. Printed JSON and merged text have unknown channels.
            value = item.get("output") if source == "codex" else None
            valid = (isinstance(value, dict) and type(value.get("exit_code")) is int and value["exit_code"] == 0
                     and isinstance(value.get("stdout"), str) and value.get("stderr") == ""
                     and owner.get("name") in ("exec_command", "Bash", "bash"))
            stdout = value["stdout"] if valid else ""
            lines = stdout.splitlines()
            if (not valid or not lines or len(stdout.encode("utf-8")) > MAX_REPLAY_OUTPUT
                    or len(lines) > 20_000 or not all(LOG_LINE.match(line) for line in lines)):
                report["protected"] += 1
                continue
            compacted = collapse_runs(stdout)
            restored = expand_runs(compacted)
            # The whole output envelope is recovered, including channels, status and any unknown metadata.
            if {**value, "stdout": restored} != value:
                report["protected"] += 1
                continue
            before, after = len(stdout.encode("utf-8")), len(compacted.encode("utf-8"))
            if after >= before:
                compacted, after = stdout, before
            report["eligible"] += 1
            report["compacted"] += int(compacted != stdout)
            report["before_bytes"] += before
            report["after_bytes"] += after
        return {"status": "trial_ready", "report": report}
    except OverflowError:
        return {"status": "too_large"}
    except (ValueError, RecursionError, TypeError):
        return {"status": "invalid_file"}


def block_egress(event, _args):
    if event.startswith("socket.") or event in ("subprocess.Popen", "os.system", "os.startfile"):
        raise PermissionError("Learn preview is local")


if __name__ == "__main__":
    sys.addaudithook(block_egress)
    sys.path.append(str(Path.cwd() / "Lib" / "site-packages"))
    operation = local_replay if sys.argv[1:] == ["replay"] else preview
    print(json.dumps(operation(sys.stdin.buffer.read(MAX_BYTES + 1)), ensure_ascii=True))
