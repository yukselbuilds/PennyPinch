#!/usr/bin/env python3
"""Check the installed PennyPinch window in a private Ubuntu test session."""

from importlib.util import module_from_spec, spec_from_file_location
import json
import os
from pathlib import Path
import re
import socket
import stat
import struct
import subprocess
import sys
import time
import traceback


APP = Path("/usr/bin/pennypinch")
ENGINE = Path("/usr/lib/PennyPinch/runtime/payload/python/bin/python3.13")
START = ("Başlat", "Start")
OPTIONS = ("Motor seçenekleri", "Engine options")
STOP = ("Motoru durdur", "Stop engine")
EXIT = ("PennyPinch’ten çık", "Exit PennyPinch")
CONFIRM = ("Çıkışı onayla", "Confirm exit")


class ProbeError(Exception):
    pass


def require(condition, code):
    if not condition:
        raise ProbeError(code)


def command(*args):
    return subprocess.run(
        args, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=5
    )


def wait_for(read, code, seconds, app=None):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if app is not None:
            require(app.poll() is None, "app_exited_early")
        if result := read():
            return result
        time.sleep(0.2)
    raise ProbeError(code)


def private_dir(path):
    info = path.lstat()
    require(
        stat.S_ISDIR(info.st_mode)
        and info.st_uid == os.geteuid()
        and stat.S_IMODE(info.st_mode) == 0o700
        and path.resolve(strict=True) == path,
        "unsafe_private_directory",
    )


def validate_profile(path):
    work = Path(__file__).resolve().parent.parent.joinpath("work").resolve(strict=True)
    require(path.is_absolute() and work in path.parents, "profile_outside_work")
    private_dir(path)
    for env, name in (
        ("HOME", "home"), ("XDG_DATA_HOME", "data"), ("XDG_CONFIG_HOME", "config"),
        ("XDG_CACHE_HOME", "cache"), ("XDG_RUNTIME_DIR", "session"), ("TMPDIR", "tmp"),
    ):
        directory = path / name
        require(os.environ.get(env) == str(directory), "unexpected_profile_environment")
        private_dir(directory)
    links = command("ip", "-j", "link", "show")
    require(
        links.returncode == 0 and [link["ifname"] for link in json.loads(links.stdout)] == ["lo"],
        "network_is_not_loopback_only",
    )


def socket_reader():
    path = Path(__file__).with_name("probe-linux-engine.py")
    spec = spec_from_file_location("probe_linux_engine", path)
    module = module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.owned_get


def process_identity(pid):
    try:
        root = Path("/proc") / str(pid)
        fields = (root / "stat").read_text().rsplit(")", 1)[1].split()
        return (int(fields[1]), int(fields[19]), (root / "exe").resolve(strict=True))
    except (OSError, IndexError, ValueError):
        return None


def sockets(state, session):
    return sorted(state.glob("e-*/s")) + sorted(session.glob("hr-*/s"))


def ready_engine(app, state, session, owned_get):
    for path in sockets(state, session):
        private_dir(path.parent)
        try:
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as peer:
                peer.settimeout(1)
                peer.connect(str(path))
                pid, uid, _ = struct.unpack("iII", peer.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
            identity = process_identity(pid)
            if identity is None:
                continue
            require(
                uid == os.geteuid() and identity[0] == app.pid and identity[2] == ENGINE.resolve(strict=True),
                "engine_peer_mismatch",
            )
            row = owned_get(path, pid, "/readyz")
        except OSError:
            continue
        if row.get("ready") is True:
            require(row.get("service") == "headroom-proxy" and row.get("version") == "0.44.0", "engine_version_mismatch")
            return path, pid, identity
    return None


def endpoint_port(state):
    endpoint = state / "connection-endpoint.json"
    row = json.loads(endpoint.read_bytes())
    port = row["port"]
    require(row["bind_address"] == "127.0.0.1" and type(port) is int and 1 <= port <= 65535, "invalid_ingress")
    return port


def listeners(port):
    info = command("ss", "-H", "-ltnp", f"sport = :{port}")
    require(info.returncode == 0, "ingress_observation_failed")
    return info.stdout.splitlines()


def check_listener(pid, port):
    rows = listeners(port)
    require(
        len(rows) == 1
        and rows[0].split()[3] == f"127.0.0.1:{port}"
        and re.search(rf"\bpid={pid},", rows[0]) is not None,
        "ingress_owner_or_address_mismatch",
    )


def visible_window(app):
    found = command("xdotool", "search", "--onlyvisible", "--all", "--pid", str(app.pid), "--name", "^PennyPinch$")
    return found.returncode == 0 and bool(found.stdout.strip())


def controls(atspi, app, labels):
    desktop = atspi.Registry.getDesktop(0)
    try:
        stack = [candidate for candidate in desktop if candidate.get_process_id() == app.pid]
        remaining = 3000
        while stack and remaining:
            remaining -= 1
            node = stack.pop()
            if node.name in labels:
                state = node.getState()
                if state.contains(atspi.STATE_SHOWING) and state.contains(atspi.STATE_ENABLED):
                    try:
                        action = node.queryAction()
                    except NotImplementedError:
                        action = None
                    if action is not None and action.nActions > 0:
                        return action
            stack.extend(reversed(tuple(node)))
    except (RuntimeError, AttributeError):
        return None
    return None


def click(atspi, app, labels, code):
    action = wait_for(lambda: controls(atspi, app, labels), code, 10, app)
    require(action.doAction(0), code)


def check_lifecycle(atspi, profile):
    owned_get = socket_reader()
    state = profile / "data/dev.headroom.desktop/runtime"
    session = profile / "session"
    with subprocess.Popen(
        [APP], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
    ) as app:
        try:
            wait_for(lambda: visible_window(app), "visible_window_timeout", 30, app)
            wait_for(lambda: (state / "connection-endpoint.json").is_file(), "ingress_record_timeout", 10, app)
            private_dir(state)
            port = endpoint_port(state)
            check_listener(app.pid, port)
            print("headroom-linux-package: visible window", flush=True)
            click(atspi, app, START, "start_control_unavailable")
            first = wait_for(lambda: ready_engine(app, state, session, owned_get), "start_readiness_timeout", 40, app)
            check_listener(app.pid, port)
            print("headroom-linux-package: start ready", flush=True)

            click(atspi, app, OPTIONS, "engine_options_unavailable")
            click(atspi, app, STOP, "stop_control_unavailable")
            path, pid, identity = first
            wait_for(lambda: not path.exists() and process_identity(pid) != identity, "engine_stop_timeout", 25, app)
            require(visible_window(app) and endpoint_port(state) == port, "app_lost_after_stop")
            check_listener(app.pid, port)

            click(atspi, app, START, "restart_control_unavailable")
            second = wait_for(lambda: ready_engine(app, state, session, owned_get), "restart_readiness_timeout", 40, app)
            require(first[:2] != second[:2] and endpoint_port(state) == port, "restart_identity_mismatch")
            check_listener(app.pid, port)
            print("headroom-linux-package: stop and restart verified", flush=True)

            click(atspi, app, EXIT, "exit_control_unavailable")
            click(atspi, app, CONFIRM, "exit_confirmation_unavailable")
            require(app.wait(timeout=25) == 0, "nonzero_app_exit")
            path, pid, identity = second
            wait_for(
                lambda: not path.exists() and not sockets(state, session)
                and process_identity(pid) != identity and not listeners(port),
                "exit_cleanup_timeout", 8,
            )
        finally:
            if app.poll() is None:
                app.terminate()
                try:
                    app.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    app.kill()
                    app.wait(timeout=5)


def main():
    try:
        require(sys.platform == "linux" and os.geteuid() != 0 and len(sys.argv) == 2, "invalid_probe_environment")
        profile = Path(sys.argv[1])
        validate_profile(profile)
        import pyatspi
        check_lifecycle(pyatspi, profile)
    except ProbeError as error:
        print(f"headroom-linux-package-error={error}", flush=True)
        return 1
    except Exception as error:
        probe_lines = [line for frame, line in traceback.walk_tb(error.__traceback__)
                       if frame.f_code.co_filename == __file__]
        location = probe_lines[-1] if probe_lines else 0
        print(f"headroom-linux-package-error={type(error).__name__} check_line={location}", flush=True)
        return 1
    print("headroom-linux-package-result=pass", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
