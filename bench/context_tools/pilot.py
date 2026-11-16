"""Model-free paired candidate replay. Synthetic stdout/stderr only; no client setup."""
import argparse
from collections import Counter
import hashlib
from http.server import ThreadingHTTPServer
import importlib.util
import json
import os
from pathlib import Path
import platform
import queue
import re
import shutil
import site
import sqlite3
import subprocess
import sys
import threading
import time
import traceback

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


rtk = load("context_pilot_rtk", ROOT / "bench/rtk/replay.py")
base, fixtures = rtk.base, rtk.fixtures
LOSSLESS = None


def require(value, code):
    base.require(value, code)


def write(path, data):
    path.write_text(json.dumps(data, ensure_ascii=True, indent=2) + "\n", encoding="utf-8")


def fingerprints(paths):
    return {(path.relative_to(ROOT).as_posix() if path.is_relative_to(ROOT) else str(path)): base.sha(path)
            for path in sorted(set(paths))}


def text(reply):
    return "\n".join(item["text"] for item in reply.get("content", []) if item.get("type") == "text")


class MCP:
    def __init__(self, node, entry, workspace, env, supervisor):
        self.job = supervisor.KillOnCloseJob()
        self.process = subprocess.Popen([str(node), str(entry), "--agent", "codex"], cwd=workspace, env=env,
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                        creationflags=subprocess.CREATE_NO_WINDOW)
        self.job.assign(self.process.pid)
        self.messages, self.sequence = queue.Queue(), 0
        self.stderr_tail = bytearray()
        threading.Thread(target=self.read, daemon=True).start()
        threading.Thread(target=self.read_errors, daemon=True).start()
        self.stopped = False

    def read_errors(self):
        while data := self.process.stderr.read(1024):
            self.stderr_tail.extend(data)
            if len(self.stderr_tail) > 8192:
                del self.stderr_tail[:-8192]

    def read(self):
        try:
            for line in iter(lambda: self.process.stdout.readline(2 * 1024 * 1024), b""):
                self.messages.put(json.loads(line))
        except (ValueError, OSError):
            self.messages.put({"reader_error": True})

    def send(self, message):
        self.process.stdin.write(base.encoded(message) + b"\n")
        self.process.stdin.flush()

    def request(self, method, params):
        self.sequence += 1
        identity = self.sequence
        self.send({"jsonrpc": "2.0", "id": identity, "method": method, "params": params})
        end = time.monotonic() + 30
        while time.monotonic() < end:
            message = self.messages.get(timeout=max(0.01, end - time.monotonic()))
            require(not message.get("reader_error"), "mcp_reader_error")
            if "method" in message and "id" in message:
                result = {"roots": []} if message["method"] == "roots/list" else {}
                self.send({"jsonrpc": "2.0", "id": message["id"], "result": result})
            elif message.get("id") == identity:
                require("error" not in message, "mcp_protocol_error")
                return message["result"]
        raise AssertionError("mcp_timeout")

    def call(self, name, arguments):
        start = time.perf_counter()
        reply = self.request("tools/call", {"name": name, "arguments": arguments})
        return reply, (time.perf_counter() - start) * 1000

    def close(self):
        try:
            was_alive = self.process.poll() is None
            self.process.stdin.close()
            self.process.wait(timeout=10)
            # Pinned 2.2.0 installExitGuard sends SIGTERM to itself on stdin EOF.
            # Windows reports that intentional path as exit 1, not POSIX -SIGTERM.
            self.stopped = was_alive and self.process.returncode in (0, 1)
        finally:
            self.job.close()
            if self.process.poll() is None:
                self.process.wait(timeout=10)


def create_fixtures(workspace, env, git):
    fixtures.create(workspace, env, git)
    result = []
    for name in fixtures.EXPECTED_EXIT:
        command, _ = fixtures.commands(name, workspace, git, Path("unused-rtk"))
        raw = rtk.command(command, workspace, env, 30)
        require(raw["exit_code"] == fixtures.EXPECTED_EXIT[name], "raw_fixture_exit")
        result.append({"case": name, "exit_code": raw["exit_code"],
                       "stdout": raw["stdout"].decode("utf-8"), "stderr": raw["stderr"].decode("utf-8")})
    result.extend([
        {"case": "repeated_log", "stdout": "INFO worker ready for next item\n" * 2000 + "SUMMARY all 2000 items completed\n", "stderr": "", "exit_code": 0},
        {"case": "stdout_fail", "stdout": "test fixture ... ok\n" * 600 + "FAIL: STDOUT_FAILURE_9007199254740993 expected=-17.2500 actual=0.000000000000001\n", "stderr": "", "exit_code": 1},
        {"case": "stderr_success", "stdout": "SUCCESS fixture complete\n", "stderr": "WARNING: SUCCESS_STDERR_9007199254740993 value=-17.2500\n", "exit_code": 0},
        {"case": "short_unicode", "stdout": "Tamam — Türkçe λ, sayı=9007199254740993, oran=-17.2500\n", "stderr": "", "exit_code": 0},
        {"case": "long_source", "stdout": (workspace / "source.py").read_bytes().decode("utf-8") * 5, "stderr": "", "exit_code": 0},
        {"case": "event_count", "stdout": "EVENT identical successful event\n" * 200, "stderr": "", "exit_code": 0},
    ])
    for item in result:
        lines = (item["stdout"] + item["stderr"]).splitlines()
        item["protect"] = [line for line in lines if any(word in line for word in ("FAIL:", "error:", "WARNING:", "SUMMARY", "test result:"))]
        item["full_read"] = item["case"] in ("source_read", "long_source", "json_exact", "git_diff", "short_unicode")
        # A query may be echoed by the tool. Never send the full expected evidence as the query.
        item["query"] = item["protect"][0].split()[0].rstrip(":") if item["protect"] else ""
        require(all(line != item["query"] for line in item["protect"]), "query_contains_complete_expected_line")
        require(len((item["stdout"] + item["stderr"]).encode("utf-8")) <= 1024 * 1024, "fixture_size")
        for channel in ("stdout", "stderr"):
            (workspace / (item["case"] + "." + channel)).write_bytes(item[channel].encode("utf-8"))
    # JSON lexemes intentionally exceed IEEE-754 precision. Query code must keep the row text.
    collection = "\n".join('{"id":"row_%04d","integer":18446744073709551617,"decimal":0.1234567890123456789012345,"state":"ready"}' % n for n in range(1000)) + "\n"
    (workspace / "structured.jsonl").write_bytes(collection.encode("utf-8"))
    return result


def envelope(item):
    return base.encoded({key: item[key] for key in ("exit_code", "stdout", "stderr")}).decode("utf-8")


def predicates(item, visible):
    return {"required_lines_visible": all(line in visible for line in item["protect"]),
            "full_read_exact": (item["stdout"] in visible or json.dumps(item["stdout"], ensure_ascii=False)[1:-1] in visible) if item["full_read"] else None,
            "event_occurrences": visible.count("EVENT identical successful event") if item["case"] == "event_count" else None}


def mode_storage(data_root, label, expected):
    databases = list(data_root.glob("projects/*/context.db"))
    if not databases:
        return {"indexed": False}, None
    require(len(databases) == 1, "single_isolated_mode_database")
    with sqlite3.connect(databases[0].as_uri() + "?mode=ro", uri=True) as db:
        rows = db.execute("SELECT id FROM sources WHERE label=? ORDER BY id DESC", (label,)).fetchall()
        if not rows:
            return {"indexed": False}, None
        identity = rows[0][0]
        chunks = [row[0] for row in db.execute("SELECT body FROM chunks WHERE source_id=? ORDER BY CAST(ord AS INTEGER)", (identity,))]
    variants = ("".join(chunks), "\n".join(chunks))
    normalized = expected.replace("\r\n", "\n").rstrip("\n")
    count = identity
    handle = ""
    while count:
        count, remainder = divmod(count, 36)
        handle = "0123456789abcdefghijklmnopqrstuvwxyz"[remainder] + handle
    return {"indexed": True, "chunks": len(chunks), "raw_bytes_reconstructed": expected in variants,
            "newline_normalized_reconstructed": any(part.replace("\r\n", "\n").rstrip("\n") == normalized for part in variants),
            "all_line_occurrences_stored": Counter(expected.splitlines()) == Counter("\n".join(chunks).splitlines())}, "s" + handle


def headroom(engine, server, value, case, key, tokenizer, protected, has_envelope=False):
    body = fixtures.body(value, "source_read" if protected else "tool_result", 0, 0)
    outcome = base.request(engine.port, body, key, 30)
    with server.capture_lock:
        captured = server.captured.pop(key, [])
    require(outcome["status"] == 200 and len(captured) == 1 and captured[0]["auth_preserved"], "owned_upstream_required")
    forwarded = captured[0]["body"]
    base.validate_body(body, forwarded, None, protected)
    output = list(base.fixtures.outputs(forwarded).values())[0]
    if has_envelope:
        recovery = rtk.verify_recovery(engine.port, value, output, 30)
    else:
        recovered = value == output
        hashes = set(re.findall(r"(?:Retrieve (?:more|original): hash=|<<ccr:)([a-fA-F0-9]{12,24})(?=[^a-fA-F0-9]|$)", output))
        for content_hash in hashes:
            base.retrieval(engine.port, content_hash, value, 30)
            recovered = True
        recovery = {"verified": recovered, "retrievals": len(hashes)}
    if not recovery["verified"]:
        # The 0.44.0 log format is reversible RLE, not a CCR marker. Use its pinned inverse.
        expanded = LOSSLESS.unfold_repeated_blocks(LOSSLESS.expand_runs(output))
        if expanded == value:
            recovery.update(verified=True, scope="pinned_lossless_run_and_block_inverse")
    if not recovery["verified"]:
        # A failed candidate contract is a measured rejection, not a reason to skip later cases.
        # Only these explicitly synthetic outputs may be retained for format inspection.
        diagnostics = engine.directory.parent / "diagnostics"
        diagnostics.mkdir(exist_ok=True)
        (diagnostics / (key + ".txt")).write_text(output, encoding="utf-8", newline="")
        recovery["eligibility"] = "reject_until_exact_reconstruction_is_proven"
    return {"input_request_tokens": len(tokenizer.encode(base.encoded(body).decode(), disallowed_special=())),
            "forwarded_request_tokens": len(tokenizer.encode(base.encoded(forwarded).decode(), disallowed_special=())),
            "forwarded_text_tokens": len(tokenizer.encode(output, disallowed_special=())),
            "elapsed_ms": outcome["total_ms"], "recovery": recovery}, output


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--reuse-diet-proof", type=Path)
    args = parser.parse_args()
    run = args.directory.resolve()
    run.relative_to(ROOT / "work/proofs/context-tools")
    require(Path.cwd().resolve() == ROOT and os.name == "nt", "canonical_windows_workspace")
    require(not (run / "result.json").exists(), "new_result_required")
    controller = base.load_module("context_controller", ROOT / "runtime/desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(ROOT / "runtime/manifest.json")
    require(base.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "pinned_python_required")
    supervisor = base.load_module("context_supervisor", payload / manifest["engine"]["runner"])
    os.environ["TIKTOKEN_CACHE_DIR"] = str(controller.verify_tokenizer_cache(payload))
    os.environ.update(HF_HUB_OFFLINE="1", TRANSFORMERS_OFFLINE="1", LITELLM_LOCAL_MODEL_COST_MAP="True")
    site.addsitedir(str(payload / "Lib/site-packages"))
    import tiktoken
    global LOSSLESS
    from headroom.transforms import lossless_compaction as LOSSLESS
    tokenizer = tiktoken.get_encoding("o200k_base")
    count = lambda value: len(tokenizer.encode(value, disallowed_special=()))
    node, git = Path(shutil.which("node")), Path(shutil.which("git"))
    workspace = run / "workspace"
    env = supervisor.safe_environment(run / "tool-state", workspace, run / "tool-config", 1)
    # create_fixtures owns the initially absent directory.
    workspace.rmdir()
    env.update(PATH=os.pathsep.join((str(node.parent), str(git.parent), "C:/Program Files/Git/bin", str(payload), str(Path(os.environ["SystemRoot"]) / "System32"))),
               SHELL="C:/Program Files/Git/bin/bash.exe", GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
               GIT_CONFIG_SYSTEM=os.devnull, GIT_TERMINAL_PROMPT="0", LANG="C.UTF-8", PYTHONIOENCODING="utf-8",
               NO_COLOR="1", DO_NOT_TRACK="1")
    cases = create_fixtures(workspace, env, git)
    write(run / "fixtures.json", cases)
    overlay = base.prepare_fixture_runtime(run, payload)
    source_paths = [*HERE.glob("*.*"), ROOT / "bench/rtk/replay.py", ROOT / "bench/rtk/fixtures.py",
                    ROOT / "bench/optimization/replay.py", ROOT / "bench/optimization/fixtures.py",
                    *ROOT.joinpath("runtime").glob("*.json"), *ROOT.joinpath("runtime").glob("*.py"), *overlay,
                    node, git, Path(sys.executable), payload / "runtime-manifest.json", ROOT / "node_modules/typescript/package.json",
                    run / "fixtures.json", run / "packages.json", run / "diet/compile.json"]
    source_paths += [p for p in (run / "diet").rglob("*") if p.is_file() and p.suffix in (".js", ".ts")]
    source_paths += [p for p in (run / "mode").rglob("*") if p.is_file()]
    source_paths += [p for p in workspace.rglob("*") if p.is_file()]
    # Product runtime implementation and lock are proof inputs even though the pilot never edits them.
    source_paths += [p for p in (payload / "Lib/site-packages/headroom").rglob("*.py")]
    before = fingerprints(source_paths)
    environment = {"os": platform.platform(), "python": sys.version, "node": subprocess.check_output([str(node), "--version"], env=env).decode().strip(),
                   "encoding": "o200k_base", "model_calls": 0, "actions_runs": 0}
    server = ThreadingHTTPServer(("127.0.0.1", 0), base.FixtureHandler)
    server.capture_lock, server.captured = threading.Lock(), {}
    threading.Thread(target=server.serve_forever, daemon=True).start()
    engine = None
    result = {"status": "running", "environment": environment, "rows": [], "engines": [], "mode_processes": [], "before": before}
    try:
        if args.reuse_diet_proof:
            previous_path = args.reuse_diet_proof.resolve()
            previous_path.relative_to(ROOT / "work/proofs/context-tools")
            previous = json.loads(previous_path.read_text(encoding="utf-8"))
            old_prefix = previous_path.parent.relative_to(ROOT).as_posix() + "/diet/"
            new_prefix = run.relative_to(ROOT).as_posix() + "/diet/"
            unit_keys = [key for key in before if key.startswith(new_prefix) and key.endswith((".js", ".ts"))]
            require(previous["inputs_unchanged"] and previous["diet_upstream_tests"]["exit"] == 0, "reused_unit_proof_failed")
            require(all(before[key] == previous["before"].get(key.replace(new_prefix, old_prefix, 1)) for key in unit_keys)
                    and before[str(node)] == previous["before"].get(str(node)), "reused_unit_inputs_changed")
            result["diet_upstream_tests"] = {**previous["diet_upstream_tests"], "reused_from": str(previous_path.relative_to(ROOT)), "matched_files": len(unit_keys) + 1}
        else:
            tests = [str(p) for p in sorted((run / "diet/tests").glob("*.test.js"))]
            check = subprocess.run([str(node), "--test", *tests], env=env, cwd=ROOT, capture_output=True, timeout=30)
            lines = check.stdout.decode("utf-8", "replace").splitlines()
            result["diet_upstream_tests"] = {"exit": check.returncode, "summary": [line for line in lines if re.match(r"(?:ℹ|#) (?:tests|pass|fail) ", line)]}
            require(check.returncode == 0, "diet_upstream_tests_failed")
        started = time.perf_counter()
        filtered = subprocess.run([str(node), str(HERE / "diet.mjs"), str(run)], cwd=workspace, env=env,
                                  input=base.encoded(cases), capture_output=True, timeout=30)
        require(filtered.returncode == 0, "diet_bridge_failed")
        diet = json.loads(filtered.stdout)
        result["diet_process_ms"] = (time.perf_counter() - started) * 1000
        result["diet_protected_budget_rejected"] = diet["protectedBudgetRejected"]
        require(diet["protectedBudgetRejected"], "protected_budget_did_not_fail")
        diet_map = {row["case"]: row["rounds"] for row in diet["results"]}
        for repeat in range(3):
            # 44 calls per owned engine preserve the actual 60/min default rate limit.
            # The earlier one-engine stress attempt is retained; never bypass the product limiter.
            engine = base.Engine(controller, supervisor, payload, run / f"engine-{repeat}", server.server_port,
                                 {"thresholds": {"startup_timeout_seconds": 120}})
            engine_meta = {"repeat": repeat}
            result["engines"].append(engine_meta)
            engine_meta["ready"] = engine.start()
            mode_env = dict(env, CONTEXT_MODE_DATA_DIR=str(run / f"mode-state-{repeat}"))
            started = time.perf_counter()
            mode = MCP(node, run / "mode/server.mjs", workspace, mode_env, supervisor)
            try:
                hello = mode.request("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "headroom-local-pilot", "version": "1"}})
                mode.send({"jsonrpc": "2.0", "method": "notifications/initialized"})
                tools = mode.request("tools/list", {})
                require({"ctx_execute", "ctx_search", "ctx_index"} <= {tool["name"] for tool in tools["tools"]}, "mode_tools_missing")
                mode_meta = {"repeat": repeat, "startup_ms": (time.perf_counter() - started) * 1000,
                             "tool_count": len(tools["tools"]), "schema_tokens": count(base.encoded(tools["tools"]).decode()),
                             "instructions_tokens": count(hello.get("instructions", ""))}
                result["mode_processes"].append(mode_meta)
                for item in cases:
                    name = item["case"]
                    result["stage"] = f"repeat_{repeat}_{name}"
                    raw = envelope(item)
                    raw_body = fixtures.body(raw, "source_read" if item["full_read"] else "tool_result", 0, 0)
                    raw_tokens = count(base.encoded(raw_body).decode())
                    raw_stats, raw_forward = headroom(engine, server, raw, name, f"pilot-{repeat}-{name}-raw", tokenizer, item["full_read"], True)
                    result["rows"].append({"case": name, "repeat": repeat, "arm": "headroom", "raw_request_tokens": raw_tokens,
                                           "tool_text_tokens": count(raw), **raw_stats, **predicates(item, raw_forward)})
                    round_diet = diet_map[name][repeat]
                    if "error" in round_diet:
                        result["rows"].append({"case": name, "repeat": repeat, "arm": "diet_headroom", "rejected": round_diet["error"]})
                    else:
                        changed = {key: round_diet["channels"][key]["text"] for key in ("stdout", "stderr")}
                        diet_text = envelope({**changed, "exit_code": item["exit_code"]})
                        stats, forwarded = headroom(engine, server, diet_text, name, f"pilot-{repeat}-{name}-diet", tokenizer, item["full_read"], True)
                        result["rows"].append({"case": name, "repeat": repeat, "arm": "diet_headroom", "raw_request_tokens": raw_tokens,
                            "tool_text_tokens": count(diet_text), "filter_ms": round_diet["elapsed_ms"],
                            "original_archive": "not_provided_by_diet", "exit_and_channel_envelope": "preserved_by_pilot_adapter",
                            "manifest_verbatim": all(round_diet["channels"][key]["manifest"]["verbatim"] for key in ("stdout", "stderr")),
                            **stats, **predicates(item, diet_text), "post_headroom_required_lines_visible": predicates(item, forwarded)["required_lines_visible"]})
                    code = f"// fixture:{name}\nconst fs = process.getBuiltinModule('fs');\n"
                    for channel in ("stdout", "stderr"):
                        code += f"process.{channel}.write(fs.readFileSync({json.dumps(str(workspace / (name + '.' + channel)))}));\n"
                    code += f"process.exitCode = {item['exit_code']};"
                    arguments = {"language": "javascript", "code": code, "timeoutMs": 15000}
                    if item["query"]:
                        arguments["intent"] = item["query"]
                    require(all(line not in base.encoded(arguments).decode("utf-8") for line in item["protect"]), "expected_line_leaked_into_execute_query")
                    reply, elapsed = mode.call("ctx_execute", arguments)
                    visible = text(reply)
                    storage, handle = mode_storage(run / f"mode-state-{repeat}", "ctx_execute:" + code.splitlines()[0], item["stdout"] + item["stderr"])
                    search_tokens, search_args_tokens, search_ok = 0, 0, None
                    if handle and item["query"]:
                        search_args = {"queries": [item["query"]], "source": handle}
                        require(all(line not in base.encoded(search_args).decode("utf-8") for line in item["protect"]), "expected_line_leaked_into_search_query")
                        found, _ = mode.call("ctx_search", search_args)
                        search_tokens, search_args_tokens = count(text(found)), count(base.encoded(search_args).decode())
                        search_ok = not found.get("isError", False) and all(line in text(found) for line in item["protect"])
                    stats, forwarded = headroom(engine, server, visible, name, f"pilot-{repeat}-{name}-mode", tokenizer, item["full_read"])
                    result["rows"].append({"case": name, "repeat": repeat, "arm": "mode_headroom", "raw_request_tokens": raw_tokens,
                        "tool_text_tokens": count(visible), "mode_ms": elapsed, "mode_is_error": reply.get("isError", False),
                        "expected_is_error": item["exit_code"] != 0, "invocation_argument_tokens": count(base.encoded(arguments).decode()),
                        "search_reply_tokens": search_tokens, "search_argument_tokens": search_args_tokens, "search_required_lines_visible": search_ok,
                        "original_storage": storage, **stats, **predicates(item, visible),
                        "post_headroom_required_lines_visible": predicates(item, forwarded)["required_lines_visible"]})
                print(json.dumps({"repeat": repeat, "completed_cases": len(cases)}), flush=True)
                shell, _ = mode.call("ctx_execute", {"language": "shell", "code": "printf 'FAIL: SHELL_STDOUT_ONLY_9007199254740993\\n'; exit 1", "timeoutMs": 15000})
                mode_meta["shell_stdout_exit1"] = {"is_error": shell.get("isError", False), "failure_visible": "SHELL_STDOUT_ONLY_9007199254740993" in text(shell), "exit1_visible": bool(re.search(r"exit[^\n]*1", text(shell), re.I))}
                collection = (workspace / "structured.jsonl").read_text(encoding="utf-8")
                target = collection.splitlines()[777]
                raw_query = envelope({"exit_code": 0, "stdout": collection, "stderr": ""})
                baseline, _ = headroom(engine, server, raw_query, "structured_query", f"query-{repeat}-raw", tokenizer, True, True)
                index_args = {"path": "structured.jsonl", "source": "structured-query"}
                indexed, _ = mode.call("ctx_index", index_args)
                search_args = {"queries": ["row_0777"], "source": "structured.jsonl"}
                searched, _ = mode.call("ctx_search", search_args)
                query_code = ("// fixture:structured_query\nconst lines = process.getBuiltinModule('fs').readFileSync("
                              + json.dumps(str(workspace / "structured.jsonl"))
                              + ", 'utf8').trimEnd().split('\\n');\nconsole.log('count=' + lines.length);\n"
                              + "console.log(lines.find(line => line.includes('\"id\":\"row_0777\"')));")
                query_args = {"language": "javascript", "code": query_code, "timeoutMs": 15000}
                queried, query_ms = mode.call("ctx_execute", query_args)
                query_visible = text(queried)
                optimized, _ = headroom(engine, server, query_visible, "structured_query", f"query-{repeat}-mode", tokenizer, True)
                mode_meta["structured_query"] = {
                    "row_count": 1000, "raw_text_tokens": count(collection), "baseline_headroom": baseline,
                    "execute_text_tokens": count(query_visible), "execute_argument_tokens": count(base.encoded(query_args).decode()),
                    "index_reply_tokens": count(text(indexed)), "index_argument_tokens": count(base.encoded(index_args).decode()),
                    "search_reply_tokens": count(text(searched)), "search_argument_tokens": count(base.encoded(search_args).decode()),
                    "search_exact_record": not searched.get("isError", False) and target in text(searched),
                    "execute_exact_record_and_count": not queried.get("isError", False) and target in query_visible and "count=1000" in query_visible,
                    "mode_headroom": optimized, "execute_ms": query_ms,
                    "scope": "handwritten task-specific count and exact row selection; not full JSON compression"}
            finally:
                mode.close()
                if result["mode_processes"]:
                    result["mode_processes"][-1]["normal_exit"] = mode.stopped
                    result["mode_processes"][-1]["exit_code"] = mode.process.returncode
                    owners = list((run / f"mode-state-{repeat}").glob("projects/*/owners/*"))
                    result["mode_processes"][-1]["owner_files_after_eof"] = sum(path.is_file() for path in owners)
                if sys.exc_info()[0] is None:
                    require(mode.stopped, "mode_did_not_close_normally")
                    require(not any(path.is_file() for path in owners), "mode_owner_cleanup_failed")
            engine.close()
            engine_meta["stop"] = engine.stopped
            print(json.dumps({"repeat": repeat, "engine_stop": engine.stopped["shutdown"]}), flush=True)
        result["status"] = "evaluation_completed"
    except Exception as error:
        result["status"] = "pilot_failed"
        result["failure"] = str(error)[:120] if isinstance(error, AssertionError) else type(error).__name__
        result["failure_frames"] = [{"file": str(Path(frame.filename)), "line": frame.lineno, "function": frame.name}
                                    for frame in traceback.extract_tb(error.__traceback__)[-7:]]
        if isinstance(error, KeyError) and len(error.args) == 1 and re.fullmatch(r"[A-Za-z_]{1,50}", str(error.args[0])):
            result["failure_key"] = error.args[0]
    finally:
        try:
            if engine is not None and engine.stopped is None:
                engine.close()
            if engine is not None and result["engines"]:
                result["engines"][-1]["stop"] = engine.stopped
        finally:
            server.shutdown()
            server.server_close()
            result["after"] = fingerprints(source_paths)
            result["inputs_unchanged"] = result["before"] == result["after"]
            result["fixture_network"] = base.fixture_network_evidence(run)
            write(run / "result.json", result)
    require(result["status"] == "evaluation_completed" and result["inputs_unchanged"], "pilot_incomplete_or_changed")
    print(json.dumps({"status": result["status"], "cases": len(cases), "rows": len(result["rows"]),
                      "fingerprints": len(before), "normal_mode_exits": len(result["mode_processes"]),
                      "inputs_unchanged": result["inputs_unchanged"]}))


if __name__ == "__main__":
    main()
