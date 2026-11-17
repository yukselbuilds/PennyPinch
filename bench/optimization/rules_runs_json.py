"""Local-only representation candidate. No product/runtime import or activation."""
import itertools
import json

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
