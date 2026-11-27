import ctypes
from ctypes import wintypes
import json
import sys


class ProcessEntry(ctypes.Structure):
    _fields_ = [
        ("size", wintypes.DWORD), ("usage", wintypes.DWORD), ("pid", wintypes.DWORD),
        ("heap", ctypes.c_size_t), ("module", wintypes.DWORD), ("threads", wintypes.DWORD),
        ("parent", wintypes.DWORD), ("priority", wintypes.LONG), ("flags", wintypes.DWORD),
        ("exe", wintypes.WCHAR * 260),
    ]


def python_parents(kernel):
    kernel.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
    kernel.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CloseHandle.restype = wintypes.BOOL
    first, next_entry = kernel.Process32FirstW, kernel.Process32NextW
    for function in (first, next_entry):
        function.argtypes = [wintypes.HANDLE, ctypes.POINTER(ProcessEntry)]
        function.restype = wintypes.BOOL
    snapshot = kernel.CreateToolhelp32Snapshot(2, 0)
    if snapshot == ctypes.c_void_p(-1).value:
        raise OSError("process_snapshot_failed")
    try:
        entry = ProcessEntry(size=ctypes.sizeof(ProcessEntry))
        if not first(snapshot, ctypes.byref(entry)):
            raise OSError("process_snapshot_empty")
        parents = {}
        while True:
            if entry.exe.casefold() == "python.exe":
                parents[entry.pid] = entry.parent
            if not next_entry(snapshot, ctypes.byref(entry)):
                return parents
    finally:
        kernel.CloseHandle(snapshot)

owner = int(sys.argv[1])
assert owner > 0
parents = python_parents(ctypes.WinDLL("kernel32", use_last_error=True))
controllers = sorted(pid for pid, parent in parents.items() if parent == owner)
helpers = sorted(pid for pid, parent in parents.items() if parent in controllers)
engines = sorted(pid for pid, parent in parents.items() if parent in helpers)
print(json.dumps({"controller": controllers, "helper": helpers, "engine": engines}))
