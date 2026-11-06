"""A clean process exit must not hide uvicorn cancelling an active response."""
import io
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest import mock
from runtime.desktop_helper import BoundedEngineOutput, _shutdown_kind
from runtime import desktop_controller


class ShutdownEvidenceTests(unittest.TestCase):
    def output(self, value):
        output = BoundedEngineOutput(io.BytesIO(value))
        self.assertTrue(output.finish())
        return output

    def test_clean_exit_without_cancelled_tasks_keeps_graceful_evidence(self):
        output = self.output(b"Application shutdown complete\nHeadroom Proxy Session Summary\n")
        self.assertEqual(_shutdown_kind({"shutdown": "graceful"}, output), "graceful")
        self.assertEqual(_shutdown_kind({"shutdown": "incomplete"}, output), "incomplete")
        self.assertEqual(_shutdown_kind({"shutdown": "graceful"}, None), "incomplete")

    def test_drain_timeout_over_chunk_boundary_overrides_zero_exit(self):
        prefix = b"x" * (4096 - len(b"timeout graceful"))
        output = self.output(prefix + b"timeout graceful shutdown exceeded\nApplication shutdown complete\n")
        self.assertTrue(output.has("request_drain_timeout"))
        self.assertEqual(_shutdown_kind({"shutdown": "graceful", "engine_exit_code": 0}, output), "incomplete")

    def test_tokenizer_cache_missing_or_changed_data_is_not_ready(self):
        root = Path(__file__).resolve().parents[2]
        parent = root / "work/proofs/disconnect/tokenizer-unit"
        parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=parent) as directory:
            payload = Path(directory).resolve(strict=True)
            self.assertTrue(payload.is_relative_to(root))
            cache = payload / "tiktoken-cache"
            cache.mkdir()
            url = "https://tokenizer.invalid/fixture"
            target = cache / hashlib.sha1(url.encode()).hexdigest()
            sources = ((url, hashlib.sha256(b"verified fixture").hexdigest()),)
            with mock.patch.object(desktop_controller, "TOKENIZER_SOURCES", sources):
                with self.assertRaises(OSError):
                    desktop_controller.verify_tokenizer_cache(payload)
                target.write_bytes(b"verified fixture")
                self.assertEqual(desktop_controller.verify_tokenizer_cache(payload), cache)
                target.write_bytes(b"changed fixture")
                with self.assertRaises(desktop_controller.ProtocolError) as raised:
                    desktop_controller.verify_tokenizer_cache(payload)
                self.assertEqual(raised.exception.code, "runtime_pin_mismatch")


if __name__ == "__main__":
    unittest.main()
