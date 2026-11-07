"""Hidden-console adapter around the pinned H006 runtime supervisor."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import re
import sys
import threading
import time
import uuid
from pathlib import Path
from typing import Any


PROFILE_ENV = {
    "HEADROOM_CCR_TTL_SECONDS": "7200",
    "HEADROOM_TELEMETRY": "on",
    "HEADROOM_BEACON": "off",
    "HEADROOM_LOG_MESSAGES": "off",
    "HEADROOM_LOG_LEVEL": "info",
    "HEADROOM_UPDATE_CHECK": "off",
    "HEADROOM_MODE": "cache",
    "HEADROOM_SAVINGS_PROFILE": "coding",
    "HEADROOM_OUTPUT_SHAPER": "0",
    "HEADROOM_MODEL_ROUTER_ENABLED": "0",
    "HEADROOM_NO_MEMORY_TOOLS": "1",
    "HEADROOM_NO_MEMORY_CONTEXT": "1",
}
SHIM_MARKER = re.compile(
    rb"headroom-desktop-cache-shim:version=(0\.41\.0);prefix_sha256=([0-9a-f]{64});shim_sha256=([0-9a-f]{64})"
)
ROUTER_MARKER = b"headroom-desktop-model-router:disabled"
CONTROL_MARKER = re.compile(rb"headroom-desktop-optimization-control:module_sha256=([0-9a-f]{64});sources_sha256=([0-9a-f]{64})")


def _load_controller(overlay_root: Path):
    path = overlay_root / "desktop_controller.py"
    spec = importlib.util.spec_from_file_location("headroom_desktop_protocol", path)
    if spec is None or spec.loader is None:
        raise RuntimeError("controller-protocol-unavailable")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class BoundedEngineOutput:
    """Drain engine stdout without retaining logs or content."""

    NEEDLES = {
        b"headroom-runtime-console=allocated": "engine_console_allocated",
        b"application shutdown complete": "lifespan_complete",
        b"timeout graceful shutdown exceeded": "request_drain_timeout",
        b"headroom proxy session summary": "proxy_summary",
        b"local telemetry: enabled": "local_telemetry_enabled",
        b"anonymous upload beacon: disabled": "anonymous_beacon_disabled",
        ROUTER_MARKER: "model_router_disabled",
    }

    def __init__(self, stream: Any) -> None:
        self.flags: set[str] = set()
        self._overlap = b""
        self._line_tail = b""
        self._lock = threading.Lock()
        self.shim_marker: tuple[str, str, str] | None = None
        self.control_marker: tuple[str, str] | None = None
        self._thread = threading.Thread(target=self._drain, args=(stream,), daemon=True)
        self._thread.start()

    def _drain(self, stream: Any) -> None:
        try:
            while True:
                read_available = getattr(stream, "read1", stream.read)
                chunk = read_available(4096)
                if not chunk:
                    return
                combined = self._overlap + chunk.lower()
                with self._lock:
                    for needle, name in self.NEEDLES.items():
                        if needle in combined:
                            self.flags.add(name)
                    self._overlap = combined[-256:]
                line_data = self._line_tail + chunk
                lines = line_data.split(b"\n")
                self._line_tail = lines.pop()[-1024:]
                for line in lines:
                    match = SHIM_MARKER.search(line[:1024])
                    if match:
                        marker = tuple(item.decode("ascii") for item in match.groups())
                        with self._lock:
                            self.shim_marker = marker
                    match = CONTROL_MARKER.search(line[:1024])
                    if match:
                        with self._lock:
                            self.control_marker = tuple(item.decode("ascii") for item in match.groups())
        finally:
            stream.close()

    def has(self, name: str) -> bool:
        with self._lock:
            return name in self.flags

    def finish(self) -> bool:
        self._thread.join(timeout=5)
        return not self._thread.is_alive()

    def wait_for_start_markers(self, timeout: float = 5.0) -> bool:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            with self._lock:
                ready = (
                    self.shim_marker is not None
                    and self.control_marker is not None
                    and "local_telemetry_enabled" in self.flags
                    and "anonymous_beacon_disabled" in self.flags
                    and "model_router_disabled" in self.flags
                )
            if ready:
                return True
            time.sleep(0.02)
        return False


class VersionedControlInput:
    """Translate v1 stdio stop events to H006's private stop command."""

    def __init__(self, stream: Any, state: dict[str, Any], controller_protocol: Any) -> None:
        self._stream = stream
        self._state = state
        self._protocol = controller_protocol

    def readline(self, size: int = -1) -> str:
        raw = self._stream.readline(min(size, 4098) if size >= 0 else 4098)
        if not raw:
            return raw
        try:
            value = self._protocol.decode_request(raw.encode("utf-8"), self._state["generation"])
        except self._protocol.ProtocolError:
            return '{"command":"invalid"}\n'
        if value["event"] != "stop":
            return '{"command":"invalid"}\n'
        self._state["request_id"] = value["request_id"]
        return '{"command":"stop"}\n'


def _shutdown_kind(source: dict[str, Any], output: BoundedEngineOutput | None) -> str:
    if source.get("shutdown") == "graceful" and output is not None and not output.has("request_drain_timeout"):
        return "graceful"
    return "incomplete"


def _load_h006_helper(path: Path, expected_sha256: str):
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if digest != expected_sha256:
        raise RuntimeError("packaged-supervisor-source-mismatch")
    spec = importlib.util.spec_from_file_location("headroom_packaged_runtime", path)
    if spec is None or spec.loader is None:
        raise RuntimeError("packaged-supervisor-unavailable")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _engine_bootstrap(payload_root: Path, overlay_root: Path, manifest: dict[str, Any], generation: str, control_token: str) -> str:
    engine = manifest["engine"]
    shim_file = overlay_root / manifest["shim"]["file"]
    prefix_sha = engine["prefix_tracker_sha256"]
    shim_sha = manifest["shim"]["sha256"]
    expected_version = engine["version"]
    expected_version_literal = repr(expected_version)
    return (
        "import ctypes,os,signal,site,sys; from ctypes import wintypes; "
        f"sys.prefix=sys.exec_prefix={str(payload_root)!r}; "
        f"site.addsitedir({str(payload_root / engine['site_packages'])!r}); "
        "kernel32=ctypes.WinDLL('kernel32',use_last_error=True); "
        "kernel32.GetConsoleWindow.argtypes=[]; "
        "kernel32.GetConsoleWindow.restype=wintypes.HWND; "
        "print('headroom-runtime-console=' + ('allocated' if kernel32.GetConsoleWindow() else 'missing'),flush=True); "
        "signal.signal(signal.SIGBREAK,lambda signum,frame: signal.raise_signal(signal.SIGINT)); "
        f"sys.path.insert(0,{str(overlay_root)!r}); "
        "from headroom.proxy.model_router import ModelRouterConfig; "
        "router=ModelRouterConfig.from_env(os.environ.get('HEADROOM_MODEL_ROUTER_ENABLED'),os.environ.get('HEADROOM_MODEL_ROUTES')); "
        "assert router.enabled is False; "
        "print('headroom-desktop-model-router:disabled',flush=True); "
        "import cache_control_shim as desktop_shim; "
        "shim=desktop_shim.install(); "
        f"assert shim['version']=={expected_version_literal} and shim['source_sha256']=={prefix_sha!r} and shim['shim_sha256']=={shim_sha!r} and shim['active']=='true'; "
        "print('headroom-desktop-cache-shim:version='+shim['version']+';prefix_sha256='+shim['source_sha256']+';shim_sha256='+shim['shim_sha256'],flush=True); "
        "import optimization_control; "
        f"control=optimization_control.install({generation!r},{control_token!r}); "
        f"assert control['module_sha256']=={manifest['optimization_control']['sha256']!r} and control['sources_sha256']=={manifest['optimization_control']['sources_sha256']!r}; "
        "print('headroom-desktop-optimization-control:module_sha256='+control['module_sha256']+';sources_sha256='+control['sources_sha256'],flush=True); "
        "from headroom.cli import main; main()"
    )


def _verify_live_runtime(helper: Any, process: Any, port: int, output: BoundedEngineOutput, manifest: dict[str, Any], shim_sha: str) -> tuple[dict[str, Any], dict[str, Any]]:
    engine = manifest["engine"]
    profile = manifest["profile"]
    listeners = helper.listener_pids(port, helper.windows_api()[2])
    if process.poll() is not None or listeners != {"127.0.0.1": process.pid}:
        raise RuntimeError("owned-listener-check-failed")
    ready_status, ready = helper.request_json(port, "/readyz")
    if (
        ready_status != 200
        or not isinstance(ready, dict)
        or ready.get("ready") is not True
        or ready.get("version") != engine["version"]
    ):
        raise RuntimeError("engine-readiness-check-failed")
    health_status, health = helper.request_json(port, "/health")
    if health_status != 200 or not isinstance(health, dict):
        raise RuntimeError("engine-config-check-failed")
    effective = health.get("config")
    controller = _load_controller(Path(__file__).resolve().parent)
    protected_tools = controller.protected_tools_from_environment()
    if not isinstance(effective, dict) or not set(protected_tools).issubset(set(effective.get("exclude_tools", []))):
        raise RuntimeError("engine-protected-tools-check-failed")
    live_env = effective.get("runtime_env") if isinstance(effective, dict) else None
    if not (
        isinstance(effective, dict)
        and effective.get("mode") == profile["mode"]
        and effective.get("savings_profile") == profile["savings_profile"]
        and effective.get("optimize") is True
        and effective.get("cache") is True
        and effective.get("memory") is profile["memory"]
        and effective.get("learn") is profile["learn"]
        and isinstance(live_env, dict)
        and str(live_env.get("HEADROOM_OUTPUT_SHAPER", "")).lower() in {"0", "false", "no"}
    ):
        raise RuntimeError("engine-config-check-failed")
    stats_status, stats = helper.request_json(port, "/stats")
    telemetry = stats.get("telemetry") if isinstance(stats, dict) else None
    if (
        stats_status != 200
        or not isinstance(telemetry, dict)
        or telemetry.get("enabled") is not True
        or stats.get("anon_telemetry_shipping") is not False
    ):
        raise RuntimeError("engine-telemetry-check-failed")
    ttl_status, ttl_stats = helper.request_json(port, "/v1/retrieve/stats")
    store = ttl_stats.get("store") if isinstance(ttl_stats, dict) else None
    if (
        ttl_status != 200
        or not isinstance(store, dict)
        or store.get("default_ttl_seconds") != profile["ccr_ttl_seconds"]
    ):
        raise RuntimeError("engine-ttl-check-failed")
    if not output.wait_for_start_markers():
        raise RuntimeError("engine-start-markers-unavailable")
    shim_marker = output.shim_marker
    if shim_marker != (engine["version"], engine["prefix_tracker_sha256"], shim_sha):
        raise RuntimeError("cache-shim-provenance-mismatch")
    if output.control_marker != (manifest["optimization_control"]["sha256"],manifest["optimization_control"]["sources_sha256"]):
        raise RuntimeError("optimization-provenance-mismatch")
    config = {
        "mode": effective["mode"],
        "savings_profile": effective["savings_profile"],
        "ttl_seconds": store["default_ttl_seconds"],
        "telemetry_enabled": telemetry["enabled"],
        "beacon_enabled": stats["anon_telemetry_shipping"],
        "memory_enabled": effective["memory"],
        "learning_enabled": effective["learn"],
        "output_shaper_enabled": False,
        "model_router_enabled": False,
        "protected_tools": protected_tools,
    }
    shim = {
        "active": True,
        "version": shim_marker[0],
        "prefix_tracker_sha256": shim_marker[1],
        "shim_sha256": shim_marker[2],
    }
    return config, shim


def _patch_and_run(args: argparse.Namespace) -> int:
    overlay_root = Path(args.manifest).resolve(strict=True).parent
    controller = _load_controller(overlay_root)
    manifest, payload_root, overlay_root = controller.load_manifest(Path(args.manifest))
    helper_path = payload_root / manifest["engine"]["runner"]
    helper = _load_h006_helper(helper_path, manifest["engine"]["runner_sha256"])
    process_state: dict[str, Any] = {
        "request_id": args.request_id,
        "generation": args.runtime_generation,
        "stage": "supervisor-setup",
        "engine": None,
        "output": None,
        "port": None,
        "live_config": None,
        "shim": None,
        "control_token": uuid.uuid4().hex,
    }
    original_start_engine = helper.start_engine
    original_safe_environment = helper.safe_environment
    original_load_manifest = helper.load_manifest
    original_add_runtime_packages = helper.add_runtime_packages
    original_wait_for_ready = helper.wait_for_ready

    def load_manifest():
        process_state["stage"] = "packaged-manifest-check"
        return original_load_manifest()

    def add_runtime_packages(packaged_manifest: dict[str, Any]):
        process_state["stage"] = "package-import-setup"
        return original_add_runtime_packages(packaged_manifest)

    def start_engine(*call_args: Any, **call_kwargs: Any):
        process_state["stage"] = "engine-process-start"
        process, output, port = original_start_engine(*call_args, **call_kwargs)
        process_state["engine"] = process
        process_state["output"] = output
        process_state["port"] = port
        process_state["stage"] = "engine-process-owned"
        return process, output, port

    def safe_environment(*call_args: Any, **call_kwargs: Any):
        process_state["stage"] = "engine-environment-setup"
        env = original_safe_environment(*call_args, **call_kwargs)
        env.update(PROFILE_ENV)
        env["HEADROOM_PROTECT_TOOL_RESULTS"] = ",".join(controller.protected_tools_from_environment())
        env["TIKTOKEN_CACHE_DIR"] = str(controller.verify_tokenizer_cache(payload_root))
        return env

    def wait_for_ready(*call_args: Any, **call_kwargs: Any):
        process_state["stage"] = "engine-readiness-check"
        result = original_wait_for_ready(*call_args, **call_kwargs)
        process_state["stage"] = "engine-ready-observed"
        return result

    def engine_command(port: int) -> list[str]:
        bootstrap = _engine_bootstrap(payload_root, overlay_root, manifest,args.runtime_generation,process_state["control_token"])
        return [
            str(payload_root / manifest["engine"]["python"]),
            "-I",
            "-u",
            "-c",
            bootstrap,
            "proxy",
            "--host",
            "127.0.0.1",
            "--port",
            str(port),
            "--no-learn",
            "--no-memory-tools",
            "--no-memory-context",
        ]

    class DesktopOutput(BoundedEngineOutput):
        NEEDLES = {
            **helper.SafeOutput._MARKERS,
            **BoundedEngineOutput.NEEDLES,
        }

    def send_event(source: dict[str, Any]) -> None:
        event = source.get("event")
        request_id = process_state["request_id"]
        if event == "ready":
            process_state["stage"] = "effective-config-check"
            engine_process = process_state["engine"]
            output = process_state["output"]
            port = process_state["port"]
            if engine_process is None or output is None or port is None:
                raise RuntimeError("engine-state-unavailable")
            config, shim = _verify_live_runtime(
                helper, engine_process, port, output, manifest, manifest["shim"]["sha256"].lower()
            )
            process_state["live_config"] = config
            process_state["shim"] = shim
            data = {
                "helper_pid": os.getpid(),
                "engine_pid": engine_process.pid,
                "listener_port": port,
                "listener_pid": engine_process.pid,
                "engine_version": manifest["engine"]["version"],
                "controller_consoleless": True,
                "manager_hidden_console": source.get("manager_hidden_console") is True,
                "engine_console_allocated": source.get("engine_console_allocated") is True,
                "config": config,
                "shim": shim,
                "optimization_control": {"protocol_version":1,"control_token":process_state["control_token"],
                                         "module_sha256":manifest["optimization_control"]["sha256"],
                                         "sources_sha256":manifest["optimization_control"]["sources_sha256"]},
            }
            sys.stdout.buffer.write(controller.encode_frame(request_id, args.runtime_generation, "ready", data))
            sys.stdout.buffer.flush()
            return
        if event == "stopped":
            shutdown = _shutdown_kind(source, process_state["output"])
            data = {
                "shutdown": shutdown,
                "engine_exit_code": source.get("engine_exit_code") if type(source.get("engine_exit_code")) is int else None,
                "owned_port_closed": source.get("owned_port_closed") is True,
                "hidden_console_verified": source.get("hidden_console_verified") is True,
                "lifespan_complete": source.get("lifespan_complete") is True,
                "proxy_summary": source.get("proxy_summary") is True,
                "force_fallback": source.get("force_fallback") is True,
            }
            sys.stdout.buffer.write(controller.encode_frame(request_id, args.runtime_generation, "stopped", data))
            sys.stdout.buffer.flush()
            return
        if event == "error":
            codes = {
                "windows-only-runtime": "windows_only",
                "hidden-controller-console-required": "controller_console_required",
                "runtime-start-or-control-failed": {
                    "engine-readiness-check": "engine_readiness_failed",
                    "effective-config-check": "effective_config_failed",
                }.get(process_state["stage"], "engine_start_failed"),
                "control-message-too-large": "frame_too_large",
                "invalid-control-command": "invalid_shape",
            }
            code = codes.get(str(source.get("code")), "engine_start_failed")
            sys.stdout.buffer.write(controller.encode_frame(request_id, args.runtime_generation, "error", {"error_code": code}))
            sys.stdout.buffer.flush()

    helper.load_manifest = load_manifest
    helper.add_runtime_packages = add_runtime_packages
    helper.start_engine = start_engine
    helper.safe_environment = safe_environment
    helper.wait_for_ready = wait_for_ready
    helper.engine_command = engine_command
    helper.SafeOutput = DesktopOutput
    helper.send_event = send_event
    original_stdin = sys.stdin
    sys.stdin = VersionedControlInput(original_stdin, process_state, controller)
    try:
        return helper.run(
            argparse.Namespace(state_root=args.state_root, workspace=args.workspace, config=args.config)
        )
    finally:
        sys.stdin = original_stdin


def main() -> int:
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--state-root", required=True)
    parser.add_argument("--workspace", required=True)
    parser.add_argument("--config", required=True)
    parser.add_argument("--request-id", required=True)
    parser.add_argument("--runtime-generation", required=True)
    args = parser.parse_args()
    try:
        return _patch_and_run(args)
    except Exception as exc:
        # The process protocol exposes fixed codes only; diagnostics never include raw paths or logs.
        try:
            controller = _load_controller(Path(args.manifest).resolve(strict=True).parent)
            if controller._valid_token(args.request_id) and controller._valid_token(args.runtime_generation):
                code = exc.code if isinstance(exc, controller.ProtocolError) else "engine_start_failed"
                controller._emit_error(args.request_id, args.runtime_generation, code)
        except Exception:
            pass
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
