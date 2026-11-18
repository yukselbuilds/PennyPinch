"""Fixed synthetic report tools for one isolated CLI-owned stdio connection."""
import argparse
from dataclasses import replace
import json
import os
from pathlib import Path
import runpy
import struct
import subprocess
import sys
from typing import Literal


HERE = Path(__file__).resolve().parent
prototype = runpy.run_path(str(HERE / "targeted_query_replay.py"))
query = prototype["query"]
READ_PART_BYTES = 6 * 1024
READ_REPLY_BYTES = 8 * 1024
FAILURE_FIXTURE = (b"Partial report\r\n", "ERROR checksum — λ\r\n".encode(), 9)


def encode(value):
    return json.dumps(value, ensure_ascii=True, separators=(",", ":"))


def envelope(output):
    return {"source_tool": output.tool, "source_call_id": output.call_id,
            "stdout": output.stdout.decode("utf-8"), "stderr": output.stderr.decode("utf-8"),
            "exit_code": output.exit_code}


class ReportSession:
    def __init__(self, output):
        self._output = output
        self._archive, self._owner = query.Archive(), object()

    def capture(self, source_request_id):
        if type(source_request_id) is not str or not 1 <= len(source_request_id) <= 128:
            raise ValueError("invalid_source_request")
        output = replace(self._output, call_id=source_request_id)
        handle = self._archive.keep(self._owner, output)
        return {"archive_id": handle, "source_tool": output.tool, "source_call_id": output.call_id}

    def select(self, arguments):
        selected = self._archive.select(self._owner, arguments)
        output = self._archive.read_original(self._owner, arguments["archive_id"])
        value = {"status": selected.status, "archive_id": arguments["archive_id"],
                 "source_tool": output.tool, "source_call_id": output.call_id}
        if selected.status == "selected":
            value.update(stdout=selected.row.decode("utf-8"), stderr="", exit_code=0,
                         record_count=selected.count, matched_count=1, truncated=False)
        return value

    def read_original(self, handle):
        return envelope(self._archive.read_original(self._owner, handle))

    def read_part(self, handle, stream, offset):
        output = self._archive.read_original(self._owner, handle)
        if stream not in ("stdout", "stderr") or type(offset) is not int:
            raise ValueError("invalid_read_range")
        data = output.stdout if stream == "stdout" else output.stderr
        if not 0 <= offset <= len(data) or (offset < len(data) and data[offset] & 0xC0 == 0x80):
            raise ValueError("invalid_read_range")
        end = min(offset + READ_PART_BYTES, len(data))
        metadata = {"archive_id": handle, "source_tool": output.tool, "source_call_id": output.call_id,
                    "stream": stream, "offset": offset, "total_bytes": len(data), "exit_code": output.exit_code}
        while True:
            while offset < end < len(data) and data[end] & 0xC0 == 0x80:
                end -= 1
            if end == offset and offset < len(data):
                raise ValueError("invalid_read_range")
            piece = {**metadata, "next_offset": end, "text": data[offset:end].decode("utf-8")}
            # Escape-heavy text can exceed the wire budget even within the raw byte limit.
            if len(encode(piece)) <= READ_REPLY_BYTES:  # The encoder emits ASCII only.
                return piece
            if end == offset:
                raise ValueError("invalid_read_range")
            end = offset + (end - offset) // 2


def serve(failed_report=False, *, source=None):
    from mcp.server.mcpserver import MCPServer
    from mcp.server.mcpserver.context import Context
    from mcp.server.mcpserver.exceptions import ToolError
    from mcp.types import ToolAnnotations
    from pydantic import StrictInt

    if source is None:
        source = FAILURE_FIXTURE if failed_report else (b"".join(prototype["report_rows"]()), b"", 0)
    raw, stderr, exit_code = source
    captured = subprocess.run([sys.executable, "-I", "-B", "-c", prototype["PRODUCER"]],
        input=struct.pack(">II", len(raw), exit_code) + raw + stderr, capture_output=True, timeout=10,
        env={"SystemRoot": os.environ.get("SystemRoot", "")}, cwd=Path.cwd(),
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    assert (captured.stdout, captured.stderr, captured.returncode) == (raw, stderr, exit_code), "fixed_producer_changed"
    session = ReportSession(query.ToolOutput(query.REPORT_TOOL, "", captured.stdout, captured.stderr, captured.returncode))
    server = MCPServer("headroom_query_pilot")
    annotations = ToolAnnotations(read_only_hint=True, destructive_hint=False, open_world_hint=False)

    @server.tool(structured_output=False, annotations=annotations)
    async def read_structured_report(ctx: Context) -> str:
        """Capture only the fixed synthetic report. Return this connection's archive reference."""
        try:
            return encode(session.capture("mcp:" + str(ctx.request_id)))
        except ValueError:
            raise ToolError("archive_capture_unavailable") from None

    @server.tool(structured_output=False, annotations=annotations)
    async def select_report_record(archive_id: str, record_id: str) -> str:
        """Select an exact ID and count all records in this connection's captured report."""
        try:
            return encode(session.select({"archive_id": archive_id, "record_id": record_id}))
        except LookupError:
            raise ToolError("archive_unavailable") from None
        except ValueError:
            raise ToolError("invalid_query") from None

    @server.tool(structured_output=False, annotations=annotations)
    async def Read(archive_id: str, stream: Literal["stdout", "stderr"], offset: StrictInt) -> str:
        """Read original bytes as UTF-8 text. Start each stream at 0; continue at next_offset up to total_bytes."""
        try:
            return encode(session.read_part(archive_id, stream, offset))
        except LookupError:
            raise ToolError("archive_unavailable") from None
        except ValueError:
            raise ToolError("invalid_read_range") from None

    server.run(transport="stdio")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", action="store_true", required=True)
    parser.add_argument("--failed-report", action="store_true")
    serve(parser.parse_args().failed_report)
