from collections import Counter
import json
import re
import subprocess


MAX_OUTPUT = 512 * 1024
KNOWN_EVENTS = {
    "thread.started", "turn.started", "turn.completed", "turn.failed",
    "item.started", "item.updated", "item.completed", "error",
}
REASONS = {
    "sandbox": r"sandbox|access is denied|privilege",
    "usage_limit": r"usage.limit|insufficient.quota|quota exceeded|rate.limit",
    "model": r"model.{0,100}(not available|not supported|not found)",
    "authentication": r"unauthori[sz]ed|refresh token|invalid.grant|not logged in",
    "transport": r"connection|timed out|dns|websocket|stream disconnected|tls|certificate",
    "route": r"unexpected status|incorrect api|error sending request",
}


def run_turn(cli, workspace, marker, env=None):
    command = [str(cli), "--no-daemon", "-a", "never", "exec", "--ephemeral", "--json", "--color", "never",
               "--sandbox", "read-only", "--skip-git-repo-check", "--ignore-rules", "-C", str(workspace),
               "-c", 'windows.sandbox="unelevated"',
               f"Reply with exactly {marker} and no other text. Use no tools or subagents."]
    try:
        proc = subprocess.run(command, cwd=workspace, env=env, timeout=110, stdin=subprocess.DEVNULL,
                              capture_output=True, creationflags=subprocess.CREATE_NO_WINDOW)
    except subprocess.TimeoutExpired:
        return {"outcome": "timeout"}, None
    evidence = {"exit": proc.returncode, "stdout_bytes": len(proc.stdout), "stderr_bytes": len(proc.stderr)}
    if len(proc.stdout) > MAX_OUTPUT or len(proc.stderr) > MAX_OUTPUT:
        return {**evidence, "outcome": "output_limit"}, None
    records = []
    invalid = 0
    for line in proc.stdout.splitlines():
        if not line.strip():
            continue
        try:
            row = json.loads(line)
            if isinstance(row, dict):
                records.append(row)
            else:
                invalid += 1
        except (ValueError, UnicodeError):
            invalid += 1
    evidence["invalid_lines"] = invalid
    counts = Counter(row.get("type") if isinstance(row.get("type"), str) and row["type"] in KNOWN_EVENTS else "other"
                     for row in records)
    evidence["events"] = dict(counts)
    if counts["error"] or counts["turn.failed"] or proc.returncode:
        safe_text = " ".join([proc.stderr.decode("utf-8", "replace"), *(
            json.dumps(row, ensure_ascii=False) for row in records if row.get("type") in {"error", "turn.failed"})])
        evidence["reason_classes"] = [kind for kind, pattern in REASONS.items() if re.search(pattern, safe_text, re.I)]
        evidence["http_statuses"] = sorted({int(code) for code in re.findall(
            r"(?i)(?:HTTP\s+|status(?:\s+code)?\s*[:=]?\s*)(401|403|404|409|429|500|502|503|504)\b", safe_text)})
    replies = [item.get("text", "").strip() for row in records
               if row.get("type") == "item.completed" and isinstance(item := row.get("item"), dict)
               and item.get("type") == "agent_message" and isinstance(item.get("text"), str)]
    evidence["message_count"] = len(replies)
    # Markdown delimiters do not change the unique answer token.
    evidence["marker"] = len(replies) == 1 and replies[0].strip("`'\". \r\n") == marker
    completed = [row["item"] for row in records if row.get("type") == "item.completed" and isinstance(row.get("item"), dict)]
    evidence["advisory_items"] = sum(item.get("type") == "error" for item in completed)
    evidence["tool_item_count"] = sum(item.get("type") not in {"agent_message", "reasoning", "error"} for item in completed)
    if proc.returncode or invalid or counts["turn.completed"] != 1 or counts["turn.failed"] or counts["error"]:
        return {**evidence, "outcome": "turn_incomplete"}, None
    if not evidence["marker"] or evidence["tool_item_count"]:
        return {**evidence, "outcome": "unexpected_answer"}, None
    usage = next(row.get("usage") for row in records if row.get("type") == "turn.completed")
    if not isinstance(usage, dict) or any(type(usage.get(key)) is not int or usage[key] <= 0
                                          for key in ("input_tokens", "output_tokens")):
        return {**evidence, "outcome": "usage_missing"}, None
    return {**evidence, "outcome": "pass"}, usage
