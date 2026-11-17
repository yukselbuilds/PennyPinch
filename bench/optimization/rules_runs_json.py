"""Local-only representation candidate. No product/runtime import or activation."""
import itertools
import hashlib
import json
from pathlib import Path
import runpy

FORMAT = "headroom.runs.json.v1"
MAX_BYTES = 16 * 1024 * 1024
MAX_LINES = 64000


def encode_runs(text):
    if type(text) is not str or len(text.encode("utf-8")) > MAX_BYTES:
        raise ValueError("source_outside_scope")
    lines = text.splitlines(keepends=True)
    if len(lines) > MAX_LINES:
        raise ValueError("source_outside_scope")
    runs = [{"line": line, "occurrences": sum(1 for _ in group)} for line, group in itertools.groupby(lines)]
    encoded = json.dumps({"format": FORMAT, "runs": runs}, ensure_ascii=False, separators=(",", ":"))
    if len(encoded.encode("utf-8")) > MAX_BYTES:
        raise ValueError("frame_outside_scope")
    return encoded


def decode_runs(encoded):
    if type(encoded) is not str or len(encoded.encode("utf-8")) > MAX_BYTES:
        raise ValueError("frame_outside_scope")
    try:
        frame = json.loads(encoded)
    except (ValueError, RecursionError) as error:
        raise ValueError("invalid_frame") from error
    if (type(frame) is not dict or set(frame) != {"format", "runs"} or frame["format"] != FORMAT
            or type(frame["runs"]) is not list or len(frame["runs"]) > MAX_LINES):
        raise ValueError("invalid_frame")
    pieces, size, lines = [], 0, 0
    for run in frame["runs"]:
        if type(run) is not dict or set(run) != {"line", "occurrences"}:
            raise ValueError("invalid_run")
        line, count = run["line"], run["occurrences"]
        if type(line) is not str or type(count) is not int or count <= 0 or line.splitlines(keepends=True) != [line]:
            raise ValueError("invalid_run")
        size += len(line.encode("utf-8")) * count
        lines += count
        if size > MAX_BYTES or lines > MAX_LINES:
            raise ValueError("expansion_outside_scope")
        pieces.append(line * count)
    return "".join(pieces)


def trial_capture(raw, owner, call_id):
    """Shape one owned capture locally; reuse the existing channel/source gate."""
    preview = runpy.run_path(str(Path(__file__).resolve().parents[2] / "runtime/learn_preview.py"))
    outcome = preview["local_replay"](raw)
    if outcome["status"] != "trial_ready":
        return {"status": outcome["status"]}
    report = outcome["report"]
    if report["original_sha256"] != hashlib.sha256(raw).hexdigest():
        raise ValueError("capture_fingerprint_changed")
    records, source = preview["read_transcript"](raw)
    items = list(preview["replay_items"](records, source))
    if (type(owner) is not str or not owner or type(call_id) is not str or not call_id or source != "codex"
            or len(items) != 2 or items[0][2] or not items[1][2]
            or any(scope != owner or identity != call_id for scope, identity, _, _ in items)
            or report["checked"] != 1 or report["ambiguous"]):
        return {"status": "ambiguous"}
    result = items[1][3]
    original = result.get("output")
    error = any(item.get("error") is not None or item.get("isError") is True or item.get("is_error") is True
                or item.get("status") in ("failed", "error", "cancelled")
                for item in (result, original) if isinstance(item, dict))
    if error or report["eligible"] != 1 or report["protected"] or not report["round_trip_exact"]:
        return {"status": "protected", "candidate": original}
    encoded = encode_runs(original["stdout"])
    before, after = len(original["stdout"].encode("utf-8")), len(encoded.encode("utf-8"))
    if after >= before:
        return {"status": "unchanged", "candidate": original}
    candidate = {**original, "stdout": encoded}
    if {**candidate, "stdout": decode_runs(encoded)} != original:
        raise ValueError("capture_roundtrip_changed")
    return {"status": "candidate_ready", "candidate": candidate, "before_bytes": before, "after_bytes": after}
