from hashlib import sha256
import json
from pathlib import Path
import re


keys = {
    "specialize": "specialize",
    "unattend": "unattend",
    "fully handle the first boot": "first_boot",
    "computer restarted unexpectedly": "unexpected_restart",
    "windeploy": "windeploy",
    "childcompletion": "child_completion",
    "runsynchronous": "run_synchronous",
    "microsoft-windows-deployment": "deployment_component",
    "microsoft-windows-shell-setup": "shell_setup_component",
    "oobe": "oobe",
    "bitlocker": "bitlocker",
    "setupcl": "setup_cl",
    "cbs": "component_servicing",
    "driver": "driver",
    "device": "device",
}
summary = {}
for file in Path("setup-diagnostics").glob("*.log"):
    raw = file.read_bytes()
    encoding = "utf-16" if raw.startswith((b"\xff\xfe", b"\xfe\xff")) else "utf-8-sig"
    lines = raw.decode(encoding, errors="replace").splitlines()
    tail = []
    for line in lines:
        line = line.casefold()
        if not re.search(r"\b(error|fail(?:ed|ure)?|abort(?:ed)?)\b", line):
            continue
        tags = sorted({tag for pattern, tag in keys.items() if pattern in line})
        codes = sorted(set(re.findall(r"\b0x[0-9a-f]{8}\b", line)))
        symbols = re.findall(r"(?:\[|\b)([a-z][a-z0-9_]{1,50}::[a-z][a-z0-9_]{1,80})(?:\]|\b)", line)
        symbols = sorted({symbol for symbol in symbols if symbol.startswith(("windeploy::", "csetup", "unattend"))})
        if tags or codes or symbols:
            tail.append({"tags": tags, "codes": codes[:5], "setup_symbols": symbols[:3]})
    summary[file.name] = {"sha256": sha256(raw).hexdigest(), "lines": len(lines), "last_errors": tail[-14:]}
print("::notice title=Windows 11 Panther diagnosis::" + json.dumps(summary, separators=(",", ":")), flush=True)
