"""No-account Desktop preflight; this is not a live response/compatibility claim."""

import importlib.util
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import re
import selectors
import shutil
import stat
import subprocess
import sys
import tempfile
import threading
import time


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("_headroom_owned_package", ROOT / "scripts/probe-macos-package.py")
core = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(core)
CDP_SPEC = importlib.util.spec_from_file_location("_headroom_desktop_cdp", ROOT / "scripts/macos-desktop-cdp.py")
cdp = importlib.util.module_from_spec(CDP_SPEC)
CDP_SPEC.loader.exec_module(cdp)


def config_observation(reply, endpoint):
    core.require(isinstance(reply, dict) and "error" not in reply
                 and isinstance(reply.get("result"), dict), "config_read_failed")
    config = reply["result"].get("config")
    core.require(isinstance(config, dict), "config_read_failed")
    core.require(config.get("openai_base_url") == endpoint, "isolated_endpoint_not_read")
    core.require(config.get("model_provider") == "openai", "provider_changed")
    return {"endpoint_matches": True, "built_in_provider": True}


def isolated_environment(state):
    return {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin", "HOME": str(state / "home"),
            "TMPDIR": str(state / "temp"), "CODEX_HOME": str(state / "codex"),
            "CODEX_ELECTRON_USER_DATA_PATH": str(state / "electron")}


def digest(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def profile_metadata(path):
    try:
        info = path.lstat()
    except FileNotFoundError:
        return {"kind": "absent"}
    if not stat.S_ISDIR(info.st_mode):
        return {"kind": "not_directory"}
    known = {"Crashpad", "GPUCache", "DawnGraphiteCache", "DawnWebGPUCache", "Cache",
             "Local State", "Preferences", "Default", "Local Storage", "Session Storage",
             "IndexedDB", "Network", "config.toml", "auth.json", "log", "logs",
             "state_5.sqlite", "state_6.sqlite", "bin", "models_cache.json"}
    observed, other, count = set(), 0, 0
    with os.scandir(path) as entries:
        for item in entries:
            count += 1
            if count > 64:
                break
            if item.name in known:
                observed.add(item.name)
            else:
                other += 1
    return {"kind": "directory", "known_entries": sorted(observed), "other_entries": other,
            "truncated": count > 64}


def rpc(process, request, selector):
    process.stdin.write(json.dumps(request, separators=(",", ":")).encode() + b"\n")
    process.stdin.flush()
    deadline, data, total = time.monotonic() + 20, b"", 0
    while time.monotonic() < deadline:
        if not selector.select(timeout=0.2):
            core.require(process.poll() is None, "backend_exited_before_reply")
            continue
        block = os.read(process.stdout.fileno(), 65536)
        core.require(bool(block), "backend_stdout_closed")
        total += len(block)
        data += block
        core.require(total <= 2 * 1024 * 1024 and len(data) <= 512 * 1024, "rpc_output_limit")
        while b"\n" in data:
            line, data = data.split(b"\n", 1)
            value = json.loads(line)
            core.require(isinstance(value, dict), "invalid_rpc_reply")
            if value.get("id") == request["id"]:
                core.require("error" not in value, "rpc_error")
                return value
    raise core.ProbeError("rpc_reply_timeout")


def backend_config(backend, environment, state, endpoint):
    process = subprocess.Popen([str(backend), "app-server"], cwd=state, env=environment,
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    try:
        with selectors.DefaultSelector() as selector:
            selector.register(process.stdout, selectors.EVENT_READ)
            rpc(process, {"id": 1, "method": "initialize", "params": {
                "clientInfo": {"name": "headroom_mac_preflight", "version": "1"},
                "capabilities": {"experimentalApi": True}}}, selector)
            process.stdin.write(b'{"method":"initialized"}\n')
            process.stdin.flush()
            observed = config_observation(rpc(process, {"id": 2, "method": "config/read",
                "params": {"includeLayers": False}}, selector), endpoint)
            account = rpc(process, {"id": 3, "method": "account/read",
                "params": {"refreshToken": False}}, selector)
            core.require(account["result"].get("account") is None, "unexpected_account")
        process.stdin.close()
        core.require(process.wait(timeout=10) == 0, "backend_unclean_exit")
        return {**observed, "account": "none", "normal_exit": True}
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)


class RejectRequests(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def reject(self):
        self.server.observed_requests += 1
        self.send_response(401)
        self.send_header("Content-Length", "0")
        self.send_header("Connection", "close")
        self.end_headers()

    do_GET = reject
    do_POST = reject


def gui_state(action, pid):
    result = subprocess.run(["/usr/bin/osascript", str(ROOT / "scripts/macos-desktop-input.applescript"),
                             action, str(pid)], capture_output=True, text=True, timeout=20)
    core.require(result.returncode == 0, "desktop_ui_observation_failed")
    return json.loads(result.stdout) if action == "observe" else None


def owned_descendants(app, bundle):
    table = core.process_table()
    parents, owned = {app[0]}, []
    for _ in range(8):
        found = set()
        for pid, stamp in table.items():
            if stamp[0] in parents and pid not in parents:
                executable = core.executable_path(pid)
                if executable and executable.is_relative_to(bundle):
                    found.add(pid)
                    owned.append((pid, stamp, executable))
        if not found:
            break
        parents.update(found)
    return owned


def run(root):
    core.require(sys.platform == "darwin" and os.environ.get("GITHUB_ACTIONS") == "true", "ci_macos_required")
    core.require(root.is_absolute() and root.resolve(strict=True) == root
                 and root.parent == (ROOT / "work").resolve(strict=True)
                 and root.name.startswith("macos-desktop."), "invalid_desktop_root")
    core.require(root.stat().st_uid == os.geteuid() and stat.S_IMODE(root.stat().st_mode) == 0o700,
                 "invalid_desktop_owner")
    bundle = root / "home/Applications/ChatGPT.app"
    launcher = (bundle / "Contents/MacOS/ChatGPT").resolve(strict=True)
    backend = (bundle / "Contents/Resources/codex-cli/CodexCLI.app/Contents/MacOS/codex").resolve(strict=True)
    files = [bundle / "Contents/Info.plist", launcher, backend, bundle / "Contents/Resources/app.asar"]
    before = {path.relative_to(bundle).as_posix(): digest(path) for path in files}
    signature = subprocess.run(["/usr/bin/codesign", "--verify", "--deep", "--strict", str(bundle)],
                               capture_output=True, timeout=90)
    core.require(signature.returncode == 0, "desktop_full_signature_invalid")
    print("headroom-desktop-phase=full_signature_verified", flush=True)
    core.require(core.find_app(launcher) is None, "desktop_already_open")
    normal_home = Path.home()
    untouched = [normal_home / ".codex", *[normal_home / "Library/Application Support" / name
                 for name in ("Codex", "ChatGPT", "com.openai.codex")]]
    core.require(not any(path.exists() for path in untouched), "runner_default_profile_not_empty")
    # Explicit, short, owned CI app state avoids macOS's Unix-socket path limit.
    state = Path(tempfile.mkdtemp(prefix="hr-c7-", dir="/private/tmp")).resolve(strict=True)
    identity = (state.stat().st_dev, state.stat().st_ino)
    for name in ("home", "temp", "codex", "electron"):
        (state / name).mkdir(mode=0o700)
    environment = isolated_environment(state)
    server = ThreadingHTTPServer(("127.0.0.1", 0), RejectRequests)
    server.daemon_threads = True
    server.observed_requests = 0
    serving = threading.Thread(target=server.serve_forever, daemon=True)
    serving.start()
    endpoint = f"http://127.0.0.1:{server.server_address[1]}/v1"
    config = state / "codex/config.toml"
    config.write_text(f'openai_base_url = "{endpoint}"\nmodel_provider = "openai"\ncli_auth_credentials_store = "file"\n'
                      '[analytics]\nenabled = false\n', encoding="utf-8")
    config.chmod(0o600)
    original_config = digest(config)
    owned, opening, descendants, vendor_pin = None, None, [], None
    renderer = None
    try:
        version = subprocess.run([str(backend), "--version"], env=environment, cwd=state,
                                 capture_output=True, text=True, timeout=10)
        core.require(version.returncode == 0 and len(version.stdout) < 256
                     and re.fullmatch(r"codex(?:-cli)? 0\.167\.0-alpha\.4\.1\s*", version.stdout),
                     "unexpected_backend_version")
        contract = backend_config(backend, environment, state, endpoint)
        print("headroom-desktop-phase=isolated_backend_config_read", flush=True)
        args = ["/usr/bin/open", "-n", "-W"]
        for name, value in environment.items():
            args.extend(["--env", f"{name}={value}"])
        args.extend(["-a", str(bundle), "--args", f'--user-data-dir={state / "electron"}',
                     "--force-renderer-accessibility"])
        if os.environ.get("HEADROOM_DESKTOP_RENDERER_PROBE") == "true":
            args.extend(["--remote-debugging-address=127.0.0.1", "--remote-debugging-port=0"])
        opening = subprocess.Popen(args, cwd=state, env=environment, stdin=subprocess.DEVNULL,
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        owned = core.wait_for(lambda: core.find_app(launcher), "desktop_open_timeout", seconds=30)
        print("headroom-desktop-phase=owned_desktop_opened", flush=True)
        observed = core.wait_for(lambda: (value if value["windows"] > 0 else None)
            if (value := gui_state("observe", owned[0])) else None, "desktop_window_timeout", seconds=45)
        time.sleep(3)
        observed = gui_state("observe", owned[0])
        print("headroom-desktop-gui=" + json.dumps(observed, separators=(",", ":")), flush=True)
        core.require(observed["windows"] > 0, "desktop_window_disappeared")
        if os.environ.get("HEADROOM_DESKTOP_RENDERER_PROBE") == "true":
            import site

            site.addsitedir(str(Path(os.environ["HEADROOM_MACOS_RUNTIME_ROOT"]) / "payload/site-packages"))
            page = cdp.Page(state / "electron", owned[0])
            try:
                deadline = time.monotonic() + 45
                while True:
                    renderer = page.observe()
                    if renderer["editors"] > 0 or renderer["known_buttons"] or time.monotonic() >= deadline:
                        break
                    time.sleep(0.5)
                print("headroom-desktop-renderer=" + json.dumps(renderer, separators=(",", ":")), flush=True)
            finally:
                page.close()
        descendants = owned_descendants(owned, bundle)
        children = [child for child in descendants if child[2] == backend]
        gui_state("quit", owned[0])
        core.wait_for(lambda: not core.matches_process(owned), "desktop_quit_timeout", seconds=20)
        for child in descendants:
            core.wait_for(lambda: not core.matches_process(child), "desktop_child_alive", seconds=15)
        core.require(opening.wait(timeout=10) == 0, "desktop_launcher_unclean_exit")
        core.require(digest(config) == original_config, "desktop_changed_endpoint_config")
        profiles = {key: profile_metadata(path) for key, path in zip(
            ("codex_home", "codex_app", "chatgpt_app", "bundle_app"), untouched)}
        profiles["private_electron"] = profile_metadata(state / "electron")
        print("headroom-desktop-profiles=" + json.dumps(profiles, separators=(",", ":")), flush=True)
        core.require(not any(path.exists() or path.is_symlink() for path in untouched[:3]),
                     "desktop_wrote_default_profile")
        core.require((state / "electron/Local State").is_file()
                     and (state / "electron/Default").is_dir(), "electron_profile_not_used")
        vendor = untouched[3]
        if vendor.exists() or vendor.is_symlink():
            info = vendor.lstat()
            core.require(vendor.resolve(strict=True) == vendor and stat.S_ISDIR(info.st_mode)
                         and info.st_uid == os.geteuid() and info.st_mode & 0o022 == 0,
                         "unsafe_vendor_application_state")
            vendor_pin = (info.st_dev, info.st_ino)
        core.require(all(digest(path) == before[path.relative_to(bundle).as_posix()] for path in files),
                     "desktop_package_changed")
        result = {"schema_version": 1, "status": "pass", "scope": "desktop_no_account_preflight",
                  "desktop_version": "26.1106.51428", "backend_version": version.stdout.strip(),
                  "full_signature_verified": True, "backend_config": contract, "gui": observed,
                  "bundled_backend_children_observed": len(children), "isolated_electron_state_used": True,
                  "default_core_profiles_unchanged": True, "normal_exit": True,
                  "vendor_application_state_created": vendor_pin is not None,
                  "vendor_application_state_purpose": "unknown",
                  "full_application_state_isolation": "not_established",
                  "observed_owned_children_exited": len(descendants),
                  "endpoint_config_unchanged": True, "observed_loopback_requests": server.observed_requests,
                  "model_turn_requested": False, "live_provider_traffic": "not_tested",
                  "desktop_headroom_route": "not_tested", "package_before": before,
                  "package_after": {path.relative_to(bundle).as_posix(): digest(path) for path in files}}
        if renderer is not None:
            result["renderer"] = renderer
    finally:
        if owned and core.matches_process(owned):
            known = {item[0] for item in descendants}
            descendants.extend(item for item in owned_descendants(owned, bundle) if item[0] not in known)
        for item in ([owned] if owned else []) + list(reversed(descendants)):
            if core.matches_process(item):
                os.kill(item[0], 15)
                try:
                    core.wait_for(lambda: not core.matches_process(item), "cleanup_timeout", seconds=5)
                except core.ProbeError:
                    if core.matches_process(item):
                        os.kill(item[0], 9)
                        core.wait_for(lambda: not core.matches_process(item), "forced_cleanup_timeout", seconds=5)
        if opening and opening.poll() is None:
            opening.terminate()
            opening.wait(timeout=5)
        server.shutdown()
        server.server_close()
        serving.join(timeout=5)
        if vendor_pin is None and (untouched[3].exists() or untouched[3].is_symlink()):
            vendor = untouched[3]
            info = vendor.lstat()
            core.require(vendor.resolve(strict=True) == vendor and stat.S_ISDIR(info.st_mode)
                         and info.st_uid == os.geteuid() and info.st_mode & 0o022 == 0,
                         "unsafe_vendor_application_state")
            vendor_pin = (info.st_dev, info.st_ino)
        if vendor_pin:
            vendor = untouched[3]
            info = vendor.lstat()
            core.require(vendor.parent == normal_home / "Library/Application Support"
                         and vendor.name == "com.openai.codex" and vendor.resolve(strict=True) == vendor
                         and stat.S_ISDIR(info.st_mode) and info.st_uid == os.geteuid()
                         and info.st_mode & 0o022 == 0
                         and (info.st_dev, info.st_ino) == vendor_pin, "vendor_state_cleanup_conflict")
            shutil.rmtree(vendor)
        current = state.lstat()
        core.require(state.parent == Path("/private/tmp") and state.name.startswith("hr-c7-")
                     and state.resolve(strict=True) == state and current.st_uid == os.geteuid()
                     and stat.S_IMODE(current.st_mode) == 0o700
                     and (current.st_dev, current.st_ino) == identity, "private_state_cleanup_conflict")
        # rmtree does not follow profile symlinks; the newly created root is checked above.
        shutil.rmtree(state)
    result["private_state_removed"] = not state.exists()
    result["vendor_application_state_removed"] = not untouched[3].exists()
    core.require(result["private_state_removed"], "private_state_left")
    core.require(result["vendor_application_state_removed"], "vendor_state_left")
    print("headroom-desktop-preflight=" + json.dumps(result, separators=(",", ":")), flush=True)


if __name__ == "__main__":
    try:
        core.require(len(sys.argv) == 2, "desktop_root_required")
        run(Path(sys.argv[1]))
    except core.ProbeError as error:
        print(f"headroom-desktop-error={error}", file=sys.stderr)
        sys.exit(2)
    except cdp.CdpError as error:
        print(f"headroom-desktop-error={error}", file=sys.stderr)
        sys.exit(2)
    except (OSError, ValueError, KeyError, subprocess.SubprocessError):
        print("headroom-desktop-error=preflight_failed", file=sys.stderr)
        sys.exit(2)
