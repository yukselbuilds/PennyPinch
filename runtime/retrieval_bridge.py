"""OpenCode stdio MCP: retrieve a hash through the verified, app-owned Windows ingress."""
import ctypes
from ctypes import wintypes
import http.client
import json
import os
from pathlib import Path
import re
import socket
import stat
import sys
import threading
import uuid

MAX_MESSAGE = 64 * 1024
MAX_CONTENT = 8 * 1024 * 1024
PROTOCOLS = ("2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25")
HASH = re.compile(r"[a-fA-F0-9]{12,24}\Z")


class BridgeError(Exception):
    pass


def _object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError("duplicate_key")
        value[key] = item
    return value


def _json(raw):
    return json.loads(raw, object_pairs_hook=_object,
                      parse_constant=lambda _: (_ for _ in ()).throw(ValueError("invalid_number")))


def _path_identity(path):
    value = os.path.normcase(str(path))
    return value[4:] if value.startswith("\\\\?\\") else value


def _regular_path(value):
    path = Path(value)
    if not path.is_absolute() or ".." in path.parts:
        raise BridgeError("bridge_path_unavailable")
    try:
        for component in (*reversed(path.parents), path):
            info = component.lstat()
            if stat.S_ISLNK(info.st_mode) or getattr(info, "st_file_attributes", 0) & 0x400:
                raise BridgeError("bridge_path_unavailable")
        if not stat.S_ISREG(info.st_mode):
            raise BridgeError("bridge_path_unavailable")
    except OSError:
        raise BridgeError("bridge_path_unavailable") from None
    return path


def _endpoint(path, expected_id):
    try:
        _regular_path(path)
        with path.open("rb") as stream:
            raw = stream.read(1025)
        if len(raw) > 1024:
            raise ValueError("endpoint_too_large")
        value = _json(raw)
        if (not isinstance(value, dict)
                or set(value) != {"schema_version", "bind_address", "port", "endpoint_id", "revision"}
                or type(value["schema_version"]) is not int or value["schema_version"] != 1
                or value["bind_address"] != "127.0.0.1" or value["endpoint_id"] != expected_id
                or type(value["port"]) is not int or not 0 < value["port"] <= 65535
                or type(value["revision"]) is not int or value["revision"] < 1):
            raise ValueError("endpoint_invalid")
        return value["port"]
    except (OSError, ValueError, TypeError, RecursionError):
        raise BridgeError("headroom_endpoint_unavailable") from None


def _peer_pid(connection, api):
    # Match the server side of this exact established connection, never just the listening port.
    server, client = connection.getpeername(), connection.getsockname()
    if server[0] != "127.0.0.1" or client[0] != "127.0.0.1":
        raise BridgeError("headroom_owner_unverified")
    size = wintypes.ULONG()
    for _ in range(3):
        if api.GetExtendedTcpTable(None, ctypes.byref(size), False, 2, 5, 0) != 122 or not 4 <= size.value <= 4 * 1024 * 1024:
            break
        table = ctypes.create_string_buffer(size.value)
        result = api.GetExtendedTcpTable(table, ctypes.byref(size), False, 2, 5, 0)
        if result == 122:
            continue
        if result != 0 or not 4 <= size.value <= len(table):
            break
        rows = memoryview(table).cast("B")[:size.value]
        count = int.from_bytes(rows[:4], "little")
        if 4 + count * 24 > len(rows):
            break
        expected = (5, int.from_bytes(socket.inet_aton(server[0]), "little"), server[1],
                    int.from_bytes(socket.inet_aton(client[0]), "little"), client[1])
        owners = []
        for index in range(count):
            row = rows[4 + index * 24:4 + (index + 1) * 24]
            values = [int.from_bytes(row[offset:offset + 4], "little") for offset in range(0, 24, 4)]
            observed = (values[0], values[1], socket.ntohs(values[2] & 0xffff),
                        values[3], socket.ntohs(values[4] & 0xffff))
            if observed == expected:
                owners.append(values[5])
        if len(owners) == 1 and owners[0] > 0:
            return owners[0]
        break
    raise BridgeError("headroom_owner_unverified")


class WindowsOwner:
    def __init__(self, connection, executable):
        if sys.platform != "win32":
            raise BridgeError("bridge_platform_unavailable")
        self.handle = None
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        security = ctypes.WinDLL("advapi32", use_last_error=True)
        ip = ctypes.WinDLL("iphlpapi", use_last_error=True)
        self.kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        self.kernel.OpenProcess.restype = wintypes.HANDLE
        self.kernel.CloseHandle.argtypes = [wintypes.HANDLE]
        self.kernel.GetCurrentProcess.restype = wintypes.HANDLE
        self.kernel.QueryFullProcessImageNameW.argtypes = [wintypes.HANDLE, wintypes.DWORD, wintypes.LPWSTR, ctypes.POINTER(wintypes.DWORD)]
        self.kernel.GetExitCodeProcess.argtypes = [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)]
        security.OpenProcessToken.argtypes = [wintypes.HANDLE, wintypes.DWORD, ctypes.POINTER(wintypes.HANDLE)]
        security.GetTokenInformation.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(wintypes.DWORD)]
        security.EqualSid.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        ip.GetExtendedTcpTable.argtypes = [ctypes.c_void_p, ctypes.POINTER(wintypes.ULONG), wintypes.BOOL, wintypes.ULONG, ctypes.c_int, wintypes.ULONG]
        pid = _peer_pid(connection, ip)
        self.handle = self.kernel.OpenProcess(0x1000, False, pid)
        if not self.handle:
            raise BridgeError("headroom_owner_unverified")
        tokens = []
        try:
            name = ctypes.create_unicode_buffer(32768)
            length = wintypes.DWORD(len(name))
            code = wintypes.DWORD()
            if (not self.kernel.QueryFullProcessImageNameW(self.handle, 0, name, ctypes.byref(length))
                    or _path_identity(_regular_path(name.value)) != _path_identity(_regular_path(executable))
                    or not self.kernel.GetExitCodeProcess(self.handle, ctypes.byref(code)) or code.value != 259):
                raise BridgeError("headroom_owner_unverified")
            users = []
            for process in (self.handle, self.kernel.GetCurrentProcess()):
                token = wintypes.HANDLE()
                if not security.OpenProcessToken(process, 8, ctypes.byref(token)):
                    raise BridgeError("headroom_owner_unverified")
                tokens.append(token)
                needed = wintypes.DWORD()
                security.GetTokenInformation(token, 1, None, 0, ctypes.byref(needed))
                if not ctypes.sizeof(ctypes.c_void_p) <= needed.value <= 65536:
                    raise BridgeError("headroom_owner_unverified")
                user = ctypes.create_string_buffer(needed.value)
                if not security.GetTokenInformation(token, 1, user, len(user), ctypes.byref(needed)):
                    raise BridgeError("headroom_owner_unverified")
                users.append(user)
            if not security.EqualSid(ctypes.cast(users[0], ctypes.POINTER(ctypes.c_void_p))[0],
                                     ctypes.cast(users[1], ctypes.POINTER(ctypes.c_void_p))[0]):
                raise BridgeError("headroom_owner_unverified")
        except BaseException:
            self.close()
            raise
        finally:
            for token in tokens:
                self.kernel.CloseHandle(token)

    def close(self):
        if self.handle:
            self.kernel.CloseHandle(self.handle)
            self.handle = None


class Retrieval:
    def __init__(self, endpoint_record, endpoint_id, owner_executable):
        if str(uuid.UUID(endpoint_id)) != endpoint_id:
            raise BridgeError("bridge_arguments_invalid")
        self.record = _regular_path(endpoint_record)
        self.endpoint_id = endpoint_id
        self.executable = _regular_path(owner_executable)

    def retrieve(self, arguments):
        if (not isinstance(arguments, dict) or set(arguments) != {"hash"}
                or not isinstance(arguments["hash"], str) or not HASH.fullmatch(arguments["hash"])):
            raise BridgeError("retrieval_hash_invalid")
        key = arguments["hash"].lower()
        port = _endpoint(self.record, self.endpoint_id)
        connection = http.client.HTTPConnection("127.0.0.1", port, timeout=3)
        owner = None
        timer = None
        try:
            connection.connect()
            transport = connection.sock
            # A total deadline also bounds a peer that slowly trickles headers or content.
            def expire():
                try:
                    transport.shutdown(socket.SHUT_RDWR)
                except OSError:
                    pass
            timer = threading.Timer(10, expire)
            timer.daemon = True
            timer.start()
            owner = WindowsOwner(transport, self.executable)
            connection.request("POST", "/v1/retrieve", json.dumps({"hash": key}),
                               {"Content-Type": "application/json", "Connection": "close"})
            response = connection.getresponse()
            if response.status == 404:
                raise BridgeError("retrieval_missing_or_expired")
            if response.status != 200 or response.getheader("Content-Encoding") not in (None, "identity"):
                raise BridgeError("headroom_retrieval_unavailable")
            raw = response.read(MAX_CONTENT + 1)
            if len(raw) > MAX_CONTENT:
                raise BridgeError("retrieval_content_too_large")
            value = _json(raw)
            if not isinstance(value, dict) or value.get("hash") != key or not isinstance(value.get("original_content"), str):
                raise BridgeError("headroom_retrieval_invalid")
            return {"hash": key, "original_content": value["original_content"]}
        except (OSError, http.client.HTTPException, ValueError, RecursionError):
            raise BridgeError("headroom_retrieval_unavailable") from None
        finally:
            if timer:
                timer.cancel()
            connection.close()
            if owner:
                owner.close()


def serve(source, sink, retrieval):
    initialized = False
    while True:
        raw = source.readline(MAX_MESSAGE + 1)
        if not raw:
            return
        request_id = None
        try:
            if len(raw) > MAX_MESSAGE:
                raise ValueError("message_too_large")
            message = _json(raw)
            if not isinstance(message, dict) or message.get("jsonrpc") != "2.0" or not isinstance(message.get("method"), str):
                raise ValueError("invalid_request")
            if "id" not in message:
                continue
            request_id = message["id"]
            if not (type(request_id) is int and abs(request_id) <= 2**53 - 1 or isinstance(request_id, str) and len(request_id) <= 128):
                request_id = None
                raise ValueError("invalid_id")
            method = message["method"]
            if method == "initialize" and not initialized:
                params = message.get("params", {})
                if not isinstance(params, dict) or not isinstance(params.get("protocolVersion"), str):
                    raise ValueError("invalid_protocol")
                initialized = True
                version = params["protocolVersion"] if params["protocolVersion"] in PROTOCOLS else PROTOCOLS[-1]
                result = {"protocolVersion": version, "capabilities": {"tools": {}},
                          "serverInfo": {"name": "headroom-retrieval", "version": "1"}}
            elif method == "ping":
                result = {}
            elif not initialized:
                raise ValueError("not_initialized")
            elif method == "tools/list":
                result = {"tools": [{"name": "retrieve", "description": "Retrieve the full original content for a Headroom compression hash.",
                          "inputSchema": {"type": "object", "properties": {"hash": {"type": "string", "pattern": "^[a-fA-F0-9]{12,24}$"}},
                                          "required": ["hash"], "additionalProperties": False},
                          "annotations": {"readOnlyHint": True, "destructiveHint": False, "openWorldHint": False}}]}
            elif method == "tools/call":
                params = message.get("params", {})
                try:
                    if not isinstance(params, dict) or params.get("name") != "retrieve":
                        raise BridgeError("retrieval_tool_invalid")
                    value = retrieval.retrieve(params.get("arguments"))
                    result = {"content": [{"type": "text", "text": json.dumps(value)}], "isError": False}
                except BridgeError as error:
                    result = {"content": [{"type": "text", "text": str(error)}], "isError": True}
            else:
                response = {"jsonrpc": "2.0", "id": request_id, "error": {"code": -32601, "message": "method_unavailable"}}
                sink.write(json.dumps(response).encode() + b"\n")
                sink.flush()
                continue
            response = {"jsonrpc": "2.0", "id": request_id, "result": result}
        except (ValueError, TypeError, RecursionError):
            response = {"jsonrpc": "2.0", "id": request_id, "error": {"code": -32600, "message": "invalid_request"}}
        sink.write(json.dumps(response).encode() + b"\n")
        sink.flush()
        if len(raw) > MAX_MESSAGE:
            return


def main():
    gate = None
    try:
        if len(sys.argv) != 5 or sys.platform != "win32":
            raise BridgeError("bridge_arguments_invalid")
        if sys.argv[4]:
            gate = open_desktop_use(sys.argv[4])
        retrieval = Retrieval(*sys.argv[1:4])
        serve(sys.stdin.buffer, sys.stdout.buffer, retrieval)
        return 0
    except (BridgeError, ValueError, OSError):
        sys.stderr.write("headroom_retrieval_bridge_unavailable\n")
        return 1
    finally:
        if gate:
            gate.close()


def open_desktop_use(path):
    import msvcrt

    path = _regular_path(path)
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD,
                                  ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE]
    kernel.CreateFileW.restype = wintypes.HANDLE
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    # The bundled Python can keep DLLs mapped until OpenCode stops its MCP child.
    # Share the same read-only gate as the desktop so removal waits for both.
    handle = kernel.CreateFileW(str(path), 0x80000000, 1, None, 3, 0x00200000, None)
    if not handle or handle == ctypes.c_void_p(-1).value:
        raise BridgeError("headroom_desktop_use_unavailable")
    try:
        fd = msvcrt.open_osfhandle(handle, os.O_RDONLY | os.O_BINARY)
    except OSError:
        kernel.CloseHandle(handle)
        raise
    stream = os.fdopen(fd, "rb")
    try:
        info = os.fstat(stream.fileno())
        if (not stat.S_ISREG(info.st_mode) or info.st_size != 0
                or getattr(info, "st_file_attributes", 0) & 0x400):
            raise BridgeError("headroom_desktop_use_unavailable")
        return stream
    except BaseException:
        stream.close()
        raise


if __name__ == "__main__":
    raise SystemExit(main())
