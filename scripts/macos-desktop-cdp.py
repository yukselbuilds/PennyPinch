"""Small, loopback-only observer for one owned disposable Electron profile."""

import json
import os
import re
import stat
import subprocess
import time
from urllib.parse import urlsplit
from urllib.request import build_opener, ProxyHandler


class CdpError(RuntimeError):
    pass


def debug_port(value):
    lines = value.splitlines()
    if len(value) > 256 or len(lines) != 2 or not lines[0].isascii() or not lines[0].isdigit():
        raise CdpError("invalid_debug_port_record")
    port = int(lines[0])
    if not 1024 <= port <= 65535 or not re.fullmatch(r"/devtools/browser/[A-Za-z0-9_-]+", lines[1]):
        raise CdpError("invalid_debug_port_record")
    return port


def page_target(pages, port):
    if not isinstance(pages, list) or len(pages) > 16:
        raise CdpError("invalid_debug_pages")
    candidates = []
    for page in pages:
        if not isinstance(page, dict) or page.get("type") != "page":
            continue
        value = page.get("url")
        if not isinstance(value, str) or len(value) > 4096:
            continue
        target = urlsplit(value)
        if target.scheme == "app" and target.netloc == "-" and target.path == "/index.html":
            candidates.append(page)
    if len(candidates) != 1:
        raise CdpError("owned_app_page_missing")
    value = candidates[0].get("webSocketDebuggerUrl")
    if not isinstance(value, str) or len(value) > 512:
        raise CdpError("invalid_page_websocket")
    target = urlsplit(value)
    if (target.scheme != "ws" or target.hostname != "127.0.0.1" or target.port != port
            or target.username is not None or target.password is not None or target.query or target.fragment
            or not re.fullmatch(r"/devtools/page/[A-Za-z0-9_-]+", target.path)):
        raise CdpError("invalid_page_websocket")
    return value


class Page:
    def __init__(self, profile, app_pid):
        from websockets.sync.client import connect

        record = profile / "DevToolsActivePort"
        deadline = time.monotonic() + 20
        while not record.exists() and time.monotonic() < deadline:
            time.sleep(0.1)
        info = record.lstat()
        if not stat.S_ISREG(info.st_mode) or info.st_uid != os.geteuid() or info.st_nlink != 1 or info.st_size > 256:
            raise CdpError("unsafe_debug_port_record")
        descriptor = os.open(record, os.O_RDONLY | os.O_NOFOLLOW)
        try:
            opened = os.fstat(descriptor)
            if (opened.st_dev, opened.st_ino) != (info.st_dev, info.st_ino):
                raise CdpError("debug_port_record_changed")
            self.port = debug_port(os.read(descriptor, 257).decode("ascii"))
        finally:
            os.close(descriptor)
        listener = subprocess.run(["/usr/sbin/lsof", "-nP", "-a", "-p", str(app_pid),
            f"-iTCP:{self.port}", "-sTCP:LISTEN", "-Fn"], capture_output=True, text=True, timeout=5)
        addresses = {line[1:] for line in listener.stdout.splitlines() if line.startswith("n")}
        if listener.returncode != 0 or addresses != {f"127.0.0.1:{self.port}"}:
            raise CdpError("debug_listener_not_owned_loopback")
        opener = build_opener(ProxyHandler({}))
        with opener.open(f"http://127.0.0.1:{self.port}/json/list", timeout=5) as response:
            data = response.read(1024 * 1024 + 1)
        if len(data) > 1024 * 1024:
            raise CdpError("debug_page_list_limit")
        target = page_target(json.loads(data), self.port)
        self.socket = connect(target, proxy=None, open_timeout=10, max_size=1024 * 1024)
        self.next_id = 0

    def call(self, method, params):
        self.next_id += 1
        self.socket.send(json.dumps({"id": self.next_id, "method": method, "params": params}))
        for _ in range(128):
            message = json.loads(self.socket.recv(timeout=10))
            if message.get("id") == self.next_id:
                if "error" in message:
                    raise CdpError("debug_command_failed")
                return message["result"]
        raise CdpError("debug_message_limit")

    def evaluate(self, expression):
        result = self.call("Runtime.evaluate", {"expression": expression, "returnByValue": True, "awaitPromise": True})
        if "exceptionDetails" in result or "value" not in result.get("result", {}):
            raise CdpError("debug_evaluation_failed")
        return result["result"]["value"]

    def observe(self):
        return self.evaluate("""(() => {
          const visible = e => !!(e.getBoundingClientRect().width && e.getBoundingClientRect().height);
          const labels = ['Continue','Get started','Skip','Skip for now','Sign in','Log in',
            'Sign in with ChatGPT','Continue with ChatGPT','Sign in another way','Use an API key',
            'New chat','New thread','Open folder','Open project','Add project','Send','Send message','Submit'];
          const buttons = [...document.querySelectorAll('button,a,[role=button]')].filter(visible);
          const known = new Set(buttons.map(e => (e.innerText || e.getAttribute('aria-label') || e.title || '').trim()).filter(x => labels.includes(x)));
          return {ready:document.readyState, root_children:document.getElementById('root')?.childElementCount ?? 0,
            body_characters:document.body?.innerText.length ?? 0, visible_buttons:buttons.length,
            known_buttons:[...known].sort(), unknown_buttons:buttons.filter(e => !labels.includes((e.innerText || e.getAttribute('aria-label') || e.title || '').trim())).length,
            editors:[...document.querySelectorAll('textarea,[contenteditable=true]')].filter(visible).length,
            password_inputs:[...document.querySelectorAll('input[type=password]')].filter(visible).length};
        })()""")

    def close(self):
        self.socket.close()
