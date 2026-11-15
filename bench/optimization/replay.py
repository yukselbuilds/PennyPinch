"""Finite Windows engine A/B replay with loopback targets and local-only model assets."""
from __future__ import annotations

import argparse
import ctypes
from ctypes import wintypes
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import http.client
import importlib.util
import json
import math
import os
from pathlib import Path
import platform
import queue
import re
import site
import subprocess
import sys
import threading
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
SENTINEL = "headroom-local-fixture-only"


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


fixtures = load_module("optimization_fixtures", HERE / "fixtures.py")


def encoded(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def require(value, code):
    if not value:
        raise AssertionError(code)


def write_new(path, value):
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2, ensure_ascii=True)
        stream.write("\n")


def prepare_fixture_runtime(run, payload, *, ccr_injection=None):
    """Use local-only ML assets for this run while preserving the app's statistics profile."""
    require(ccr_injection in (None, "eager", "lazy"), "fixture_ccr_policy")
    runtime = ROOT / "runtime"
    overlay = run / "fixture-runtime"
    overlay.mkdir()
    for name in ("desktop_controller.py", "desktop_helper.py", "cache_control_shim.py",
                 "optimization_control.py", "optimization-sources.json"):
        data = (runtime / name).read_bytes()
        if name == "desktop_helper.py":
            anchor = b'PROFILE_ENV = {\n'
            if anchor not in data:
                anchor = b'PROFILE_ENV = {\r\n'
            overrides = (b'    "HF_HUB_OFFLINE": "1",', b'    "TRANSFORMERS_OFFLINE": "1",')
            if ccr_injection is not None:
                overrides += (f'    "HEADROOM_CCR_TOOL_INJECTION": "{ccr_injection}",'.encode("ascii"),)
            require(data.count(anchor) == 1 and all(line.split(b':')[0] not in data for line in overrides),
                    "fixture_profile_anchor")
            newline = anchor[len(b'PROFILE_ENV = {'):]
            data = data.replace(anchor, anchor + newline.join(overrides) + newline, 1)
        with (overlay / name).open("xb") as stream:
            stream.write(data)
    manifest = json.loads((runtime / "manifest.json").read_text(encoding="utf-8"))
    manifest["engine"]["payload_root"] = str(payload)
    write_new(overlay / "manifest.json", manifest)
    return list(overlay.iterdir())


def fixture_network_evidence(run):
    artifact_bytes = sum(
        path.stat().st_size
        for engine in run.glob("engine*")
        for path in (engine / "state/home/.cache/huggingface/hub").rglob("*")
        if path.is_file() and path.name != "CACHEDIR.TAG"
    )
    return {"hf_hub_offline": True, "transformers_offline": True, "upstream": "owned_loopback_fixture",
            "huggingface_hub_payload_bytes": artifact_bytes, "packet_capture": False}


def percentile(values):
    return sorted(values)[math.ceil(len(values) * .95) - 1]


class FixtureHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_args):
        pass

    def do_HEAD(self):
        self.send_response(200 if self.path == "/" else 404)
        self.send_header("Content-Length", "0")
        self.end_headers()

    def do_POST(self):
        size = int(self.headers.get("Content-Length", "0"))
        if self.path != "/v1/responses" or not 0 < size <= 2 * 1024 * 1024:
            self.send_error(400)
            return
        body = json.loads(self.rfile.read(size))
        case = self.headers.get("X-Proof-Case", "")
        row = {"body": body, "auth_preserved": self.headers.get("Authorization") == "Bearer " + SENTINEL}
        with self.server.capture_lock:
            self.server.captured.setdefault(case, []).append(row)
        if "-upstream_error-" in case:
            raw = encoded({"error": {"type": "fixture_throttle", "message": "Local fixture throttle"}})
            self.send_response(429)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(raw)))
            self.send_header("Retry-After", "1")
            self.end_headers()
            self.wfile.write(raw)
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Connection", "close")
        self.end_headers()
        response = {"id": "resp-fixture", "model": body["model"], "status": "in_progress", "output": []}
        events = [
            ("response.created", {"type": "response.created", "response": response}),
            ("response.output_text.delta", {"type": "response.output_text.delta", "item_id": "m",
             "output_index": 0, "content_index": 0, "delta": "FIXTURE_OK"}),
            ("response.completed", {"type": "response.completed", "response": {**response, "status": "completed"}}),
        ]
        for name, event in events:
            self.wfile.write(b"event: " + name.encode() + b"\ndata: " + encoded(event) + b"\n\n")
            self.wfile.flush()
        self.close_connection = True


class Memory(ctypes.Structure):
    _fields_ = [
        ("cb", wintypes.DWORD), ("faults", wintypes.DWORD),
        ("peak_ws", ctypes.c_size_t), ("ws", ctypes.c_size_t),
        ("peak_paged", ctypes.c_size_t), ("paged", ctypes.c_size_t),
        ("peak_nonpaged", ctypes.c_size_t), ("nonpaged", ctypes.c_size_t),
        ("pagefile", ctypes.c_size_t), ("peak_pagefile", ctypes.c_size_t),
        ("private", ctypes.c_size_t),
    ]


class Resources:
    def __init__(self, pids, interval_ms):
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        self.psapi = ctypes.WinDLL("psapi", use_last_error=True)
        self.kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        self.kernel.OpenProcess.restype = wintypes.HANDLE
        self.kernel.CloseHandle.argtypes = [wintypes.HANDLE]
        self.kernel.GetProcessTimes.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
        self.psapi.GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(Memory), wintypes.DWORD]
        self.handles = []
        self.stop = threading.Event()
        self.interval = interval_ms / 1000
        self.samples = []
        try:
            for pid in pids:
                handle = self.kernel.OpenProcess(0x410, False, pid)
                require(handle, "owned_resource_handle_unavailable")
                self.handles.append((pid, handle))
            self.samples.append(self.sample())
            self.thread = threading.Thread(target=self.collect, daemon=True)
            self.thread.start()
        except BaseException:
            for _, handle in self.handles:
                self.kernel.CloseHandle(handle)
            raise

    def sample(self):
        result = {"harness_cpu_ms": 0, "engine_cpu_ms": 0, "engine_private_bytes": 0,
                  "harness_private_bytes": 0, "working_set_bytes": 0}
        for pid, handle in self.handles:
            creation, exit_time, kernel_time, user_time = (wintypes.FILETIME() for _ in range(4))
            memory = Memory()
            memory.cb = ctypes.sizeof(memory)
            require(self.kernel.GetProcessTimes(handle, ctypes.byref(creation), ctypes.byref(exit_time),
                                                ctypes.byref(kernel_time), ctypes.byref(user_time)),
                    "cpu_sample_unavailable")
            require(self.psapi.GetProcessMemoryInfo(handle, ctypes.byref(memory), ctypes.sizeof(memory)),
                    "memory_sample_unavailable")
            cpu_ms = sum(value.dwHighDateTime * 2**32 + value.dwLowDateTime
                         for value in (kernel_time, user_time)) / 10000
            group = "harness" if pid == os.getpid() else "engine"
            result[group + "_cpu_ms"] += cpu_ms
            result[group + "_private_bytes"] += memory.private
            result["working_set_bytes"] += memory.ws
        return result

    def collect(self):
        try:
            while not self.stop.wait(self.interval):
                self.samples.append(self.sample())
        except BaseException:
            self.samples.append(None)

    def finish(self):
        self.stop.set()
        self.thread.join(2)
        try:
            require(not self.thread.is_alive() and None not in self.samples, "resource_sampling_failed")
            self.samples.append(self.sample())
            first, last = self.samples[0], self.samples[-1]
            return {
                "harness_cpu_ms": last["harness_cpu_ms"] - first["harness_cpu_ms"],
                "engine_cpu_ms": last["engine_cpu_ms"] - first["engine_cpu_ms"],
                "observed_engine_private_bytes": max(row["engine_private_bytes"] for row in self.samples),
                "observed_harness_private_bytes": max(row["harness_private_bytes"] for row in self.samples),
                "observed_working_set_bytes": max(row["working_set_bytes"] for row in self.samples),
                "resource_samples": len(self.samples),
            }
        finally:
            for _, handle in self.handles:
                self.kernel.CloseHandle(handle)


class Engine:
    def __init__(self, controller, supervisor, payload, directory, upstream_port, plan):
        self.controller, self.supervisor, self.payload = controller, supervisor, payload
        self.plan, self.directory = plan, directory
        self.job = supervisor.KillOnCloseJob()
        self.process = None
        self.generation = None
        self.frames = queue.Queue()
        self.stopped = None
        for name in ("workspace", "config"):
            (directory / name).mkdir(parents=True)
        upstream = f"http://127.0.0.1:{upstream_port}"
        write_new(directory / "workspace/settings.json",
                  {"openai_base_url": upstream + "/v1", "anthropic_base_url": upstream})
        self.environment = supervisor.safe_environment(
            directory / "state", directory / "workspace", directory / "config", 1)
        self.environment["HEADROOM_DESKTOP_PROTECTED_TOOLS"] = "[]"

    def read_frames(self):
        for line in iter(lambda: self.process.stdout.readline(64 * 1024), b""):
            self.frames.put(line)

    def receive(self, request_id, timeout):
        frame = self.frames.get(timeout=timeout)
        decoded = self.controller.decode_frame(
            frame, expected_generation=self.generation, expected_request_id=request_id)
        if decoded["event"] == "error":
            raise AssertionError("controller_" + decoded["error_code"])
        return decoded

    def start(self):
        overlay = self.directory.parent / "fixture-runtime"
        command = [str(self.payload / "python.exe"), "-I", "-B", "-u",
                   str(overlay / "desktop_controller.py"), "--manifest", str(overlay / "manifest.json"),
                   "--state-root", str(self.directory / "state"),
                   "--workspace", str(self.directory / "workspace"), "--config", str(self.directory / "config")]
        self.process = subprocess.Popen(command, cwd=self.payload, env=self.environment,
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                        creationflags=subprocess.CREATE_NO_WINDOW)
        self.job.assign(self.process.pid)
        threading.Thread(target=self.read_frames, daemon=True).start()
        request_id = uuid.uuid4().hex
        started = time.perf_counter()
        self.process.stdin.write(encoded({"protocol_version": 1, "event": "start", "request_id": request_id}) + b"\n")
        self.process.stdin.flush()
        ready = self.receive(request_id, self.plan["thresholds"]["startup_timeout_seconds"])
        require(ready["event"] == "ready", "ready_event_required")
        self.generation, self.port = ready["runtime_generation"], ready["listener_port"]
        self.pids = [self.process.pid, ready["helper_pid"], ready["engine_pid"]]
        require(self.supervisor.listener_pids(self.port, ctypes.WinDLL("iphlpapi")) ==
                {"127.0.0.1": ready["engine_pid"]}, "engine_listener_ownership")
        config = ready["config"]
        require(config["mode"] == "cache" and config["savings_profile"] == "coding"
                and config["ttl_seconds"] == 7200 and config["telemetry_enabled"]
                and not config["beacon_enabled"] and not config["model_router_enabled"]
                and not config["output_shaper_enabled"], "effective_profile_mismatch")
        return {"startup_ms": (time.perf_counter() - started) * 1000,
                "effective_profile": config, "stderr_status": "redirected_to_null_contents_unknown"}

    def close(self):
        try:
            if self.process is None or self.generation is None:
                return
            require(self.process.poll() is None, "engine_exited_before_stop")
            request_id = uuid.uuid4().hex
            self.process.stdin.write(encoded({"protocol_version": 1, "event": "stop",
                                             "request_id": request_id, "runtime_generation": self.generation}) + b"\n")
            self.process.stdin.flush()
            stopped = self.receive(request_id, 90)
            self.process.wait(timeout=10)
            self.stopped = {key: stopped[key] for key in (
                "shutdown", "engine_exit_code", "owned_port_closed", "force_fallback")}
            require(stopped["event"] == "stopped" and stopped["shutdown"] == "graceful"
                    and stopped["engine_exit_code"] == 0 and stopped["owned_port_closed"]
                    and not stopped["force_fallback"] and self.process.returncode == 0,
                    "graceful_shutdown_required")
        finally:
            self.job.close()
            if self.process is not None and self.process.poll() is None:
                self.process.wait(timeout=10)


def request(port, body, key, timeout):
    data = encoded(body)
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=timeout)
    first_delta = None
    started = time.perf_counter()
    try:
        connection.request("POST", "/v1/responses", data,
                           {"Content-Type": "application/json", "Authorization": "Bearer " + SENTINEL,
                            "X-Proof-Case": key, "x-headroom-session-id": key.rsplit("-t", 1)[0]})
        response = connection.getresponse()
        first_response = (time.perf_counter() - started) * 1000
        delta, completed = [], False
        if response.status == 429:
            require(json.loads(response.read(65536))["error"]["type"] == "fixture_throttle",
                    "fixture_error_payload_changed")
            require(response.getheader("Retry-After") == "1", "retry_after_changed")
        else:
            require(response.status == 200, "unexpected_http_status")
            while True:
                line = response.readline(65536)
                if not line:
                    break
                if not line.startswith(b"data: "):
                    continue
                event = json.loads(line[6:])
                if event["type"] == "response.output_text.delta":
                    if first_delta is None:
                        first_delta = (time.perf_counter() - started) * 1000
                    delta.append(event["delta"])
                if event["type"] == "response.completed":
                    completed = True
            require("".join(delta) == "FIXTURE_OK" and completed, "stream_content_changed")
        return {"status": response.status, "first_response_ms": first_response,
                "first_fixture_delta_ms": first_delta,
                "total_ms": (time.perf_counter() - started) * 1000, "model_ttft_ms": None}
    finally:
        connection.close()


def retrieval(port, content_hash, expected, timeout):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=timeout)
    try:
        connection.request("POST", "/v1/retrieve", encoded({"hash": content_hash}),
                           {"Content-Type": "application/json"})
        response = connection.getresponse()
        require(response.status == 200 and json.loads(response.read(2 * 1024 * 1024))["original_content"] == expected,
                "ccr_original_mismatch")
    finally:
        connection.close()


def validate_body(original, forwarded, previous, protected):
    require(forwarded["model"] == original["model"] and forwarded["reasoning"] == original["reasoning"],
            "model_or_reasoning_changed")
    calls = lambda body: [(item["call_id"], item["name"], item["arguments"])
                          for item in body["input"] if item.get("type") == "function_call"]
    require(calls(forwarded) == calls(original), "call_identity_changed")
    before, after = fixtures.outputs(original), fixtures.outputs(forwarded)
    require(set(before) == set(after), "tool_output_identity_changed")
    if protected:
        require(before == after, "protected_content_changed")
    if previous is not None:
        require(forwarded["input"][:len(previous["input"])] == previous["input"], "forwarded_prefix_changed")
        require(forwarded["tools"] == previous["tools"], "forwarded_tool_schema_changed")


def fingerprints(paths):
    return {path.relative_to(ROOT).as_posix(): sha(path) for path in sorted(set(paths))}


def replay(plan, run, controller, supervisor, payload, tokenizer):
    server = ThreadingHTTPServer(("127.0.0.1", 0), FixtureHandler)
    server.daemon_threads = True
    server.capture_lock, server.captured = threading.Lock(), {}
    threading.Thread(target=server.serve_forever, daemon=True).start()
    rows, engines, error_rows = [], [], []
    try:
        for iteration in range(plan["process_runs"]):
            engine = Engine(controller, supervisor, payload, run / f"engine-{iteration}",
                            server.server_port, plan)
            previous = {}
            engines.append({})
            try:
                engines[-1].update(engine.start())
                order = plan["cases"][iteration % len(plan["cases"]):] + plan["cases"][:iteration % len(plan["cases"])]
                for case in order:
                    bodies = fixtures.conversation(case, plan["model"], plan["reasoning_effort"],
                                                   plan["conversation_turns"])
                    for turn, body in enumerate(bodies):
                        arms = ("control", "headroom") if (iteration + turn) % 2 == 0 else ("headroom", "control")
                        for arm in arms:
                            key = f"r{iteration}-{case}-t{turn}-{arm}"
                            port = engine.port if arm == "headroom" else server.server_port
                            sampler = Resources([os.getpid()] + (engine.pids if arm == "headroom" else []),
                                                plan["memory_sample_interval_ms"])
                            try:
                                outcome = request(port, body, key, plan["thresholds"]["request_timeout_seconds"])
                            finally:
                                resource = sampler.finish()
                            with server.capture_lock:
                                captured = server.captured.pop(key, [])
                            require(len(captured) == 1 and captured[0]["auth_preserved"], "single_owned_upstream_request")
                            forwarded = captured[0]["body"]
                            if arm == "control":
                                require(forwarded == body, "control_payload_changed")
                            else:
                                validate_body(body, forwarded, previous.get(case), case in fixtures.PROTECTED)
                                previous[case] = forwarded
                            recovered = 0
                            for call_id, text in fixtures.outputs(forwarded).items():
                                for content_hash in set(re.findall(
                                        r"(?:Retrieve more: hash=|<<ccr:)([a-fA-F0-9]{12,24})(?=[^a-fA-F0-9]|$)", text)):
                                    require(arm == "headroom", "control_contains_ccr")
                                    retrieval(port, content_hash, fixtures.outputs(body)[call_id],
                                              plan["thresholds"]["request_timeout_seconds"])
                                    recovered += 1
                            original_tokens = len(tokenizer.encode(encoded(body).decode(), disallowed_special=()))
                            forwarded_tokens = len(tokenizer.encode(encoded(forwarded).decode(), disallowed_special=()))
                            rows.append({
                                "process_run": iteration, "case": case, "turn": turn, "arm": arm,
                                "conversation_state": "new" if turn == 0 else "appended_history",
                                "process_first_request": case == order[0] and turn == 0,
                                "input_sha256": hashlib.sha256(encoded(body)).hexdigest(),
                                "forwarded_sha256": hashlib.sha256(encoded(forwarded)).hexdigest(),
                                "original_tokens": original_tokens, "forwarded_tokens": forwarded_tokens,
                                "saved_tokens": original_tokens - forwarded_tokens,
                                "saved_percent": 100 * (original_tokens - forwarded_tokens) / original_tokens,
                                "ccr_retrievals_verified": recovered, "prefix_and_integrity": "pass",
                                "provider_cached_input_tokens": None, "invoice_savings": None,
                                "response_quality": None, **outcome, **resource,
                            })
                            write_new(run / f"row-{len(rows):03}.json", rows[-1])
                for arm in ("control", "headroom"):
                    key = f"r{iteration}-upstream_error-t0-{arm}"
                    body = fixtures.conversation("short_json", plan["model"], plan["reasoning_effort"], 1)[0]
                    sampler = Resources([os.getpid()] + (engine.pids if arm == "headroom" else []),
                                        plan["memory_sample_interval_ms"])
                    try:
                        outcome = request(engine.port if arm == "headroom" else server.server_port, body, key,
                                          plan["thresholds"]["request_timeout_seconds"])
                    finally:
                        resource = sampler.finish()
                    require(outcome["status"] == 429, "upstream_error_not_preserved")
                    with server.capture_lock:
                        captured = server.captured.pop(key, [])
                    error_row = {"process_run": iteration, "arm": arm,
                                 "upstream_attempts": len(captured), **outcome, **resource}
                    error_rows.append(error_row)
                    write_new(run / f"error-{len(error_rows):03}.json", error_row)
                    require(len(captured) == plan["upstream_error_expected_attempts"][arm]
                            and all(attempt["auth_preserved"] for attempt in captured),
                            "error_retry_identity")
                    for attempt in captured:
                        validate_body(body, attempt["body"], None, False)
            finally:
                engine.close()
                engines[-1]["stop"] = engine.stopped
    finally:
        server.shutdown()
        server.server_close()
    pairs = {}
    for row in rows:
        pairs.setdefault((row["process_run"], row["case"], row["turn"]), {})[row["arm"]] = row
    added = [pair["headroom"]["total_ms"] - pair["control"]["total_ms"]
             for key, pair in pairs.items() if key[2] > 0]
    cpu = [row["engine_cpu_ms"] + row["harness_cpu_ms"] for row in rows
           if row["arm"] == "headroom" and row["turn"] > 0]
    log_rows = [row for row in rows if row["arm"] == "headroom" and row["case"] == "repetitive_logs"]
    thresholds = plan["thresholds"]
    checks = {
        "paired_workloads": len(pairs) == plan["process_runs"] * len(plan["cases"]) * plan["conversation_turns"],
        "warm_added_p95_ms": percentile(added) <= thresholds["warm_added_p95_ms"],
        "warm_cpu_p95_ms": percentile(cpu) <= thresholds["warm_cpu_p95_ms"],
        "engine_private_limit": max(row["observed_engine_private_bytes"] for row in rows)
                                <= thresholds["engine_observed_private_bytes"],
        "log_token_benefit": all(row["saved_percent"] >= 100 * thresholds["repetitive_logs_min_saved_fraction"] for row in log_rows),
        "ccr_observed_and_verified": sum(row["ccr_retrievals_verified"] for row in rows) > 0,
    }
    return {"passed": all(checks.values()), "checks": checks, "rows": rows, "engines": engines,
            "error_rows": error_rows,
            "warm_added_p95_ms": percentile(added), "warm_cpu_p95_ms": percentile(cpu),
            "external_provider_calls": 0, "error_responses_verified": plan["process_runs"] * 2,
            "cold_scope": "fresh process and owned state; prepared tokenizer data; OS cache uncontrolled",
            "token_basis": plan["token_basis"], "token_encoding": plan["token_encoding"],
            "resource_scope": "harness+fixture and held controller/helper/engine PIDs; Rust ingress excluded",
            "stderr_status": "controller_redirected_to_null_contents_unknown"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-check", action="store_true")
    parser.add_argument("--run", action="store_true")
    args = parser.parse_args()
    fixtures.self_check()
    plan = json.loads((HERE / "plan.json").read_text(encoding="utf-8"))
    require(plan["schema_version"] == 1 and tuple(plan["cases"]) == fixtures.CASES
            and 1 <= plan["process_runs"] <= 5 and 2 <= plan["conversation_turns"] <= 8
            and 10 <= plan["memory_sample_interval_ms"] <= 1000,
            "invalid_benchmark_plan")
    if args.self_check and not args.run:
        print(json.dumps({"passed": True, "fixture_cases": len(fixtures.CASES), "network_calls": 0}))
        return 0
    require(args.run and os.name == "nt" and Path.cwd().resolve() == ROOT, "explicit_windows_run_required")
    controller = load_module("benchmark_controller", ROOT / "runtime/desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(ROOT / "runtime/manifest.json")
    require(sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "pinned_python_required")
    supervisor_path = payload / manifest["engine"]["runner"]
    supervisor = load_module("benchmark_supervisor", supervisor_path)
    cache = controller.verify_tokenizer_cache(payload)
    os.environ["TIKTOKEN_CACHE_DIR"] = str(cache)
    site.addsitedir(str(payload / "Lib/site-packages"))
    import tiktoken
    tokenizer = tiktoken.get_encoding(plan["token_encoding"])
    run = ROOT / "work/proofs/optimization/runs" / str(uuid.uuid4())
    run.mkdir(parents=True)
    sources = [Path(__file__), HERE / "fixtures.py", HERE / "plan.json", ROOT / "runtime/manifest.json",
               ROOT / "runtime/desktop_controller.py", ROOT / "runtime/desktop_helper.py",
               ROOT / "runtime/cache_control_shim.py", ROOT / "runtime/optimization_control.py",
               ROOT / "runtime/optimization-sources.json", supervisor_path, Path(sys.executable)]
    sources.extend((payload / "Lib/site-packages/headroom").rglob("*.py"))
    sources.extend(path for path in cache.iterdir() if path.is_file())
    sources.extend(prepare_fixture_runtime(run, payload))
    before = fingerprints(sources)
    write_new(run / "plan.json", plan)
    result = {"passed": False}
    try:
        result = replay(plan, run, controller, supervisor, payload, tokenizer)
    except BaseException as error:
        result.update(passed=False, error_class=type(error).__name__,
                      error_code=str(error) if isinstance(error, AssertionError) else "bounded_replay_failed")
    finally:
        result.update(before=before, after=fingerprints(sources), fixture_network=fixture_network_evidence(run),
                      environment={"windows_build": list(sys.getwindowsversion()[:3]), "python": sys.version,
                                   "processor": platform.processor(), "logical_cpus": os.cpu_count()},
                      run=run.relative_to(ROOT).as_posix())
        result["inputs_unchanged"] = result["before"] == result["after"]
        result["passed"] = (result["passed"] and result["inputs_unchanged"]
                            and result["fixture_network"]["huggingface_hub_payload_bytes"] == 0)
        write_new(run / "result.json", result)
    print(json.dumps({key: value for key, value in result.items()
                      if key not in ("before", "after", "rows", "engines", "error_rows")}))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

