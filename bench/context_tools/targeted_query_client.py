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


HERE = Path(__file__).resolve().parent
prototype = runpy.run_path(str(HERE / "targeted_query_replay.py"))
query = prototype["query"]


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


def serve():
    from mcp.server.mcpserver import MCPServer
    from mcp.server.mcpserver.context import Context
    from mcp.server.mcpserver.exceptions import ToolError
    from mcp.types import ToolAnnotations

    raw = b"".join(prototype["report_rows"]())
    captured = subprocess.run([sys.executable, "-I", "-B", "-c", prototype["PRODUCER"]],
        input=struct.pack(">II", len(raw), 0) + raw, capture_output=True, timeout=10,
        env={"SystemRoot": os.environ.get("SystemRoot", "")}, cwd=Path.cwd(),
        creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    assert (captured.stdout, captured.stderr, captured.returncode) == (raw, b"", 0), "fixed_producer_changed"
    session = ReportSession(query.ToolOutput(query.REPORT_TOOL, "", captured.stdout, captured.stderr, captured.returncode))
    server = MCPServer("headroom_query_pilot")
    annotations = ToolAnnotations(read_only_hint=True, destructive_hint=False, open_world_hint=False)
    encode = lambda value: json.dumps(value, ensure_ascii=True, separators=(",", ":"))

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
    async def Read(archive_id: str) -> str:
        """Read the captured report's original stdout, stderr, exit code and source identity."""
        try:
            return encode(session.read_original(archive_id))
        except LookupError:
            raise ToolError("archive_unavailable") from None

    server.run(transport="stdio")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", action="store_true", required=True)
    parser.parse_args()
    serve()
