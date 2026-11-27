from base64 import b64decode
import json
from pathlib import Path
import re
import socket
import subprocess
import time

socket_path = Path("qmp.socket").resolve()
receipts = Path("guest.serial")
BOOT_LIMIT_SECONDS = 810
INSTALLED_TEST_LIMIT_SECONDS = 925
ANNOTATED_EVENTS = {"boot", "setup", "before", "runtime", "installed", "tray", "native input", "complete", "duration", "host problem"}
guest = [
    "qemu-system-x86_64", "-enable-kvm", "-machine", "pc", "-cpu", "host",
    "-smp", "4", "-m", "8192", "-S", "-boot", "order=c,once=d",
    "-display", "none", "-vga", "std", "-rtc", "base=localtime", "-usb", "-device", "usb-tablet",
    "-drive", "file=clean.qcow2,format=qcow2,index=0",
    "-drive", "file=windows.iso,format=raw,media=cdrom,index=2,readonly=on",
    "-drive", "file=answer.iso,format=raw,media=cdrom,index=3,readonly=on",
    "-netdev", "user,id=nat", "-device", "e1000,netdev=nat,id=guestnic",
    "-serial", "file:guest.serial", "-qmp", f"unix:{socket_path},server=on,wait=off",
]


def notice(name, data):
    data = json.dumps(data, separators=(",", ":"), ensure_ascii=True)
    if name in ANNOTATED_EVENTS:
        print(f"::notice title=H037 {name}::{data}", flush=True)
    else:
        print(f"H037 {name}: {data}", flush=True)


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


def validate_pointer(row, screen):
    if row.get("event") != "pointer" or row.get("phase") not in {"tray_open", "tray_pause", "tray_resume", "tray_quit"}:
        raise RuntimeError("unexpected_pointer_phase")
    width, height = screen
    x, y = row.get("x"), row.get("y")
    if (row.get("width"), row.get("height")) != screen or not (640 <= width <= 3840 and 480 <= height <= 2160):
        raise RuntimeError("pointer_screen_size_mismatch")
    if (type(x) is not int or type(y) is not int or not (0 <= x < width and 0 <= y < height)
            or type(row.get("right")) is not bool):
        raise RuntimeError("pointer_position_outside_guest")
    return round(32767 * x / (width - 1)), round(32767 * y / (height - 1))


def screen_size(stream):
    path = Path("pointer.ppm").resolve()
    qmp(stream, "screendump", filename=str(path))
    with path.open("rb") as image:
        header = image.read(80)
    match = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", header)
    if not match:
        raise RuntimeError("invalid_guest_frame")
    return int(match[1]), int(match[2])


def send_pointer(stream, row, counter):
    phase = row.get("phase")
    count = counter.setdefault(phase, {"left": 0, "right": 0})
    button = "right" if row.get("right") is True else "left"
    if count[button] >= (1 if button == "right" else 2):
        raise RuntimeError("unexpected_number_of_tray_clicks")
    x, y = validate_pointer(row, screen_size(stream))
    if sum(sum(counts.values()) for counts in counter.values()) == 0:
        tablets = [item for item in qmp(stream, "query-mice") if item["absolute"]]
        if len(tablets) != 1:
            raise RuntimeError("guest_tablet_missing")
        if not tablets[0]["current"]:
            index = tablets[0]["index"]
            assert type(index) is int and 0 <= index <= 31
            qmp(stream, "human-monitor-command", **{"command-line": f"mouse_set {index}"})
    qmp(stream, "input-send-event", events=[
        {"type": "abs", "data": {"axis": "x", "value": x}},
        {"type": "abs", "data": {"axis": "y", "value": y}},
    ])
    time.sleep(0.09)
    for down in (True, False):
        qmp(stream, "input-send-event", events=[{"type": "btn", "data": {"button": button, "down": down}}])
        time.sleep(0.09)
    count[button] += 1


def setup_display(stream):
    # Keep the screenshot in the disposable runner. Report only recognized installation states.
    states = {
        "press any key to boot": "boot_prompt",
        "no bootable device": "no_boot_device",
        "no bootable disk": "no_boot_device",
        "installing windows": "installing",
        "where do you want to install windows": "disk_selection",
        "windows setup": "setup",
        "could not parse or process": "answer_file_error",
        "let's connect you to a network": "oobe_network",
        "sign in": "sign_in",
        "getting ready": "getting_ready",
        "getting devices ready": "getting_devices_ready",
        "pennypinch": "application_title",
        "procedure entry point": "missing_entry_point",
        "could not be located": "missing_entry_point",
        "system error": "system_error",
        "application error": "application_error",
        "error": "error_text",
    }
    try:
        qmp(stream, "screendump", filename=str(Path("setup.ppm").resolve()))
        scan = subprocess.run(["tesseract", "setup.ppm", "stdout"], capture_output=True, check=True, timeout=10)
        screen = " ".join(scan.stdout.decode("utf-8", errors="replace").lower().split())
    except (RuntimeError, subprocess.SubprocessError):
        return ["diagnostic_unavailable"]
    result = sorted({value for key, value in states.items() if key in screen})
    if "installing" in result:
        percentage = re.search(r"(\d{1,3})\s*%", screen)
        if percentage and 0 <= int(percentage[1]) <= 100:
            result.append(f"{percentage[1]}_percent")
    return result or ["unclassified"]


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
            preflight = None
            setup_samples = []
            diagnostic = iter([4, 11, 28, 90, 220, 380, 540, 670, 800])
            next_diagnostic = next(diagnostic)
            done = 0
            input_counts = {}
            while True:
                elapsed = time.monotonic() - start
                if not preflight and elapsed >= next_diagnostic:
                    states = setup_display(stream)
                    setup_samples.append({"seconds": next_diagnostic, "screen": states})
                    notice(f"boot at {next_diagnostic} seconds", setup_samples[-1])
                    if "boot_prompt" in states:
                        qmp(stream, "human-monitor-command", **{"command-line": "sendkey ret"})
                    next_diagnostic = next(diagnostic, float("inf"))
                if receipts.exists():
                    raw = receipts.read_bytes()
                    lines = raw.split(b"\n")[:-1]
                    for line in lines[done:]:
                        if not line.startswith(b"PPAUDIT "):
                            continue
                        _, event, payload = line.split()
                        result = json.loads(b64decode(payload, validate=True))
                        if event == b"pointer":
                            if not preflight:
                                raise RuntimeError("pointer_before_guest_ready")
                            send_pointer(stream, result, input_counts)
                            continue
                        notice(event.decode("ascii"), result)
                        if event == b"before":
                            assert not preflight and result["preflight"]["status"] == "ready"
                            notice("setup", {"samples": setup_samples, "preflight_after_seconds": round(elapsed, 3)})
                            preflight = time.monotonic()
                        if event == b"complete":
                            if input_counts:
                                notice("native input", {"method": "qmp_usb_tablet", "clicks": input_counts})
                            notice("duration", {"guest_seconds": round(time.monotonic() - start, 3)})
                            if not preflight:
                                notice("setup", {"samples": setup_samples, "preflight_after_seconds": None})
                            return result["status"] == "pass"
                    done = len(lines)
                if process.poll() is not None:
                    raise RuntimeError("vm_exited_before_receipt")
                if not preflight and elapsed >= BOOT_LIMIT_SECONDS:
                    notice("setup", {"samples": setup_samples, "preflight_after_seconds": None})
                    raise RuntimeError("guest_timeout_before_baseline")
                if preflight and time.monotonic() - preflight >= INSTALLED_TEST_LIMIT_SECONDS:
                    raise RuntimeError("guest_timeout_after_baseline")
                time.sleep(1)


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
