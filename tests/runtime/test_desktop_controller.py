from __future__ import annotations

import hashlib
import json
import os
import queue
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from runtime.desktop_controller import (
    HelperFrameReader,
    ProtocolError,
    decode_request,
    encode_frame,
    validate_runtime_profile,
    verify_runtime_files,
    validate_protected_tools,
)


class _IncompleteHelperStream:
    def __init__(self):
        self._sent = False
        self.closed = False

    def readline(self, _size: int) -> bytes:
        if self._sent:
            return b""
        self._sent = True
        return b'{"protocol_version":1'

    def close(self) -> None:
        self.closed = True


class DesktopProtocolTests(unittest.TestCase):
    def test_protected_tool_boundary_is_bounded_sorted_and_not_a_shell_expression(self):
        self.assertEqual(validate_protected_tools(["ReadExact", "mcp__local__data"]),["ReadExact", "mcp__local__data"])
        for invalid in (None, "ReadExact", [""], ["bad name"], ["é"], ["a", "a"], ["z", "a"], ["x"*97], [f"t{i:02}" for i in range(33)]):
            with self.assertRaises(ProtocolError):
                validate_protected_tools(invalid)

    def test_stalled_helper_frame_has_a_bounded_timeout(self):
        child = subprocess.Popen(
            [sys.executable, "-I", "-c", "import time; time.sleep(30)"],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        )
        self.assertIsNotNone(child.stdout)
        reader = HelperFrameReader(child.stdout)
        try:
            with self.assertRaises(ProtocolError) as raised:
                reader.read_event("g1", "r1", timeout=0.05)
            self.assertEqual(raised.exception.code, "helper_timeout")
        finally:
            if child.poll() is None:
                child.terminate()
                child.wait(timeout=3)
        self.assertTrue(reader.finish_after_process_exit(timeout=2))
        self.assertTrue(child.stdout.closed)

    def test_incomplete_helper_frame_fails_closed(self):
        stream = _IncompleteHelperStream()
        reader = HelperFrameReader(stream)
        with self.assertRaises(ProtocolError) as raised:
            reader.read_event("g1", "r1", timeout=1)
        self.assertEqual(raised.exception.code, "helper_protocol_invalid")
        self.assertTrue(reader.finish_after_process_exit(timeout=1))
        self.assertTrue(stream.closed)

    def test_manifest_profile_is_fixed_and_rejects_drift(self):
        profile = {
            "mode": "cache",
            "savings_profile": "coding",
            "ccr_ttl_seconds": 7200,
            "telemetry": "on",
            "beacon": "off",
            "output_shaper": "0",
            "model_router_enabled": "0",
            "memory": False,
            "learn": False,
        }
        validate_runtime_profile(profile)
        for changed in (
            {**profile, "mode": "passthrough"},
            {**profile, "ccr_ttl_seconds": 1800},
            {**profile, "telemetry": "off"},
            {key: value for key, value in profile.items() if key != "beacon"},
        ):
            with self.subTest(profile=changed), self.assertRaises(ProtocolError):
                validate_runtime_profile(changed)

    def test_start_request_keeps_opaque_id_and_rejects_unknown_fields(self):
        request = decode_request(b'{"protocol_version":1,"request_id":"task/alpha+9","event":"start"}\n')
        self.assertEqual(request["request_id"], "task/alpha+9")
        with self.assertRaises(ProtocolError):
            decode_request(b'{"protocol_version":1,"request_id":"x","event":"start","path":"secret"}\n')

    def test_version_unknown_event_id_and_size_fail_closed(self):
        invalid = (
            b'{"protocol_version":2,"request_id":"x","event":"start"}\n',
            b'{"protocol_version":1,"request_id":"x","event":"launch"}\n',
            b'{"protocol_version":1,"request_id":"bad id","event":"start"}\n',
            b'{"protocol_version":1,"request_id":"x","event":"start"}' + b" " * 4096 + b"\n",
        )
        for payload in invalid:
            with self.subTest(payload_length=len(payload)), self.assertRaises(ProtocolError):
                decode_request(payload)

    def test_stop_request_requires_current_generation(self):
        good = b'{"protocol_version":1,"request_id":"s-1","event":"stop","runtime_generation":"g-1"}\n'
        self.assertEqual(decode_request(good, expected_generation="g-1")["event"], "stop")
        with self.assertRaises(ProtocolError):
            decode_request(good, expected_generation="g-2")

    def test_frame_allowlist_does_not_copy_untrusted_fields(self):
        frame = encode_frame("r1", "g1", "error", {"error_code": "unsupported_version"})
        self.assertEqual(json.loads(frame)["error_code"], "unsupported_version")
        with self.assertRaises(ProtocolError):
            encode_frame("r1", "g1", "error", {"error_code": "unsupported_version", "detail": "secret"})

    def test_runtime_hash_guard_rejects_changed_runner_and_shim(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            (root / "runtime-manifest.json").write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "engine": {"name": "headroom-ai", "version": "0.44.0"},
                        "python": {
                            "version": "3.13.13",
                            "sha256": hashlib.sha256(b"pinned-python").hexdigest(),
                        },
                        "runtime": {
                            "runner": "headroom_runtime.py",
                            "site_packages": "Lib/site-packages",
                        },
                    }
                ),
                encoding="utf-8",
            )
            (root / "headroom_runtime.py").write_bytes(b"pinned-runner")
            (root / "python.exe").write_bytes(b"pinned-python")
            (root / "prefix_tracker.py").write_bytes(b"pinned-prefix")
            (root / "cache_control_shim.py").write_bytes(b"pinned-shim")
            (root / "optimization_control.py").write_bytes(b"pinned-control")
            (root / "optimization-sources.json").write_bytes(b"pinned-sources")
            manifest = {
                "schema_version": 1,
                "protocol_version": 1,
                "engine": {
                    "name": "headroom-ai",
                    "version": "0.44.0",
                    "python_version": "3.13.13",
                    "payload_root": ".",
                    "runtime_manifest": "runtime-manifest.json",
                    "runtime_manifest_sha256": hashlib.sha256((root / "runtime-manifest.json").read_bytes()).hexdigest(),
                    "runner": "headroom_runtime.py",
                    "runner_sha256": hashlib.sha256(b"pinned-runner").hexdigest(),
                    "python": "python.exe",
                    "python_sha256": hashlib.sha256(b"pinned-python").hexdigest(),
                    "site_packages": "Lib/site-packages",
                    "prefix_tracker": "prefix_tracker.py",
                    "prefix_tracker_sha256": hashlib.sha256(b"pinned-prefix").hexdigest(),
                },
                "shim": {"file": "cache_control_shim.py", "sha256": hashlib.sha256(b"pinned-shim").hexdigest()},
                "optimization_control": {"file": "optimization_control.py", "sha256": hashlib.sha256(b"pinned-control").hexdigest(), "sources": "optimization-sources.json", "sources_sha256": hashlib.sha256(b"pinned-sources").hexdigest()},
                "profile": {
                    "mode": "cache",
                    "savings_profile": "coding",
                    "ccr_ttl_seconds": 7200,
                    "telemetry": "on",
                    "beacon": "off",
                    "output_shaper": "0",
                    "model_router_enabled": "0",
                    "memory": False,
                    "learn": False,
                },
            }
            pins = {
                "engine_name": manifest["engine"]["name"],
                "engine_version": manifest["engine"]["version"],
                "runtime_manifest_sha256": manifest["engine"]["runtime_manifest_sha256"],
                "runner_sha256": manifest["engine"]["runner_sha256"],
                "python_version": manifest["engine"]["python_version"],
                "python_sha256": manifest["engine"]["python_sha256"],
                "site_packages": manifest["engine"]["site_packages"],
                "prefix_tracker_sha256": manifest["engine"]["prefix_tracker_sha256"],
                "shim_sha256": manifest["shim"]["sha256"],
                "optimization_sha256": manifest["optimization_control"]["sha256"],
                "optimization_sources_sha256": manifest["optimization_control"]["sources_sha256"],
            }
            verified = verify_runtime_files(manifest, root, expected_pins=pins)
            self.assertEqual(verified["runner"], hashlib.sha256(b"pinned-runner").hexdigest())
            for name, original in (("optimization_control.py", b"pinned-control"), ("optimization-sources.json", b"pinned-sources")):
                (root / name).write_bytes(b"changed-control")
                with self.assertRaises(ProtocolError):
                    verify_runtime_files(manifest, root, expected_pins=pins)
                (root / name).write_bytes(original)
            (root / "headroom_runtime.py").write_bytes(b"changed-runner")
            with self.assertRaises(ProtocolError):
                verify_runtime_files(manifest, root, expected_pins=pins)
            (root / "headroom_runtime.py").write_bytes(b"pinned-runner")
            (root / "cache_control_shim.py").write_bytes(b"changed-shim")
            with self.assertRaises(ProtocolError):
                verify_runtime_files(manifest, root, expected_pins=pins)


def _read_protocol(stream, events: queue.Queue[bytes | None]) -> None:
    while True:
        line = stream.readline(8194)
        if not line:
            events.put(None)
            return
        events.put(line)


def _next_protocol(events: queue.Queue[bytes | None], timeout: float) -> bytes:
    try:
        line = events.get(timeout=timeout)
    except queue.Empty as exc:
        raise AssertionError("desktop controller protocol deadline expired") from exc
    if line is None:
        raise AssertionError("desktop controller exited before the expected event")
    if len(line) > 8192:
        raise AssertionError("desktop controller protocol frame exceeded its limit")
    return line


class DesktopRuntimeSmokeTests(unittest.TestCase):
    @unittest.skipUnless(os.name == "nt", "desktop runtime supervisor targets Windows")
    def test_consoleless_controller_starts_and_gracefully_stops_owned_engine(self):
        root = Path(__file__).resolve().parents[2]
        run_parent = root / "work" / "proofs" / "runtime" / "runs"
        run_parent.mkdir(parents=True, exist_ok=True)
        run_parent = run_parent.resolve(strict=True)
        run_root = Path(tempfile.mkdtemp(prefix="h014a desktop ", dir=run_parent)).resolve(strict=True)
        if run_parent not in run_root.parents:
            raise AssertionError("refusing to use a run directory outside the owned runtime runs root")
        manifest_path = root / "runtime/manifest.json"
        payload = (manifest_path.parent / json.loads(manifest_path.read_text(encoding="utf-8"))["engine"]["payload_root"]).resolve()
        controller_script = root / "runtime" / "desktop_controller.py"
        manifest = root / "runtime" / "manifest.json"
        state_root = run_root / "state with spaces"
        workspace = run_root / "workspace with spaces"
        config = run_root / "config with spaces"
        for path in (state_root, workspace, config):
            path.mkdir(parents=True, exist_ok=True)
        windows_root = os.environ.get("SystemRoot", r"C:\Windows")
        system_drive = os.environ.get("SystemDrive", "C:")
        home = state_root / "controller-home"
        temp = state_root / "controller-temp"
        env = {
            "SystemRoot": windows_root,
            "SystemDrive": system_drive,
            "WINDIR": windows_root,
            "PATH": str(Path(windows_root) / "System32"),
            "TEMP": str(temp),
            "TMP": str(temp),
            "USERPROFILE": str(home),
            "HOMEDRIVE": home.drive or system_drive,
            "HOMEPATH": str(home)[len(home.drive) :],
            "HOME": str(home),
            "APPDATA": str(home / "AppData" / "Roaming"),
            "LOCALAPPDATA": str(home / "AppData" / "Local"),
        }
        command = [
            str(payload / "python.exe"),
            "-I",
            "-B",
            "-u",
            str(controller_script),
            "--manifest",
            str(manifest),
            "--state-root",
            str(state_root),
            "--workspace",
            str(workspace),
            "--config",
            str(config),
        ]
        process = subprocess.Popen(
            command,
            cwd=payload,
            env=env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            creationflags=subprocess.CREATE_NO_WINDOW,
            close_fds=True,
        )
        events: queue.Queue[bytes | None] = queue.Queue()
        self.assertIsNotNone(process.stdin)
        self.assertIsNotNone(process.stdout)
        reader = threading.Thread(target=_read_protocol, args=(process.stdout, events), daemon=True)
        reader.start()
        generation: str | None = None
        listener_port: int | None = None
        try:
            process.stdin.write(b'{"protocol_version":1,"request_id":"h014a-start-1","event":"start"}\n')
            process.stdin.flush()
            ready = json.loads(_next_protocol(events, 150))
            self.assertEqual(ready.get("event"), "ready", ready)
            self.assertEqual(ready.get("request_id"), "h014a-start-1")
            self.assertEqual(ready.get("protocol_version"), 1)
            generation = ready.get("runtime_generation")
            self.assertIsInstance(generation, str)
            self.assertEqual(ready.get("engine_version"), "0.44.0")
            self.assertTrue(ready.get("controller_consoleless"))
            self.assertTrue(ready.get("manager_hidden_console"))
            self.assertTrue(ready.get("engine_console_allocated"))
            self.assertGreater(ready.get("helper_pid", 0), 0)
            self.assertGreater(ready.get("engine_pid", 0), 0)
            self.assertNotEqual(ready.get("helper_pid"), ready.get("engine_pid"))
            self.assertEqual(ready.get("listener_pid"), ready.get("engine_pid"))
            listener_port = ready.get("listener_port")
            self.assertGreater(listener_port or 0, 0)
            self.assertEqual(
                ready.get("config"),
                {
                    "mode": "cache",
                    "savings_profile": "coding",
                    "ttl_seconds": 7200,
                    "telemetry_enabled": True,
                    "beacon_enabled": False,
                    "memory_enabled": False,
                    "learning_enabled": False,
                    "output_shaper_enabled": False,
                    "model_router_enabled": False,
                    "protected_tools": [],
                },
            )
            shim = ready.get("shim")
            self.assertEqual(shim.get("active"), True)
            self.assertEqual(shim.get("version"), "0.44.0")
            self.assertEqual(
                shim.get("prefix_tracker_sha256"),
                "f1af4ee279f879efc48ddfebbff125a088893bb64ee960ca2dbd4ad1cdb628ee",
            )
            self.assertEqual(
                shim.get("shim_sha256"),
                hashlib.sha256((root / "runtime" / "cache_control_shim.py").read_bytes()).hexdigest(),
            )

            stop = {
                "protocol_version": 1,
                "request_id": "h014a-stop-1",
                "runtime_generation": generation,
                "event": "stop",
            }
            process.stdin.write((json.dumps(stop, separators=(",", ":")) + "\n").encode("ascii"))
            process.stdin.flush()
            stopped = json.loads(_next_protocol(events, 45))
            self.assertEqual(stopped.get("event"), "stopped")
            self.assertEqual(stopped.get("request_id"), "h014a-stop-1")
            self.assertEqual(stopped.get("runtime_generation"), generation)
            self.assertEqual(stopped.get("shutdown"), "graceful")
            self.assertEqual(stopped.get("engine_exit_code"), 0)
            self.assertTrue(stopped.get("owned_port_closed"))
            self.assertTrue(stopped.get("hidden_console_verified"))
            self.assertTrue(stopped.get("lifespan_complete"))
            self.assertTrue(stopped.get("proxy_summary"))
            self.assertFalse(stopped.get("force_fallback"))
            self.assertEqual(process.wait(timeout=35), 0)
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
                sock.settimeout(1)
                self.assertNotEqual(sock.connect_ex(("127.0.0.1", listener_port)), 0)
        finally:
            if process.poll() is None and process.stdin is not None and not process.stdin.closed:
                process.stdin.close()
                try:
                    process.wait(timeout=40)
                except subprocess.TimeoutExpired:
                    process.terminate()
                    process.wait(timeout=5)
            if process.stdin is not None and not process.stdin.closed:
                process.stdin.close()
            reader.join(timeout=2)
            if process.stdout is not None and not process.stdout.closed:
                process.stdout.close()
            cleanup_target = run_root.resolve(strict=True)
            if run_parent not in cleanup_target.parents:
                raise AssertionError("refusing to remove a run directory outside the owned runtime runs root")
            if cleanup_target.exists():
                shutil.rmtree(cleanup_target)


if __name__ == "__main__":
    unittest.main()
