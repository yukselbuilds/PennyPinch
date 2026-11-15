"""Summarize one user-selected tool transcript without invoking an analyzer."""

import json
import re
import sys
from collections import Counter
from pathlib import Path

MAX_BYTES = 16 * 1024 * 1024
MAX_RECORDS = 64_000
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


def preview(raw):
    if len(raw) > MAX_BYTES:
        return {"status": "too_large"}
    try:
        lines = raw.decode("utf-8-sig").split("\n", MAX_RECORDS + 1)
        if len(lines) - int(lines[-1] == "") > MAX_RECORDS:
            return {"status": "too_large"}
        records = [(number, json.loads(line)) for number, line in enumerate(lines, 1) if line.strip()]
        if any(not isinstance(record, dict) for _, record in records):
            return {"status": "invalid_file"}
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
            return {"status": "invalid_file"}

        # Load the exact runtime's detector without its scanners or LLM analyzer.
        from headroom.learn.loops import DEFAULT_MIN_OCCURRENCES, detect_loops
        from headroom.learn.models import SessionData

        source = "codex" if codex_sessions else "claude_code"
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
    except (ValueError, RecursionError):
        return {"status": "invalid_file"}


def block_egress(event, _args):
    if event.startswith("socket.") or event in ("subprocess.Popen", "os.system", "os.startfile"):
        raise PermissionError("Learn preview is local")


if __name__ == "__main__":
    sys.addaudithook(block_egress)
    sys.path.append(str(Path.cwd() / "Lib" / "site-packages"))
    print(json.dumps(preview(sys.stdin.buffer.read(MAX_BYTES + 1)), ensure_ascii=True))
