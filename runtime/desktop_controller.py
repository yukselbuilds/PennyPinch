"""Consoleless JSONL controller for the pinned Headroom desktop runtime."""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import os
import queue
import subprocess
import sys
import threading
import uuid
from ctypes import wintypes
from pathlib import Path
from typing import Any


PROTOCOL_VERSION = 1
MAX_FRAME_BYTES = 4096
MAX_HELPER_FRAME_BYTES = 8192
HELPER_STARTUP_TIMEOUT_SECONDS = 90.0
HELPER_STOP_TIMEOUT_SECONDS = 30.0
HELPER_EXIT_TIMEOUT_SECONDS = 30.0
HELPER_TERMINATE_TIMEOUT_SECONDS = 5.0
HELPER_READER_JOIN_TIMEOUT_SECONDS = 2.0
# Public vocabularies and checksums from the pinned tiktoken_ext/openai_public.py.
TOKENIZER_SOURCES = (
    ("https://openaipublic.blob.core.windows.net/gpt-2/encodings/main/vocab.bpe", "1ce1664773c50f3e0cc8842619a93edc4624525b728b188a9e0be33b7726adc5"),
    ("https://openaipublic.blob.core.windows.net/gpt-2/encodings/main/encoder.json", "196139668be63f3b5d6574427317ae82f612a97c5d1cdaf36ed2256dbf636783"),
    ("https://openaipublic.blob.core.windows.net/encodings/r50k_base.tiktoken", "306cd27f03c1a714eca7108e03d66b7dc042abe8c258b44c199a7ed9838dd930"),
    ("https://openaipublic.blob.core.windows.net/encodings/p50k_base.tiktoken", "94b5ca7dff4d00767bc256fdd1b27e5b17361d7b8a5f968547f9f23eb70d2069"),
    ("https://openaipublic.blob.core.windows.net/encodings/cl100k_base.tiktoken", "223921b76ee99bde995b7ff738513eef100fb51d18c93597a113bcffe865b2a7"),
    ("https://openaipublic.blob.core.windows.net/encodings/o200k_base.tiktoken", "446a9538cb6c348e3516120d7c08b09f57c36495e2acfffe59a5bf8b0cfb1a2d"),
)
PINNED_EXPECTED = {
    "engine_name": "headroom-ai",
    "engine_version": "0.41.0",
    "runtime_manifest_sha256": "8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908",
    "runner_sha256": "bf887b6f783f4385debc84836353340f39e3773433782305376a3247be0f4ac1",
    "python_version": "3.13.13",
    "python_sha256": "282c2635fbd49b4aeefda4a965d84a745508c87e6e5eb95947efd0b5a1c1fb07",
    "site_packages": "Lib/site-packages",
    "prefix_tracker_sha256": "f1af4ee279f879efc48ddfebbff125a088893bb64ee960ca2dbd4ad1cdb628ee",
    "shim_sha256": "ebc80ca2958b178ed488fb953f1a09cf2dd803b23352dcc3f8f0a644414e30c1",
    "optimization_sha256": "972c0e60fb791fb9ccd27167e561bf31cef735da88e3423b0e607b2205c8a283",
    "optimization_sources_sha256": "f96b93d2c574a2155012c7e587aaed2a2c86f6fe288cd88f847e81e9a27b4bfa",
}
ERROR_CODES = frozenset(
    {
        "frame_too_large",
        "invalid_json",
        "invalid_shape",
        "unsupported_version",
        "invalid_request_id",
        "unknown_event",
        "generation_mismatch",
        "invalid_state",
        "windows_only",
        "controller_console_required",
        "manifest_invalid",
        "runtime_pin_mismatch",
        "helper_start_failed",
        "helper_protocol_invalid",
        "helper_timeout",
        "engine_start_failed",
        "engine_readiness_failed",
        "effective_config_failed",
        "shutdown_incomplete",
        "cleanup_failed",
    }
)
_READY_KEYS = frozenset(
    {
        "helper_pid",
        "engine_pid",
        "listener_port",
        "listener_pid",
        "engine_version",
        "controller_consoleless",
        "manager_hidden_console",
        "engine_console_allocated",
        "config",
        "shim",
        "optimization_control",
    }
)
_CONFIG_KEYS = frozenset(
    {
        "mode",
        "savings_profile",
        "ttl_seconds",
        "telemetry_enabled",
        "beacon_enabled",
        "memory_enabled",
        "learning_enabled",
        "output_shaper_enabled",
        "model_router_enabled",
        "protected_tools",
    }
)
_SHIM_KEYS = frozenset({"active", "version", "prefix_tracker_sha256", "shim_sha256"})
_STOPPED_KEYS = frozenset(
    {
        "shutdown",
        "engine_exit_code",
        "owned_port_closed",
        "hidden_console_verified",
        "lifespan_complete",
        "proxy_summary",
        "force_fallback",
    }
)
PINNED_PROFILE = {
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


class ProtocolError(ValueError):
    def __init__(self, code: str) -> None:
        super().__init__(code)
        self.code = code if code in ERROR_CODES else "invalid_shape"


def _valid_token(value: object) -> bool:
    return (
        isinstance(value, str)
        and 1 <= len(value) <= 128
        and all("!" <= char <= "~" for char in value)
    )


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ProtocolError("invalid_json")
        result[key] = value
    return result


def _parse_line(raw: bytes, maximum: int) -> dict[str, Any]:
    if len(raw) > maximum:
        raise ProtocolError("frame_too_large")
    if not raw.endswith(b"\n"):
        raise ProtocolError("invalid_shape")
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=_unique_object)
    except ProtocolError:
        raise
    except (UnicodeDecodeError, json.JSONDecodeError):
        raise ProtocolError("invalid_json") from None
    if not isinstance(value, dict):
        raise ProtocolError("invalid_shape")
    return value


def decode_request(raw: bytes, expected_generation: str | None = None) -> dict[str, Any]:
    value = _parse_line(raw, MAX_FRAME_BYTES)
    event = value.get("event")
    expected_keys = (
        {"protocol_version", "request_id", "event"}
        if event == "start"
        else {"protocol_version", "request_id", "event", "runtime_generation"}
        if event == "stop"
        else set()
    )
    if not expected_keys or set(value) != expected_keys:
        raise ProtocolError("unknown_event" if event not in {"start", "stop"} else "invalid_shape")
    if type(value.get("protocol_version")) is not int or value["protocol_version"] != PROTOCOL_VERSION:
        raise ProtocolError("unsupported_version")
    if not _valid_token(value.get("request_id")):
        raise ProtocolError("invalid_request_id")
    if event == "stop":
        generation = value.get("runtime_generation")
        if not _valid_token(generation):
            raise ProtocolError("invalid_shape")
        if expected_generation is not None and generation != expected_generation:
            raise ProtocolError("generation_mismatch")
    return value


def _validate_event_data(event: str, data: dict[str, Any]) -> None:
    if event == "ready":
        if set(data) != _READY_KEYS:
            raise ProtocolError("invalid_shape")
        if any(type(data.get(key)) is not int or data[key] <= 0 for key in ("helper_pid", "engine_pid", "listener_port", "listener_pid")):
            raise ProtocolError("invalid_shape")
        if not isinstance(data.get("engine_version"), str):
            raise ProtocolError("invalid_shape")
        if any(type(data.get(key)) is not bool for key in ("controller_consoleless", "manager_hidden_console", "engine_console_allocated")):
            raise ProtocolError("invalid_shape")
        config = data.get("config")
        shim = data.get("shim")
        control = data.get("optimization_control")
        if not isinstance(control,dict) or set(control)!={"protocol_version","control_token","module_sha256","sources_sha256"}:
            raise ProtocolError("invalid_shape")
        token=control.get("control_token")
        if type(control.get("protocol_version")) is not int or control["protocol_version"]!=1 or not isinstance(token,str) or len(token)!=32 or any(c not in "0123456789abcdef" for c in token) or token=="0"*32:
            raise ProtocolError("invalid_shape")
        if any(not isinstance(control.get(k),str) or len(control[k])!=64 or any(c not in "0123456789abcdef" for c in control[k]) for k in ("module_sha256","sources_sha256")):
            raise ProtocolError("invalid_shape")
        if not isinstance(config, dict) or set(config) != _CONFIG_KEYS:
            raise ProtocolError("invalid_shape")
        validate_protected_tools(config.get("protected_tools"))
        if not isinstance(shim, dict) or set(shim) != _SHIM_KEYS:
            raise ProtocolError("invalid_shape")
        if type(config.get("ttl_seconds")) is not int or any(
            type(config.get(key)) is not bool
            for key in (
                "telemetry_enabled",
                "beacon_enabled",
                "memory_enabled",
                "learning_enabled",
                "output_shaper_enabled",
                "model_router_enabled",
            )
        ):
            raise ProtocolError("invalid_shape")
        if type(shim.get("active")) is not bool:
            raise ProtocolError("invalid_shape")
        for key in ("version", "prefix_tracker_sha256", "shim_sha256"):
            if not isinstance(shim.get(key), str):
                raise ProtocolError("invalid_shape")
    elif event == "stopped":
        if set(data) != _STOPPED_KEYS:
            raise ProtocolError("invalid_shape")
        if data.get("shutdown") not in {"graceful", "incomplete", "control-failed", "already-exited"}:
            raise ProtocolError("invalid_shape")
        if data.get("engine_exit_code") is not None and type(data["engine_exit_code"]) is not int:
            raise ProtocolError("invalid_shape")
        if any(type(data.get(key)) is not bool for key in _STOPPED_KEYS - {"shutdown", "engine_exit_code"}):
            raise ProtocolError("invalid_shape")
    elif event == "error":
        if set(data) != {"error_code"} or data.get("error_code") not in ERROR_CODES:
            raise ProtocolError("invalid_shape")
    else:
        raise ProtocolError("unknown_event")


def encode_frame(
    request_id: str | None, runtime_generation: str | None, event: str, data: dict[str, Any]
) -> bytes:
    if request_id is None and event != "error":
        raise ProtocolError("invalid_request_id")
    if runtime_generation is None and event in {"ready", "stopped"}:
        raise ProtocolError("invalid_shape")
    if request_id is not None and not _valid_token(request_id):
        raise ProtocolError("invalid_request_id")
    if runtime_generation is not None and not _valid_token(runtime_generation):
        raise ProtocolError("invalid_shape")
    _validate_event_data(event, data)
    frame = {
        "protocol_version": PROTOCOL_VERSION,
        "request_id": request_id,
        "runtime_generation": runtime_generation,
        "event": event,
        **data,
    }
    encoded = (json.dumps(frame, ensure_ascii=True, separators=(",", ":")) + "\n").encode("ascii")
    if len(encoded) > MAX_HELPER_FRAME_BYTES:
        raise ProtocolError("frame_too_large")
    return encoded


def decode_frame(
    raw: bytes, expected_generation: str | None = None, expected_request_id: str | None = None
) -> dict[str, Any]:
    value = _parse_line(raw, MAX_HELPER_FRAME_BYTES)
    if value.get("protocol_version") != PROTOCOL_VERSION or type(value.get("protocol_version")) is not int:
        raise ProtocolError("unsupported_version")
    event = value.get("event")
    if event not in {"ready", "stopped", "error"}:
        raise ProtocolError("unknown_event")
    data_keys = {"ready": _READY_KEYS, "stopped": _STOPPED_KEYS, "error": {"error_code"}}[event]
    if set(value) != {"protocol_version", "request_id", "runtime_generation", "event", *data_keys}:
        raise ProtocolError("invalid_shape")
    request_id = value.get("request_id")
    generation = value.get("runtime_generation")
    if request_id is None and event != "error":
        raise ProtocolError("invalid_request_id")
    if generation is None and event in {"ready", "stopped"}:
        raise ProtocolError("invalid_shape")
    if request_id is not None and not _valid_token(request_id):
        raise ProtocolError("invalid_request_id")
    if generation is not None and not _valid_token(generation):
        raise ProtocolError("invalid_shape")
    if expected_generation is not None and generation != expected_generation:
        raise ProtocolError("generation_mismatch")
    if expected_request_id is not None and request_id != expected_request_id:
        raise ProtocolError("invalid_shape")
    data = {key: value[key] for key in data_keys}
    _validate_event_data(event, data)
    return value


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _under(root: Path, relative: str) -> Path:
    candidate = Path(relative)
    if candidate.is_absolute():
        raise ProtocolError("runtime_pin_mismatch")
    resolved_root = root.resolve(strict=True)
    resolved = (resolved_root / candidate).resolve(strict=True)
    if resolved != resolved_root and resolved_root not in resolved.parents:
        raise ProtocolError("runtime_pin_mismatch")
    if not resolved.is_file():
        raise ProtocolError("runtime_pin_mismatch")
    return resolved


def verify_tokenizer_cache(payload_root: Path) -> Path:
    root = payload_root.resolve(strict=True)
    cache = (root / "tiktoken-cache").resolve(strict=True)
    if root not in cache.parents or not cache.is_dir():
        raise ProtocolError("runtime_pin_mismatch")
    for url, expected in TOKENIZER_SOURCES:
        key = hashlib.sha1(url.encode("utf-8")).hexdigest()
        if _sha256_file(_under(cache, key)) != expected:
            raise ProtocolError("runtime_pin_mismatch")
    return cache


def verify_runtime_files(
    manifest: dict[str, Any],
    payload_root: Path,
    overlay_root: Path | None = None,
    expected_pins: dict[str, str] | None = None,
) -> dict[str, str]:
    if manifest.get("schema_version") != 1 or manifest.get("protocol_version") != PROTOCOL_VERSION:
        raise ProtocolError("manifest_invalid")
    validate_runtime_profile(manifest.get("profile"))
    engine = manifest.get("engine")
    shim = manifest.get("shim")
    control = manifest.get("optimization_control")
    if not isinstance(engine, dict) or not isinstance(shim, dict) or not isinstance(control,dict) or control.get("file")!="optimization_control.py" or control.get("sources")!="optimization-sources.json":
        raise ProtocolError("manifest_invalid")
    pins = expected_pins or PINNED_EXPECTED
    manifest_pins = {
        "engine_name": engine.get("name"),
        "engine_version": engine.get("version"),
        "runtime_manifest_sha256": engine.get("runtime_manifest_sha256"),
        "runner_sha256": engine.get("runner_sha256"),
        "python_version": engine.get("python_version"),
        "python_sha256": engine.get("python_sha256"),
        "site_packages": engine.get("site_packages"),
        "prefix_tracker_sha256": engine.get("prefix_tracker_sha256"),
        "shim_sha256": shim.get("sha256"),
        "optimization_sha256": control.get("sha256"),
        "optimization_sources_sha256": control.get("sources_sha256"),
    }
    if manifest_pins != pins:
        raise ProtocolError("runtime_pin_mismatch")
    try:
        paths = {
            "runtime_manifest": _under(payload_root, engine["runtime_manifest"]),
            "runner": _under(payload_root, engine["runner"]),
            "python": _under(payload_root, engine["python"]),
            "prefix_tracker": _under(payload_root, engine["prefix_tracker"]),
            "shim": _under(overlay_root or payload_root, shim["file"]),
            "optimization_control": _under(overlay_root or payload_root,control["file"]),
            "optimization_sources": _under(overlay_root or payload_root,control["sources"]),
        }
        expected = {
            "runtime_manifest": engine["runtime_manifest_sha256"],
            "runner": engine["runner_sha256"],
            "python": engine["python_sha256"],
            "prefix_tracker": engine["prefix_tracker_sha256"],
            "shim": shim["sha256"],
            "optimization_control": control["sha256"],
            "optimization_sources": control["sources_sha256"],
        }
    except (KeyError, TypeError):
        raise ProtocolError("manifest_invalid") from None
    for key, path in paths.items():
        if not isinstance(expected[key], str) or _sha256_file(path) != expected[key].lower():
            raise ProtocolError("runtime_pin_mismatch")
    try:
        packaged_manifest = json.loads(paths["runtime_manifest"].read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError, UnicodeDecodeError):
        raise ProtocolError("manifest_invalid") from None
    if (
        not isinstance(packaged_manifest, dict)
        or packaged_manifest.get("schema_version") != 1
        or packaged_manifest.get("engine", {}).get("name") != engine.get("name")
        or packaged_manifest.get("engine", {}).get("version") != engine.get("version")
        or packaged_manifest.get("python", {}).get("version") != engine.get("python_version")
        or packaged_manifest.get("python", {}).get("sha256") != engine.get("python_sha256")
        or packaged_manifest.get("runtime", {}).get("runner") != engine.get("runner")
        or packaged_manifest.get("runtime", {}).get("site_packages") != engine.get("site_packages")
    ):
        raise ProtocolError("runtime_pin_mismatch")
    return {key: _sha256_file(path) for key, path in paths.items()}


def validate_runtime_profile(profile: Any) -> None:
    if not isinstance(profile, dict) or profile != PINNED_PROFILE:
        raise ProtocolError("runtime_pin_mismatch")


def load_manifest(path: Path) -> tuple[dict[str, Any], Path, Path]:
    try:
        manifest = json.loads(path.read_text(encoding="utf-8"))
        engine = manifest["engine"]
        payload_root = (path.parent / engine["payload_root"]).resolve(strict=True)
        overlay_root = path.parent.resolve(strict=True)
        verify_runtime_files(manifest, payload_root, overlay_root)
    except ProtocolError:
        raise
    except (OSError, KeyError, TypeError, json.JSONDecodeError, UnicodeDecodeError):
        raise ProtocolError("manifest_invalid") from None
    return manifest, payload_root, overlay_root


def _write(frame: bytes) -> None:
    sys.stdout.buffer.write(frame)
    sys.stdout.buffer.flush()


def _emit_error(request_id: str | None, generation: str | None, code: str) -> None:
    if code not in ERROR_CODES:
        code = "invalid_shape"
    _write(encode_frame(request_id, generation, "error", {"error_code": code}))


def _request_id_from_line(raw: bytes) -> str | None:
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=_unique_object)
    except (ProtocolError, UnicodeDecodeError, json.JSONDecodeError):
        return None
    return value.get("request_id") if isinstance(value, dict) and _valid_token(value.get("request_id")) else None


def _next_request(expected_generation: str | None = None) -> tuple[dict[str, Any] | None, bytes | None]:
    raw = sys.stdin.buffer.readline(MAX_FRAME_BYTES + 2)
    if not raw:
        return None, None
    try:
        return decode_request(raw, expected_generation), raw
    except ProtocolError as exc:
        _emit_error(_request_id_from_line(raw), expected_generation, exc.code)
        return None, raw


def _windows_consoleless() -> bool:
    if os.name != "nt":
        return False
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.GetConsoleWindow.argtypes = []
    kernel32.GetConsoleWindow.restype = wintypes.HWND
    return not bool(kernel32.GetConsoleWindow())


def validate_protected_tools(value: Any) -> list[str]:
    if (not isinstance(value, list) or len(value) > 32
        or any(not isinstance(name, str) or not 1 <= len(name) <= 96
               or any(not (c.isascii() and (c.isalnum() or c in "_-.:/")) for c in name) for name in value)
        or value != sorted(set(value))):
        raise ProtocolError("invalid_shape")
    return value


def protected_tools_from_environment() -> list[str]:
    try:
        return validate_protected_tools(json.loads(os.environ.get("HEADROOM_DESKTOP_PROTECTED_TOOLS", "[]")))
    except (ValueError, TypeError):
        raise ProtocolError("invalid_shape") from None


def _safe_child_environment(state_root: Path) -> dict[str, str]:
    windows_root = os.environ.get("SystemRoot", r"C:\Windows")
    system_drive = os.environ.get("SystemDrive", "C:")
    home = state_root / "controller-home"
    temp = state_root / "controller-temp"
    appdata = home / "AppData" / "Roaming"
    localappdata = home / "AppData" / "Local"
    for path in (home, temp, appdata, localappdata):
        path.mkdir(parents=True, exist_ok=True)
    return {
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
        "APPDATA": str(appdata),
        "LOCALAPPDATA": str(localappdata),
        "HEADROOM_DESKTOP_PROTECTED_TOOLS": json.dumps(protected_tools_from_environment()),
    }


class HelperFrameReader:
    """Read bounded helper frames off-thread so a stalled pipe cannot hang control."""

    def __init__(self, stream: Any) -> None:
        self._stream = stream
        self._frames: queue.Queue[bytes | None] = queue.Queue(maxsize=1)
        self._stopping = threading.Event()
        self._thread = threading.Thread(target=self._read_frames, name="headroom-helper-stdout", daemon=True)
        self._thread.start()

    def _publish(self, frame: bytes | None) -> bool:
        while not self._stopping.is_set():
            try:
                self._frames.put(frame, timeout=0.05)
                return True
            except queue.Full:
                continue
        return False

    def _read_frames(self) -> None:
        try:
            while not self._stopping.is_set():
                frame = self._stream.readline(MAX_HELPER_FRAME_BYTES + 2)
                if not self._publish(frame):
                    return
                if not frame:
                    return
        except (OSError, ValueError):
            self._publish(None)

    def read_event(
        self,
        generation: str,
        request_id: str,
        timeout: float,
    ) -> dict[str, Any]:
        try:
            raw = self._frames.get(timeout=timeout)
        except queue.Empty:
            raise ProtocolError("helper_timeout") from None
        if not raw:
            raise ProtocolError("helper_protocol_invalid")
        try:
            return decode_frame(raw, expected_generation=generation, expected_request_id=request_id)
        except ProtocolError:
            raise ProtocolError("helper_protocol_invalid") from None

    def finish_after_process_exit(self, timeout: float = HELPER_READER_JOIN_TIMEOUT_SECONDS) -> bool:
        """Join after the owned child exits; never close a pipe under its reader thread."""
        self._stopping.set()
        while True:
            try:
                self._frames.get_nowait()
            except queue.Empty:
                break
        self._thread.join(timeout=timeout)
        if self._thread.is_alive():
            return False
        self._stream.close()
        return True


def _read_helper_event(
    reader: HelperFrameReader,
    generation: str,
    request_id: str,
    timeout: float,
) -> dict[str, Any]:
    return reader.read_event(generation, request_id, timeout)


def _terminate_owned_helper(process: subprocess.Popen[bytes]) -> bool:
    if process.poll() is None:
        try:
            process.terminate()
            process.wait(timeout=HELPER_TERMINATE_TIMEOUT_SECONDS)
        except subprocess.TimeoutExpired:
            try:
                process.kill()
                process.wait(timeout=HELPER_TERMINATE_TIMEOUT_SECONDS)
            except (OSError, subprocess.SubprocessError):
                return process.poll() is not None
        except (OSError, subprocess.SubprocessError):
            return process.poll() is not None
    return process.poll() is not None


def _start_helper(
    manifest_path: Path,
    manifest: dict[str, Any],
    payload_root: Path,
    overlay_root: Path,
    state_root: Path,
    workspace: Path,
    config: Path,
    request_id: str,
    generation: str,
) -> subprocess.Popen[bytes]:
    engine = manifest["engine"]
    executable = payload_root / engine["python"]
    helper_script = overlay_root / "desktop_helper.py"
    command = [
        str(executable),
        "-I",
        "-u",
        str(helper_script),
        "--manifest",
        str(manifest_path),
        "--state-root",
        str(state_root),
        "--workspace",
        str(workspace),
        "--config",
        str(config),
        "--request-id",
        request_id,
        "--runtime-generation",
        generation,
    ]
    startup = subprocess.STARTUPINFO()
    startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
    startup.wShowWindow = 0
    try:
        return subprocess.Popen(
            command,
            cwd=str(payload_root),
            env=_safe_child_environment(state_root),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            startupinfo=startup,
            creationflags=subprocess.CREATE_NEW_CONSOLE | subprocess.CREATE_NEW_PROCESS_GROUP,
            close_fds=True,
        )
    except (OSError, ValueError):
        raise ProtocolError("helper_start_failed") from None


def _send_helper_stop(process: subprocess.Popen[bytes], request_id: str, generation: str) -> None:
    if process.stdin is None:
        raise ProtocolError("helper_protocol_invalid")
    control = {
        "protocol_version": PROTOCOL_VERSION,
        "request_id": request_id,
        "runtime_generation": generation,
        "event": "stop",
    }
    raw = (json.dumps(control, separators=(",", ":")) + "\n").encode("ascii")
    process.stdin.write(raw)
    process.stdin.flush()


def serve(args: argparse.Namespace) -> int:
    if os.name != "nt":
        _emit_error(None, None, "windows_only")
        return 2
    if not _windows_consoleless():
        _emit_error(None, None, "controller_console_required")
        return 2
    try:
        manifest_path = Path(args.manifest).resolve(strict=True)
        manifest, payload_root, overlay_root = load_manifest(manifest_path)
        state_root, workspace, config = (Path(value).resolve() for value in (args.state_root, args.workspace, args.config))
        if not all(Path(value).is_absolute() for value in (args.state_root, args.workspace, args.config)):
            raise ProtocolError("invalid_shape")
        if state_root == payload_root or payload_root in state_root.parents or config == payload_root or payload_root in config.parents:
            raise ProtocolError("runtime_pin_mismatch")
    except ProtocolError as exc:
        _emit_error(None, None, exc.code)
        return 2
    except (OSError, ValueError):
        _emit_error(None, None, "manifest_invalid")
        return 2

    request, raw = _next_request()
    if request is None:
        return 2 if raw is None else 2
    if request["event"] != "start":
        _emit_error(request["request_id"], None, "invalid_state")
        return 2
    generation = uuid.uuid4().hex
    helper: subprocess.Popen[bytes] | None = None
    helper_reader: HelperFrameReader | None = None
    graceful = False
    stop_sent = False
    force_helper_cleanup = False
    stop_request_id: str | None = None
    try:
        helper = _start_helper(
            manifest_path,
            manifest,
            payload_root,
            overlay_root,
            state_root,
            workspace,
            config,
            request["request_id"],
            generation,
        )
        if helper.stdout is None:
            raise ProtocolError("helper_protocol_invalid")
        helper_reader = HelperFrameReader(helper.stdout)
        ready = _read_helper_event(
            helper_reader,
            generation,
            request["request_id"],
            HELPER_STARTUP_TIMEOUT_SECONDS,
        )
        if ready["event"] == "error":
            _emit_error(request["request_id"], generation, ready["error_code"])
            return 3
        if ready["event"] != "ready":
            _emit_error(request["request_id"], generation, "engine_start_failed")
            return 3
        if (
            ready.get("helper_pid") != helper.pid
            or ready.get("engine_pid") == helper.pid
            or ready.get("listener_pid") != ready.get("engine_pid")
            or ready.get("engine_version") != manifest["engine"]["version"]
            or ready.get("manager_hidden_console") is not True
            or ready.get("engine_console_allocated") is not True
            or ready.get("controller_consoleless") is not True
            or ready.get("shim", {}).get("active") is not True
            or ready.get("optimization_control",{}).get("module_sha256")!=manifest["optimization_control"]["sha256"]
            or ready.get("optimization_control",{}).get("sources_sha256")!=manifest["optimization_control"]["sources_sha256"]
        ):
            _emit_error(request["request_id"], generation, "engine_start_failed")
            return 3
        ready_data = {key: ready[key] for key in _READY_KEYS}
        ready_data["helper_pid"] = helper.pid
        ready_data["controller_consoleless"] = True
        _write(encode_frame(request["request_id"], generation, "ready", ready_data))

        while True:
            next_request, raw = _next_request(generation)
            if raw is None:
                stop_request_id = uuid.uuid4().hex
                break
            if next_request is None:
                if len(raw) > MAX_FRAME_BYTES or not raw.endswith(b"\n"):
                    stop_request_id = uuid.uuid4().hex
                    break
                continue
            if next_request["event"] != "stop":
                _emit_error(next_request["request_id"], generation, "invalid_state")
                continue
            stop_request_id = next_request["request_id"]
            break

        assert helper is not None and stop_request_id is not None
        _send_helper_stop(helper, stop_request_id, generation)
        stop_sent = True
        stopped = _read_helper_event(
            helper_reader,
            generation,
            stop_request_id,
            HELPER_STOP_TIMEOUT_SECONDS,
        )
        if stopped["event"] != "stopped":
            _emit_error(stop_request_id, generation, "shutdown_incomplete")
            return 4
        stopped_data = {key: stopped[key] for key in _STOPPED_KEYS}
        _write(encode_frame(stop_request_id, generation, "stopped", stopped_data))
        try:
            helper_exit = helper.wait(timeout=HELPER_EXIT_TIMEOUT_SECONDS)
        except subprocess.TimeoutExpired:
            force_helper_cleanup = True
            _emit_error(stop_request_id, generation, "shutdown_incomplete")
            return 4
        graceful = (
            stopped_data["shutdown"] == "graceful"
            and stopped_data["engine_exit_code"] == 0
            and stopped_data["owned_port_closed"] is True
            and helper_exit == 0
        )
        return 0 if graceful else 4
    except ProtocolError as exc:
        if exc.code == "helper_timeout":
            force_helper_cleanup = True
        _emit_error(stop_request_id or request["request_id"], generation, exc.code)
        return 3
    except (OSError, ValueError, subprocess.SubprocessError):
        _emit_error(stop_request_id or request["request_id"], generation, "helper_protocol_invalid")
        return 3
    finally:
        if helper is not None:
            if helper.poll() is None and force_helper_cleanup:
                if not _terminate_owned_helper(helper):
                    _emit_error(stop_request_id or request["request_id"], generation, "cleanup_failed")
            elif helper.poll() is None:
                try:
                    if not stop_sent and helper.stdin is not None and not helper.stdin.closed:
                        fallback_id = stop_request_id or uuid.uuid4().hex
                        _send_helper_stop(helper, fallback_id, generation)
                        stop_sent = True
                    helper.wait(timeout=HELPER_EXIT_TIMEOUT_SECONDS)
                except (OSError, ProtocolError, subprocess.SubprocessError):
                    if not _terminate_owned_helper(helper):
                        _emit_error(stop_request_id or request["request_id"], generation, "cleanup_failed")
            if helper.stdin is not None and not helper.stdin.closed:
                helper.stdin.close()
            if helper_reader is not None:
                if not helper_reader.finish_after_process_exit():
                    _emit_error(stop_request_id or request["request_id"], generation, "cleanup_failed")
            elif helper.stdout is not None and not helper.stdout.closed:
                helper.stdout.close()


def main() -> int:
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--manifest", default=str(Path(__file__).with_name("manifest.json")))
    parser.add_argument("--state-root", required=True)
    parser.add_argument("--workspace", required=True)
    parser.add_argument("--config", required=True)
    return serve(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
