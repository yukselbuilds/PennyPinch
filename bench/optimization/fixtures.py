"""Synthetic coding fixtures. No user files or account data are consumed."""
from copy import deepcopy
import json


CASES = ("source_read", "numeric_read", "repetitive_logs", "short_json")
PROTECTED = ("source_read", "numeric_read")


def output(case, index):
    if case == "source_read":
        return "\n".join(
            f"def fixture_value_{number}(text):\n    return text + 'Türkçe λ {index}-{number}'"
            for number in range(80)
        ) + "\n"
    if case == "numeric_read":
        return json.dumps([
            {"sequence": number, "exact": "9007199254740993", "small": "0.000000000000001",
             "negative": "-17.2500", "sample": index}
            for number in range(80)
        ], ensure_ascii=False, separators=(",", ":"))
    if case == "repetitive_logs":
        return "\n".join(
            f"2026-01-01T00:00:{number % 60:02d}.{number % 1000:03d}Z INFO "
            f"worker-{number % 8:03d} request completed status=200 latency_ms=17 "
            f"region=fixture trace_id={index:04d}{number:012d}"
            for number in range(200)
        )
    if case == "short_json":
        return json.dumps({"ok": True, "count": index}, separators=(",", ":"))
    raise ValueError("unknown_fixture")


def append(body, case, index):
    body = deepcopy(body)
    tool = "Read" if case in PROTECTED else "inspect_workspace"
    call_id = f"call-{case}-{index}"
    body["input"].extend([
        {"type": "message", "role": "assistant",
         "content": [{"type": "output_text", "text": "Inspecting the local fixture."}]},
        {"type": "function_call", "call_id": call_id, "name": tool, "arguments": "{}"},
        {"type": "function_call_output", "call_id": call_id, "output": output(case, index)},
        {"type": "message", "role": "user",
         "content": [{"type": "input_text", "text": f"Summarize fixture turn {index}."}]},
    ])
    return body


def conversation(case, model, effort, turns):
    tool = "Read" if case in PROTECTED else "inspect_workspace"
    body = {
        "model": model,
        "reasoning": {"effort": effort},
        "stream": True,
        "tools": [{"type": "function", "name": tool, "description": "Read synthetic fixture",
                   "parameters": {"type": "object", "properties": {}}}],
        "input": [{"type": "message", "role": "user",
                   "content": [{"type": "input_text", "text": f"Inspect {case} fixtures."}]}],
    }
    for index in range(1, 5):
        body = append(body, case, index)
    result = [body]
    for turn in range(1, turns):
        body = append(body, case, turn + 4)
        result.append(body)
    return result


def outputs(body):
    return {item["call_id"]: item["output"] for item in body["input"]
            if item.get("type") == "function_call_output"}


def self_check():
    for case in CASES:
        rows = conversation(case, "fixture-model", "high", 3)
        assert len(rows) == 3
        assert rows[1]["input"][:len(rows[0]["input"])] == rows[0]["input"]
        assert rows[2]["input"][:len(rows[1]["input"])] == rows[1]["input"]
        for row in rows:
            calls = {item["call_id"] for item in row["input"] if item.get("type") == "function_call"}
            assert calls == set(outputs(row))
            assert len(calls) == len(outputs(row))
            assert row["model"] == "fixture-model" and row["reasoning"] == {"effort": "high"}
