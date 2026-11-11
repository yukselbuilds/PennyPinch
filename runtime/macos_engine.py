"""Pinned macOS engine bootstrap with a parent-pipe lifetime and inherited state lock."""

import argparse
import hashlib
import hmac
import json
import os
from pathlib import Path
import platform
import signal
import site
import stat
import sys
import threading


class StartupError(RuntimeError):
    pass


def unique_keys(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise StartupError("duplicate_start_key")
        value[key] = item
    return value


def start_request(stream):
    raw = stream.readline(4097)
    if len(raw) > 4096 or not raw.endswith(b"\n"):
        raise StartupError("invalid_start_frame")
    try:
        value = json.loads(raw, object_pairs_hook=unique_keys)
    except (ValueError, UnicodeError):
        raise StartupError("invalid_start_json") from None
    if (
        not isinstance(value, dict)
        or set(value) != {"schema_version", "generation", "secret", "protected_tools"}
        or type(value["schema_version"]) is not int
        or value["schema_version"] != 1
    ):
        raise StartupError("invalid_start_shape")
    for name in ("generation", "secret"):
        text = value[name]
        if (
            not isinstance(text, str)
            or len(text) != 32
            or any(char not in "0123456789abcdef" for char in text)
            or int(text, 16) == 0
        ):
            raise StartupError("invalid_start_identity")
    tools = value["protected_tools"]
    if (
        not isinstance(tools, list)
        or len(tools) > 32
        or any(
            not isinstance(name, str)
            or not 1 <= len(name) <= 96
            or any(not (char.isascii() and (char.isalnum() or char in "_-.:/")) for char in name)
            for name in tools
        )
        or tools != sorted(set(tools))
    ):
        raise StartupError("invalid_protected_tools")
    return value


def private_directory(path):
    info = path.lstat()
    if (
        not path.is_absolute()
        or path.resolve(strict=True) != path
        or not stat.S_ISDIR(info.st_mode)
        or info.st_uid != os.geteuid()
        or stat.S_IMODE(info.st_mode) != 0o700
    ):
        raise StartupError("invalid_private_state")
    return path


def verify_lock(state, lock_fd):
    import fcntl

    inherited = os.fstat(lock_fd)
    named = (state / ".engine.lock").lstat()
    if (
        not stat.S_ISREG(inherited.st_mode)
        or inherited.st_uid != os.geteuid()
        or inherited.st_nlink != 1
        or stat.S_IMODE(inherited.st_mode) != 0o600
        or (inherited.st_dev, inherited.st_ino) != (named.st_dev, named.st_ino)
    ):
        raise StartupError("invalid_state_lock")
    fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)


def watch_parent(stream):
    stream.read(1)
    os.kill(os.getpid(), signal.SIGINT)
    # A stuck extension or drain must not leave an orphan holding the state lock.
    threading.Event().wait(15)
    os._exit(1)


def install_private_dashboard(secret):
    import headroom.proxy.server as server
    from headroom.proxy.loopback_guard import is_loopback_host_header

    original = server._request_is_loopback

    def private_request(request):
        if request.client is None and request.scope.get("path") in {"/health", "/stats"}:
            supplied = request.headers.get("x-headroom-desktop-control", "")
            try:
                allowed = hmac.compare_digest(supplied.encode("latin1"), secret.encode("ascii"))
            except (UnicodeError, AttributeError):
                allowed = False
            if allowed and is_loopback_host_header(request.headers.get("host")):
                return True
        return original(request)

    server._request_is_loopback = private_request


def run(args):
    if sys.platform != "darwin" or platform.machine() != "arm64" or sys.version_info[:3] != (3, 13, 13):
        raise StartupError("unsupported_runtime")
    state = private_directory(args.state)
    endpoint = private_directory(args.socket_dir) / "s"
    if endpoint.exists() or endpoint.is_symlink() or len(os.fsencode(endpoint)) >= 104:
        raise StartupError("invalid_socket_path")
    verify_lock(state, args.state_lock_fd)
    with (args.runtime / "manifest.json").open("rb") as stream:
        manifest_bytes = stream.read(16 * 1024 * 1024 + 1)
    if len(manifest_bytes) > 16 * 1024 * 1024 or hashlib.sha256(manifest_bytes).hexdigest() != args.manifest_sha256:
        raise StartupError("runtime_manifest_changed")
    manifest = json.loads(manifest_bytes, object_pairs_hook=unique_keys)
    if manifest["engine_version"] != "0.41.0" or manifest["python_version"] != "3.13.13":
        raise StartupError("runtime_pin_mismatch")
    request = start_request(sys.stdin.buffer)
    threading.Thread(target=watch_parent, args=(sys.stdin.buffer,), daemon=True).start()
    for name in ("home", "temp", "workspace", "config", "cache"):
        private_directory(state / name)
    payload = args.runtime / "payload"
    site.addsitedir(str(payload / "site-packages"))
    from importlib.metadata import version

    if version("headroom-ai") != "0.41.0":
        raise StartupError("engine_version_mismatch")
    os.environ.update({
        "HOME": str(state / "home"),
        "TMPDIR": str(state / "temp"),
        "XDG_CACHE_HOME": str(state / "cache"),
        "HEADROOM_WORKSPACE_DIR": str(state / "workspace"),
        "HEADROOM_CONFIG_DIR": str(state / "config"),
        "HEADROOM_MODE": "cache",
        "HEADROOM_SAVINGS_PROFILE": "coding",
        "HEADROOM_CCR_TTL_SECONDS": "7200",
        "HEADROOM_TELEMETRY": "on",
        "HEADROOM_BEACON": "off",
        "HEADROOM_LOG_MESSAGES": "off",
        "HEADROOM_UPDATE_CHECK": "off",
        "HEADROOM_MODEL_ROUTER_ENABLED": "0",
        "HEADROOM_OUTPUT_SHAPER": "0",
        "HEADROOM_PROTECT_TOOL_RESULTS": ",".join(request["protected_tools"]),
    })
    sys.path.insert(0, str(args.runtime))
    import cache_control_shim
    import optimization_control
    from headroom.proxy.model_router import ModelRouterConfig

    if ModelRouterConfig.from_env("0", None).enabled:
        raise StartupError("model_router_enabled")
    shim = cache_control_shim.install()
    if shim.get("active") != "true" or shim.get("version") != "0.41.0":
        raise StartupError("cache_shim_inactive")
    optimization_control.install(request["generation"], request["secret"])
    install_private_dashboard(request["secret"])
    from headroom.cli import main

    sys.argv = ["headroom", "proxy", "--uds", str(endpoint), "--workers", "1",
                "--no-learn", "--no-memory-tools", "--no-memory-context"]
    main()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--state", type=Path, required=True)
    parser.add_argument("--socket-dir", type=Path, required=True)
    parser.add_argument("--state-lock-fd", type=int, required=True)
    parser.add_argument("--manifest-sha256", required=True)
    arguments = parser.parse_args()
    try:
        run(arguments)
    except KeyboardInterrupt:
        pass
    except StartupError as error:
        sys.exit(f"headroom-macos-error={error}")
    except Exception as error:
        sys.exit(f"headroom-macos-error={type(error).__name__}")
