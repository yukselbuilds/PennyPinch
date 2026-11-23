"""The private startup frame and UDS-only dashboard capability boundaries."""

import importlib.util
import io
import json
from pathlib import Path
import sys
import types
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("macos_engine", ROOT / "runtime/macos_engine.py")
engine = importlib.util.module_from_spec(spec)
spec.loader.exec_module(engine)
SECRET = "123456789abcdef0123456789abcdef0"


class MacOSBootstrapTests(unittest.TestCase):
    def frame(self, **changes):
        value = dict(schema_version=1, generation=SECRET, secret=SECRET,
                     protected_tools=["exec_command", "headroom_retrieve"])
        value.update(changes)
        return json.dumps(value).encode() + b"\n"

    def test_valid_frame_preserves_identity_and_protected_tools(self):
        value = engine.start_request(io.BytesIO(self.frame()))
        self.assertEqual(value["secret"], SECRET)
        self.assertEqual(value["protected_tools"], ["exec_command", "headroom_retrieve"])

    def test_rejects_ambiguous_identity_shape_and_unbounded_frames(self):
        frames = [
            self.frame(schema_version=True), self.frame(secret="0" * 32),
            self.frame(secret="bad\r\nheader"), self.frame(extra=True),
            self.frame(protected_tools=["z", "a"]), self.frame(protected_tools=["a", "a"]),
            self.frame(protected_tools=[{}]), self.frame(protected_tools=["bad,name"]),
            self.frame()[:-1], b"x" * 4097 + b"\n", b"{}\n",
            self.frame().replace(b'"schema_version": 1', b'"schema_version": 1,"schema_version": 1'),
        ]
        for frame in frames:
            with self.subTest(length=len(frame)):
                with self.assertRaises(engine.StartupError):
                    engine.start_request(io.BytesIO(frame))

    def test_linux_x64_checks_the_private_state_directory(self):
        args = types.SimpleNamespace(state=ROOT)
        with (
            patch.object(engine.sys, "platform", "linux"),
            patch.object(engine.sys, "version_info", (3, 13, 13)),
            patch.object(engine.platform, "machine", return_value="x86_64"),
            patch.object(engine, "private_directory", side_effect=engine.StartupError("invalid_private_state")),
        ):
            with self.assertRaisesRegex(engine.StartupError, "^invalid_private_state$"):
                engine.run(args)

    def test_dashboard_capability_is_only_for_two_private_uds_reads(self):
        package = types.ModuleType("headroom")
        proxy = types.ModuleType("headroom.proxy")
        server = types.ModuleType("headroom.proxy.server")
        guard = types.ModuleType("headroom.proxy.loopback_guard")
        package.proxy = proxy
        proxy.server = server
        server._request_is_loopback = lambda request: request.client == "original-loopback"
        guard.is_loopback_host_header = lambda host: host == "127.0.0.1"
        modules = {module.__name__: module for module in (package, proxy, server, guard)}
        with patch.dict(sys.modules, modules):
            engine.install_private_dashboard(SECRET)
            for path in ("/health", "/stats"):
                request = types.SimpleNamespace(client=None, scope={"path": path},
                    headers={"host": "127.0.0.1", "x-headroom-desktop-control": SECRET})
                self.assertTrue(server._request_is_loopback(request))
                for header, value in (("host", "outside.example"),
                                      ("x-headroom-desktop-control", ""),
                                      ("x-headroom-desktop-control", "wrong"),
                                      ("x-headroom-desktop-control", "\u2603")):
                    bad = types.SimpleNamespace(**vars(request))
                    bad.headers = {**request.headers, header: value}
                    self.assertFalse(server._request_is_loopback(bad))
                request.client = "external-client"
                self.assertFalse(server._request_is_loopback(request))
                request.client = "original-loopback"
                self.assertTrue(server._request_is_loopback(request))
            for path in ("/stats/clear", "/config", "/v1/chat/completions"):
                request = types.SimpleNamespace(client=None, scope={"path": path},
                    headers={"host": "127.0.0.1", "x-headroom-desktop-control": SECRET})
                self.assertFalse(server._request_is_loopback(request))

    def test_embedded_cli_keeps_bootstrap_entrypoint_and_owned_socket(self):
        package = types.ModuleType("headroom")
        cli = types.ModuleType("headroom.cli")
        observed = []
        cli.main = lambda **kwargs: observed.append((list(sys.argv), kwargs))
        package.cli = cli
        endpoint = ROOT / "work/proofs/macos/bootstrap-fixture.sock"
        with patch.dict(sys.modules, {"headroom": package, "headroom.cli": cli}), patch.object(sys, "argv", []):
            engine.serve(endpoint)
        argv, options = observed[0]
        self.assertEqual(Path(argv[0]), Path(engine.__file__))
        self.assertEqual(argv[1:], ["proxy", "--uds", str(endpoint), "--workers", "1",
                                    "--no-learn", "--no-memory-tools", "--no-memory-context"])
        self.assertEqual(options, {"prog_name": "headroom"})


if __name__ == "__main__":
    unittest.main()
