from base64 import b64decode
import json
from pathlib import Path
import socket
import subprocess
import time

socket_path = Path("qmp.socket").resolve()
receipts = Path("guest.serial")
guest = [
    "qemu-system-x86_64", "-enable-kvm", "-machine", "pc", "-cpu", "host",
    "-smp", "3", "-m", "6144", "-S", "-boot", "order=c,once=d",
    "-display", "none", "-vga", "std", "-rtc", "base=localtime",
    "-drive", "file=clean.qcow2,format=qcow2,index=0",
    "-drive", "file=windows.iso,format=raw,media=cdrom,index=2,readonly=on",
    "-drive", "file=answer.iso,format=raw,media=cdrom,index=3,readonly=on",
    "-netdev", "user,id=nat", "-device", "e1000,netdev=nat,id=guestnic",
    "-serial", "file:guest.serial", "-qmp", f"unix:{socket_path},server=on,wait=off",
]


def notice(name, data):
    print(f"::notice title=H037 {name}::{json.dumps(data, separators=(',', ':'), ensure_ascii=True)}", flush=True)


def qmp(stream, command, **arguments):
    stream.write(json.dumps({"execute": command, "arguments": arguments}).encode() + b"\n")
    while True:
        line = stream.readline()
        if not line:
            raise RuntimeError("monitor_disconnected")
        response = json.loads(line)
        if "error" in response:
            raise RuntimeError("monitor_command_failed_" + command)
        if "return" in response:
            return response["return"]


def main(process):
    with socket.socket(socket.AF_UNIX) as monitor:
        monitor.settimeout(8)
        for _ in range(100):
            if socket_path.exists():
                break
            if process.poll() is not None:
                raise RuntimeError("vm_start_failed")
            time.sleep(0.1)
        monitor.connect(str(socket_path))
        with monitor.makefile("rwb", buffering=0) as stream:
            assert "QMP" in json.loads(stream.readline())
            qmp(stream, "qmp_capabilities")
            qmp(stream, "set_link", name="guestnic", up=False)
            qmp(stream, "cont")
            notice("boot", {"kvm": qmp(stream, "query-kvm")["enabled"], "disk": "new_qcow2"})
            start = time.monotonic()
            preflight = False
            done = 0
            while time.monotonic() - start < 1000:
                if receipts.exists():
                    raw = receipts.read_bytes()
                    lines = raw.split(b"\n")[:-1]
                    for line in lines[done:]:
                        if not line.startswith(b"PPAUDIT "):
                            continue
                        _, event, payload = line.split()
                        result = json.loads(b64decode(payload, validate=True))
                        notice(event.decode("ascii"), result)
                        if event == b"before":
                            assert not preflight and result["preflight"]["status"] == "ready"
                            preflight = True
                            qmp(stream, "set_link", name="guestnic", up=True)
                        if event == b"complete":
                            notice("duration", {"guest_seconds": round(time.monotonic() - start, 3)})
                            return result["status"] == "pass"
                    done = len(lines)
                if process.poll() is not None:
                    raise RuntimeError("vm_exited_before_receipt")
                time.sleep(1)
            raise RuntimeError("guest_timeout_after_baseline" if preflight else "guest_timeout_before_baseline")


with subprocess.Popen(guest) as process:
    try:
        passed = main(process)
    except (RuntimeError, ValueError, OSError, AssertionError) as exc:
        notice("host problem", {"code": str(exc) if isinstance(exc, RuntimeError) else type(exc).__name__})
        passed = False
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=8)
            except subprocess.TimeoutExpired:
                process.kill()
if not passed:
    raise SystemExit(1)
