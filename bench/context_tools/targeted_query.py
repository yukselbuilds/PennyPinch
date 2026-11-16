"""Local proof of exact JSONL selection from an already authorized tool result."""
from dataclasses import dataclass
from io import BytesIO
import json
import re
from uuid import uuid4


REPORT_TOOL = "read_structured_report"
MAX_CAPTURE_BYTES = 1024 * 1024
MAX_ARCHIVE_BYTES = 4 * MAX_CAPTURE_BYTES
MAX_CAPTURES = 16
MAX_ROW_BYTES = 64 * 1024
MAX_ROWS = 10_000
RECORD_ID = re.compile(r"[a-zA-Z0-9_-]{1,64}")
ARCHIVE_ID = re.compile(r"[a-f0-9]{32}")


@dataclass(frozen=True)
class ToolOutput:
    tool: str
    call_id: str
    stdout: bytes
    stderr: bytes
    exit_code: int


@dataclass(frozen=True)
class Selection:
    status: str
    row: bytes | None = None
    count: int | None = None


def _unique_object(pairs):
    result = dict(pairs)
    if len(result) != len(pairs):
        raise ValueError("duplicate_json_key")
    return result


def _invalid_constant(_value):
    raise ValueError("invalid_json_constant")


class Archive:
    """Bounded, synchronous storage for one proof run. Owners are trusted objects."""

    def __init__(self):
        self._entries = {}

    def keep(self, owner, output: ToolOutput) -> str:
        if owner is None or type(output.stdout) is not bytes or type(output.stderr) is not bytes:
            raise ValueError("archive_input_invalid")
        size = len(output.stdout) + len(output.stderr)
        held = sum(len(item.stdout) + len(item.stderr) for _, item in self._entries.values())
        if size > MAX_CAPTURE_BYTES or held + size > MAX_ARCHIVE_BYTES or len(self._entries) >= MAX_CAPTURES:
            raise ValueError("archive_capacity")
        handle = uuid4().hex
        if handle in self._entries:
            raise ValueError("archive_id_collision")
        self._entries[handle] = (owner, output)
        return handle

    def read_original(self, owner, handle: str) -> ToolOutput:
        if not isinstance(handle, str) or ARCHIVE_ID.fullmatch(handle) is None:
            raise LookupError("archive_unavailable")
        entry = self._entries.get(handle)
        if entry is None or entry[0] is not owner:
            raise LookupError("archive_unavailable")
        return entry[1]

    def select(self, owner, arguments) -> Selection:
        if not isinstance(arguments, dict) or arguments.keys() != {"archive_id", "record_id"}:
            raise ValueError("invalid_query")
        record_id = arguments["record_id"]
        if not isinstance(record_id, str) or RECORD_ID.fullmatch(record_id) is None:
            raise ValueError("invalid_query")
        original = self.read_original(owner, arguments["archive_id"])
        if original.tool != REPORT_TOOL or original.exit_code != 0 or original.stderr:
            return Selection("full_output_required")

        seen, selected = set(), None
        try:
            for line in BytesIO(original.stdout):
                if len(seen) >= MAX_ROWS or len(line) > MAX_ROW_BYTES:
                    return Selection("query_capacity")
                # Only IDs are interpreted. Returning the original row preserves numeric lexemes and line endings.
                value = json.loads(line.decode("utf-8"), parse_int=lambda _: None, parse_float=lambda _: None,
                                   parse_constant=_invalid_constant, object_pairs_hook=_unique_object)
                key = value.get("id") if isinstance(value, dict) else None
                if not isinstance(key, str) or RECORD_ID.fullmatch(key) is None:
                    return Selection("invalid_jsonl")
                if key in seen:
                    return Selection("ambiguous_jsonl")
                seen.add(key)
                if key == record_id:
                    selected = line
        except (ValueError, RecursionError):
            return Selection("invalid_jsonl")
        if selected is None:
            return Selection("record_missing")
        return Selection("selected", selected, len(seen))
