"""Application-owned, in-memory pause at request boundaries for the pinned engine."""
from __future__ import annotations

import contextvars
import copy
import hashlib
import hmac
import json
import threading
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any

CONTROL_PATH = "/__headroom_desktop/control"
MAX_PAYLOAD = 2 * 1024 * 1024
MAX_TRACKERS = 64
MAX_REPLAY_BYTES = 64 * 1024 * 1024
_request = contextvars.ContextVar("headroom_desktop_request", default=None)


class ReplayUnavailable(BaseException):
    """Caught only by our outer ASGI boundary, never a vendor fail-open handler."""


@dataclass
class RequestState:
    gate: Any
    enabled: bool
    kind: str
    bypass: bool
    scope_key: str
    previous: tuple | None = None
    replay_failed: bool = False


def _encoded(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")


def _digest(value: Any) -> str:
    return hashlib.sha256(_encoded(value)).hexdigest()


def _preserve(current: list, candidate: list, previous: tuple | None, prefix: Any) -> list:
    if not previous:
        return candidate
    original, forwarded = previous
    if not original or not forwarded:
        return candidate
    relation = prefix.classify_history_relation(current, original)
    if relation.kind in (prefix.RELATION_EXACT, prefix.RELATION_MESSAGE_APPEND):
        if len(original) != len(forwarded) or len(current) != len(candidate):
            raise ReplayUnavailable()
        # This is a locally recorded prefix, not provider-confirmed cache usage.
        return copy.deepcopy(forwarded) + candidate[len(original):]
    return prefix.overlay_cached_prefix(candidate, current, original, forwarded)


class Gate:
    def __init__(self, generation: str, secret: str, prefix: Any):
        self.generation, self._secret, self.prefix = generation, secret, prefix
        self.enabled, self.revision, self.history_safe = True, 0, True
        self.active_http = 0
        self.proxy_store = None
        self.lock = threading.RLock()
        self.replay = prefix.SessionTrackerStore(prefix.PrefixFreezeConfig(
            session_ttl_seconds=7200, max_lineages_per_session=MAX_TRACKERS + 1))
        self.sizes: dict[int, int] = {}

    def state(self) -> dict:
        with self.lock:
            return {"enabled": self.enabled, "revision": self.revision,
                    "can_pause": self.history_safe, "active_http_requests": self.active_http}

    def fail_replay(self, state: RequestState) -> None:
        with self.lock:
            self.history_safe = False
        if not state.enabled:
            state.replay_failed = True
            raise ReplayUnavailable()

    def prepare(self, payload: dict, state: RequestState):
        key = "input" if isinstance(payload.get("input"), list) else "messages"
        items = payload.get(key)
        if not isinstance(items, list):
            self.fail_replay(state)
            return key, None, None
        if len(_encoded(payload)) > MAX_PAYLOAD:
            self.fail_replay(state)
            return key, None, None
        affinity = _digest({k: payload.get(k) for k in ("model", "instructions", "tools", "tool_choice", "reasoning", "text", "previous_response_id", "conversation")})
        with self.lock:
            if not self.history_safe or len(self.replay._trackers) >= MAX_TRACKERS:
                self.fail_replay(state)
                return key, None, None
            before = set(self.replay._trackers)
            tracker = self.replay.resolve_tracker(state.scope_key, "desktop_responses", messages=items,
                                                   cache_affinity=affinity, cache_ttl_seconds=7200)
            if not before.issubset(self.replay._trackers):
                self.fail_replay(state)
                return key, None, None
            previous = (tracker.get_last_original_messages(), tracker.get_last_forwarded_messages())
            return key, tracker, previous

    def record(self, tracker: Any, original: list, forwarded: list, tools: Any, state: RequestState) -> None:
        # Account conservatively for the vendor's three retained lineage states.
        size = 4 * (len(_encoded(original)) + len(_encoded(forwarded)) + len(_encoded(tools)))
        with self.lock:
            total = sum(self.sizes.values()) - self.sizes.get(id(tracker), 0) + size
            if total > MAX_REPLAY_BYTES:
                self.fail_replay(state)
                return
            self.sizes[id(tracker)] = size
            tracker._desktop_forwarded_tools = copy.deepcopy(tools)
            # Private replay bookkeeping only: no provider cache count is asserted or exported.
            tracker.update_from_response(0, 0, forwarded, original_messages=original)


class ScopedConfig:
    def __init__(self, base: Any, gate: Gate):
        object.__setattr__(self, "_base", base)
        object.__setattr__(self, "_gate", gate)

    def __getattr__(self, name: str):
        value = getattr(self._base, name)
        if name not in {"optimize", "image_optimize"}:
            return value
        state = _request.get()
        enabled = state.enabled if state is not None and state.gate is self._gate else self._gate.enabled
        # Responses still enters our replay-only adapter; no fresh compressor runs while paused.
        if name == "optimize" and state is not None and state.kind in {"responses", "responses_ws"}:
            return value
        return bool(value) and enabled

    def __setattr__(self, name: str, value: Any):
        setattr(self._base, name, value)


class ControlMiddleware:
    def __init__(self, app: Any, *, gate: Gate):
        self.app, self.gate = app, gate

    async def _reply(self, send: Any, status: int, body: dict):
        data = _encoded(body)
        await send({"type": "http.response.start", "status": status,
                    "headers": [(b"content-type", b"application/json"), (b"cache-control", b"no-store"), (b"content-length", str(len(data)).encode())]})
        await send({"type": "http.response.body", "body": data})

    async def control(self, scope: dict, receive: Any, send: Any, headers: dict):
        if scope.get("method") != "POST" or not hmac.compare_digest(headers.get("x-headroom-desktop-control", "").encode("latin1"), self.gate._secret.encode("ascii")):
            await self._reply(send, 404, {"error": "not_found"}); return
        raw = bytearray()
        while True:
            message = await receive()
            if message["type"] != "http.request": return
            raw.extend(message.get("body", b""))
            if len(raw) > 2048:
                await self._reply(send, 400, {"error": "invalid_input"}); return
            if not message.get("more_body"): break
        try:
            def unique(values):
                result = {}
                for k, v in values:
                    if k in result: raise ValueError()
                    result[k] = v
                return result
            value = json.loads(raw, object_pairs_hook=unique)
            action = value.get("action")
            keys = {"schema_version", "request_id", "runtime_generation", "action"}
            if action == "set": keys |= {"expected_revision", "enabled"}
            if not isinstance(value, dict) or set(value) != keys or type(value["schema_version"]) is not int or value["schema_version"] != 1:
                raise ValueError()
            request_id = str(uuid.UUID(value["request_id"]))
            if request_id != value["request_id"] or uuid.UUID(request_id).int == 0 or action not in {"read", "set"}:
                raise ValueError()
            if value["runtime_generation"] != self.gate.generation: raise ValueError()
            if action == "set" and (type(value["expected_revision"]) is not int or value["expected_revision"] < 0 or type(value["enabled"]) is not bool):
                raise ValueError()
        except (ValueError, TypeError, KeyError, AttributeError, UnicodeError):
            await self._reply(send, 400, {"error": "invalid_input"}); return
        code = None
        with self.gate.lock:
            if action == "set":
                if value["expected_revision"] != self.gate.revision: code = "config_conflict"
                elif not value["enabled"] and not self.gate.history_safe: code = "config_conflict"
                elif self.gate.revision >= 9_007_199_254_740_991: code = "config_conflict"
                elif self.gate.enabled != value["enabled"]:
                    self.gate.enabled = value["enabled"]; self.gate.revision += 1
            data = self.gate.state()
        await self._reply(send, 409 if code else 200,
                          {"schema_version": 1, "request_id": request_id, "runtime_generation": self.gate.generation,
                           "status": "error" if code else "ok", "code": code, **data})

    async def __call__(self, scope: dict, receive: Any, send: Any):
        if scope["type"] not in {"http", "websocket"}:
            await self.app(scope, receive, send); return
        path = scope.get("path", "")
        headers = {}
        for key, value in scope.get("headers", []):
            # Starlette's Headers.get selects the first value, including duplicate auth headers.
            headers.setdefault(key.decode("latin1").lower(), value.decode("latin1"))
        if path.startswith("/__headroom_desktop"):
            if scope["type"] == "http" and path == CONTROL_PATH:
                await self.control(scope, receive, send, headers)
            elif scope["type"] == "http": await self._reply(send, 404, {"error": "not_found"})
            else: await send({"type": "websocket.close", "code": 1008})
            return
        kind = "responses_ws" if scope["type"] == "websocket" else "responses" if path.endswith("/responses") else "anthropic" if path.endswith("/messages") else "other"
        route_headers = ("authorization", "x-api-key", "chatgpt-account-id", "openai-organization", "openai-project", "x-headroom-project-id", "x-headroom-cwd", "x-headroom-base-url", "x-headroom-original-path", "anthropic-beta", "anthropic-version", "conversation_id", "session_id", "x-headroom-session-id", "x-codex-thread-id")
        state = RequestState(self.gate, self.gate.enabled, kind,
                             headers.get("x-headroom-bypass", "").lower() == "true" or headers.get("x-headroom-mode", "").lower() == "passthrough",
                             _digest([path, "responses" if kind == "responses_ws" else kind, [(k, headers.get(k)) for k in route_headers]]))
        token = _request.set(state)
        counted = scope["type"] == "http" and scope.get("method") == "POST" and kind in {"responses", "anthropic"} or scope.get("method") == "POST" and path.endswith("/chat/completions")
        started = False
        async def observed_send(message):
            nonlocal started
            if message["type"] == "http.response.start": started = True
            await send(message)
        with self.gate.lock:
            if counted: self.gate.active_http += 1
        try:
            await self.app(scope, receive, observed_send)
        except BaseException:
            if not state.replay_failed or started or scope["type"] != "http": raise
            await self._reply(send, 503, {"error": {"type": "desktop_pause_unavailable", "message": "Safe prefix replay is unavailable."}})
        finally:
            with self.gate.lock:
                if counted: self.gate.active_http -= 1
            _request.reset(token)


def install(generation: str, secret: str) -> dict[str, str]:
    import headroom._version
    import headroom.cache.prefix_tracker as prefix
    import headroom.proxy.handlers.openai as openai
    import headroom.proxy.server as server

    root = Path(server.__file__).resolve().parents[1]
    expected = json.loads(Path(__file__).with_name("optimization-sources.json").read_text(encoding="utf-8"))
    if headroom._version.__version__ != "0.41.0" or any(hashlib.sha256((root / p).read_bytes()).hexdigest() != h for p, h in expected.items()):
        raise RuntimeError("optimization-source-mismatch")
    if len(generation) != 32 or len(secret) != 32 or any(c not in "0123456789abcdef" for c in generation + secret) or int(generation, 16) == 0 or int(secret, 16) == 0:
        raise RuntimeError("optimization-control-invalid")
    if getattr(server.create_app, "__desktop_control__", False):
        raise RuntimeError("optimization-already-installed")
    original_create = server.create_app
    original_resolve = prefix.SessionTrackerStore.resolve_tracker
    original_normalize = prefix.normalize_message_cache_control
    original_payload = openai.OpenAIHandlerMixin._compress_openai_responses_payload
    original_executor = openai.OpenAIHandlerMixin._compress_openai_responses_payload_in_executor

    def resolve(store, *args, **kwargs):
        tracker = original_resolve(store, *args, **kwargs)
        state = _request.get()
        if state is not None and store is state.gate.proxy_store and state.kind == "anthropic":
            state.previous = (tracker.get_last_original_messages(), tracker.get_last_forwarded_messages())
        return tracker

    def normalize(messages, previous_forwarded_messages=None, client_messages=None):
        state = _request.get()
        if state is not None and state.kind == "anthropic" and not state.enabled and not state.bypass:
            try:
                messages = _preserve(client_messages or [], messages, state.previous, prefix)
            except ReplayUnavailable:
                state.replay_failed = True
                with state.gate.lock: state.gate.history_safe = False
                raise
            from cache_control_shim import _has_client_markers
            if state.previous and state.previous[0] and not _has_client_markers(client_messages):
                # Keep the prior generated breakpoint during pause; do not move it onto a fresh tail.
                return messages
        return original_normalize(messages, previous_forwarded_messages, client_messages)

    def payload(proxy, body, **kwargs):
        state = _request.get()
        if state is None or state.bypass:
            return original_payload(proxy, body, **kwargs)
        gate = state.gate
        key, tracker, previous = gate.prepare(body, state)
        result = original_payload(proxy, body, **kwargs) if state.enabled else (body, False, 0, [], "desktop_paused", len(_encoded(body)), len(_encoded(body)), 0)
        if tracker is None: return result
        candidate, *rest = result
        original_items = body[key]
        try:
            forwarded = _preserve(original_items, candidate[key], previous, prefix)
        except ReplayUnavailable:
            state.replay_failed = True
            with gate.lock: gate.history_safe = False
            raise
        candidate = {**candidate, key: forwarded}
        if previous and previous[0] and hasattr(tracker, "_desktop_forwarded_tools"):
            old_tools = tracker._desktop_forwarded_tools
            if old_tools is not None: candidate["tools"] = copy.deepcopy(old_tools)
        gate.record(tracker, original_items, forwarded, candidate.get("tools"), state)
        # Count only the final payload delta; replay does not book fresh compression attempts.
        try:
            tokenizer = proxy.openai_provider.get_token_counter(kwargs["model"])
            saved = tokenizer.count_text(_encoded(body).decode()) - tokenizer.count_text(_encoded(candidate).decode())
        except Exception:
            state.replay_failed = True
            raise ReplayUnavailable() from None
        return candidate, candidate != body, saved, rest[2] if state.enabled else [], rest[3], len(_encoded(body)), len(_encoded(candidate)), rest[6] if state.enabled else 0

    async def executor(proxy, body, **kwargs):
        state = _request.get()
        if state is None: return await original_executor(proxy, body, **kwargs)
        frame = RequestState(state.gate, state.gate.enabled, state.kind, state.bypass, state.scope_key) if state.kind == "responses_ws" else state
        token = _request.set(frame)
        try: return await original_executor(proxy, body, **kwargs)
        except Exception:
            # A timed-out worker can finish later; its prepared bytes were not acknowledged for forwarding.
            frame.gate.fail_replay(frame)
            raise
        except BaseException:
            with frame.gate.lock: frame.gate.history_safe = False
            raise
        finally: _request.reset(token)

    def create(config=None):
        app = original_create(config)
        proxy = app.state.proxy
        if proxy.config.mode != "cache" or not proxy.config.optimize:
            raise RuntimeError("optimization-profile-mismatch")
        gate = Gate(generation, secret, prefix)
        gate.proxy_store = proxy.session_tracker_store
        proxy.config = ScopedConfig(proxy.config, gate)
        app.add_middleware(ControlMiddleware, gate=gate)
        app.state.desktop_control = gate
        return app

    create.__desktop_control__ = True
    prefix.SessionTrackerStore.resolve_tracker = resolve
    prefix.normalize_message_cache_control = normalize
    openai.OpenAIHandlerMixin._compress_openai_responses_payload = payload
    openai.OpenAIHandlerMixin._compress_openai_responses_payload_in_executor = executor
    server.create_app = create
    return {"active": "true", "version": "0.41.0", "source_sha256": hashlib.sha256(_encoded(expected)).hexdigest(),
            "module_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "sources_sha256": hashlib.sha256(Path(__file__).with_name("optimization-sources.json").read_bytes()).hexdigest()}
