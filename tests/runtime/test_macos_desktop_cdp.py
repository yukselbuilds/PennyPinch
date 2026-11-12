"""Bound the disposable Desktop debug target without opening a browser."""

import importlib.util
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("desktop_cdp", ROOT / "scripts/macos-desktop-cdp.py")
CDP = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CDP)


class DesktopCdpTests(unittest.TestCase):
    def test_port_record_is_bounded_and_browser_path_is_not_a_network_url(self):
        self.assertEqual(CDP.debug_port("41731\n/devtools/browser/test-id\n"), 41731)
        for value in ["80\n/devtools/browser/test", "41731\nhttps://other.invalid/x", "70000\n/devtools/browser/x", "41731\n/devtools/browser/x\nextra"]:
            with self.subTest(value=len(value)):
                with self.assertRaises(CDP.CdpError):
                    CDP.debug_port(value)

    def test_only_the_owned_loopback_app_page_can_be_selected(self):
        page = {"type": "page", "url": "app://-/index.html", "webSocketDebuggerUrl": "ws://127.0.0.1:41731/devtools/page/one"}
        self.assertEqual(CDP.page_target([page], 41731), page["webSocketDebuggerUrl"])
        for change in [{"url": "https://other.invalid"}, {"webSocketDebuggerUrl": "ws://other.invalid:41731/devtools/page/one"},
                       {"webSocketDebuggerUrl": "ws://127.0.0.1:41732/devtools/page/one"},
                       {"webSocketDebuggerUrl": "ws://user:secret@127.0.0.1:41731/devtools/page/one"}]:
            with self.subTest(fields=list(change)):
                with self.assertRaises(CDP.CdpError):
                    CDP.page_target([{**page, **change}], 41731)
        with self.assertRaises(CDP.CdpError):
            CDP.page_target([page, page], 41731)


if __name__ == "__main__":
    unittest.main()
