"""Application-owned, in-memory pause at request boundaries for the pinned engine."""
from __future__ import annotations

from collections import Counter, deque
import contextvars
import copy
import hashlib
import hmac
import json
import re
import threading
import uuid
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

CONTROL_PATH = "/__headroom_desktop/control"
USAGE_PATH = "/__headroom_desktop/provider-usage"
ANTHROPIC_MESSAGES_PATH = "/v1/messages"
RESPONSES_PATH = "/responses"
RESPONSES_SESSION_HEADERS = ("conversation_id", "session_id", "x-headroom-session-id", "x-codex-thread-id")
CHAT_COMPLETIONS_PATH = "/chat/completions"
MAX_PAYLOAD = 2 * 1024 * 1024
MAX_TRACKERS = 64
MAX_REPLAY_BYTES = 64 * 1024 * 1024
MAX_USAGE_ATTEMPTS = 16
MAX_USAGE_EVENT_BYTES = 128 * 1024
MAX_TOKEN_COUNT = (1 << 53) - 1
USAGE_FIELDS = ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens", "output_tokens")
REPEAT_ID = re.compile(r"[A-Za-z0-9_-]{1,96}")
REPEAT_START = "[Identical to the complete tool result above (call_id: "
REPEAT_END = ").]"
REPEAT_STATUS_TEXT = re.compile(
    r"(?im)^[^\S\r\n]*(?:stderr\b|exit[ _-](?:code|status)\b|process exited\b|error\b|fatal\b|traceback\b)"
)
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
    usage: UsageTrace | None = None
    responses_ccr_failed: bool = False
    ws_usage: WebSocketUsage | None = None
    explicit_session: bool = False
    skip_anthropic_generated_marker: bool = False


def _token_count(value):
    return value if type(value) is int and 0 <= value <= MAX_TOKEN_COUNT else None


def _sum_known(values):
    return _token_count(sum(values)) if values and all(value is not None for value in values) else None


@dataclass
class UsageTrace:
    surface: str = "anthropic_messages"
    request_id: str | None = None
    conflicting_ids: bool = False
    attempts: list[dict] = field(default_factory=list)
    truncated: bool = False
    failed: bool = False

    def begin_attempt(self) -> dict | None:
        if len(self.attempts) >= MAX_USAGE_ATTEMPTS:
            self.truncated = True
            return None
        attempt = {"status": None, "usage": None}
        self.attempts.append(attempt)
        return attempt

    def has_verified_responses_stream_usage(self) -> bool:
        return (self.surface == "openai_responses" and not self.truncated and bool(self.attempts)
                and all(attempt.get("_verified_responses_terminal") is True for attempt in self.attempts))

    def snapshot(self, status: int | None) -> dict:
        usage = {}
        for name in USAGE_FIELDS:
            values = [(attempt["usage"] or {}).get(name) if (
                attempt.get("completed") is True if self.surface == "openai_responses_ws" else
                attempt.get("status") is not None and 200 <= attempt["status"] < 300
            ) else None for attempt in self.attempts]
            usage[name] = None if self.truncated else _sum_known(values)
        usage["total_input_tokens"] = (_sum_known([usage[name] for name in USAGE_FIELDS[:3]])
                                        if self.surface == "anthropic_messages" else usage["input_tokens"])
        usage["total_tokens"] = _sum_known([usage["total_input_tokens"], usage["output_tokens"]])
        attempts = [{key: copy.deepcopy(value) for key, value in attempt.items() if key != "_verified_responses_terminal"}
                    for attempt in self.attempts]
        result = {"surface": self.surface, "request_id": self.request_id, "client_status": status, "truncated": self.truncated,
                  "attempts": attempts, "usage": usage}
        if self.surface == "openai_responses_ws":
            result["failed"] = self.failed
        return result


def _anthropic_json_usage(raw: bytes) -> dict | None:
    if len(raw) > MAX_PAYLOAD:
        return None
    value = json.loads(raw)
    if not isinstance(value, dict) or value.get("type") != "message":
        return None
    usage = value.get("usage")
    if not isinstance(usage, dict):
        return None
    return {name: _token_count(usage.get(name)) for name in USAGE_FIELDS}


def _bounded_usage_events(buffer, chunk, parse_events):
    # Draining complete events restores space while leaving a split event in the buffer.
    remaining = memoryview(chunk)
    while remaining:
        room = MAX_USAGE_EVENT_BYTES - len(buffer)
        if room <= 0:
            raise ValueError("usage_event_limit")
        buffer.extend(remaining[:room])
        yield from parse_events(buffer)
        remaining = remaining[room:]


async def _observe_anthropic_sse(chunks, attempt, parse_events):
    buffer = bytearray()
    usage = dict.fromkeys(USAGE_FIELDS)
    started = stopped = invalid = False
    usage_events = {"message_start", "message_delta", "message_stop", "error"}
    async for chunk in chunks:
        if not invalid:
            try:
                for event_name, data in _bounded_usage_events(buffer, chunk, parse_events):
                    if event_name is not None and event_name not in usage_events:
                        continue
                    event = json.loads(data)
                    if not isinstance(event, dict):
                        raise ValueError()
                    kind = event.get("type")
                    if (not isinstance(kind, str) or (event_name is not None and event_name != kind)
                            or kind == "error" or (kind in usage_events and stopped)):
                        raise ValueError()
                    if kind == "message_start":
                        if started or not isinstance(event.get("message"), dict):
                            raise ValueError()
                        started = True
                        supplied = event["message"].get("usage")
                        if isinstance(supplied, dict):
                            for name in USAGE_FIELDS[:3]:
                                usage[name] = _token_count(supplied.get(name))
                    elif kind == "message_delta" and started:
                        supplied = event.get("usage")
                        if isinstance(supplied, dict):
                            for name in USAGE_FIELDS:
                                if name in supplied:
                                    usage[name] = _token_count(supplied[name])
                    elif kind == "message_stop":
                        if not started:
                            raise ValueError()
                        stopped = True
                        attempt["usage"] = usage.copy()
            except (ValueError, RecursionError):
                invalid = True
                buffer.clear()
                attempt["usage"] = None
        yield chunk


def _responses_usage(value, *, verified_terminal=False):
    if (not isinstance(value, dict) or value.get("object", "response") != "response"
            or not isinstance(value.get("usage"), dict)):
        return None
    status = value.get("status")
    if status != "completed" and not (verified_terminal and status is None):
        return None
    usage = value["usage"]
    details = usage.get("input_tokens_details")
    details = details if isinstance(details, dict) else {}
    result = {"input_tokens": _token_count(usage.get("input_tokens")),
              "cache_read_input_tokens": _token_count(details.get("cached_tokens")),
              "cache_creation_input_tokens": _token_count(details.get("cache_write_tokens")),
              "output_tokens": _token_count(usage.get("output_tokens"))}
    if result["input_tokens"] is not None and any(
            result[name] is not None and result[name] > result["input_tokens"] for name in USAGE_FIELDS[1:3]):
        return None
    return result


async def _observe_responses_sse(chunks, attempt, parse_events):
    buffer = bytearray()
    response_id = None
    stopped = invalid = False
    usage_events = {"response.created", "response.completed", "response.failed", "response.incomplete", "error"}
    async for chunk in chunks:
        if not invalid:
            try:
                for event_name, data in _bounded_usage_events(buffer, chunk, parse_events):
                    if data == "[DONE]" and stopped:
                        continue
                    if event_name is not None and event_name not in usage_events:
                        continue
                    event = json.loads(data)
                    if not isinstance(event, dict):
                        raise ValueError()
                    kind = event.get("type")
                    if (not isinstance(kind, str) or (event_name is not None and event_name != kind)
                            or kind in {"response.failed", "response.incomplete", "error"}
                            or kind in usage_events and stopped):
                        raise ValueError()
                    if kind == "response.created":
                        value = event.get("response")
                        if response_id is not None or not isinstance(value, dict) or not isinstance(value.get("id"), str):
                            raise ValueError()
                        response_id = value["id"]
                    elif kind == "response.completed":
                        value = event.get("response")
                        if not isinstance(value, dict) or response_id is not None and value.get("id") != response_id:
                            raise ValueError()
                        stopped = True
                        verified_id = bool(response_id)
                        # The completed event supplies status when its response ID matches creation.
                        attempt["usage"] = _responses_usage(value, verified_terminal=verified_id)
                        # Codex can close after this event, before the optional [DONE] or HTTP EOF.
                        attempt["_verified_responses_terminal"] = verified_id and attempt["usage"] is not None
            except (ValueError, RecursionError):
                invalid = True
                buffer.clear()
                attempt["usage"] = None
                attempt.pop("_verified_responses_terminal", None)
        yield chunk


def _chat_usage(value, *, chunk=False):
    if (not isinstance(value, dict) or value.get("error") is not None
            or value.get("object") != ("chat.completion.chunk" if chunk else "chat.completion")
            or not isinstance(value.get("usage"), dict)):
        return None
    usage = value["usage"]
    details = usage.get("prompt_tokens_details")
    details = details if isinstance(details, dict) else {}
    prompt = _token_count(usage.get("prompt_tokens"))
    completion = _token_count(usage.get("completion_tokens"))
    total = _token_count(usage.get("total_tokens"))
    cached = _token_count(details.get("cached_tokens"))
    written = _token_count(details.get("cache_write_tokens"))
    if (prompt is not None and (cached or 0) + (written or 0) > prompt
            or None not in (prompt, completion, total) and prompt + completion != total):
        return None
    return dict(zip(USAGE_FIELDS, (prompt, cached, written, completion), strict=True))


async def _observe_chat_sse(chunks, attempt, parse_events):
    buffer = bytearray()
    response_id = None
    usage = None
    usage_seen = done = invalid = False
    async for chunk in chunks:
        if not invalid:
            try:
                for event_name, data in _bounded_usage_events(buffer, chunk, parse_events):
                    if done or event_name not in (None, "message"):
                        raise ValueError()
                    if data == "[DONE]":
                        done = True
                        continue
                    value = json.loads(data)
                    if (usage_seen or not isinstance(value, dict) or value.get("error") is not None
                            or value.get("object") != "chat.completion.chunk" or not isinstance(value.get("id"), str)
                            or not value["id"] or response_id is not None and value["id"] != response_id
                            or not isinstance(value.get("choices"), list)):
                        raise ValueError()
                    response_id = value["id"]
                    if value.get("usage") is not None:
                        if value["choices"]:
                            raise ValueError()
                        usage_seen = True
                        usage = _chat_usage(value, chunk=True)
            except (ValueError, RecursionError):
                invalid = True
                buffer.clear()
        yield chunk
    if not invalid and done and not buffer:
        attempt["usage"] = usage


def _install_usage(proxy, gate, parse_events, response_not_read):
    startup = proxy.startup
    record_outcome = proxy._record_request_outcome
    transports = {
        "anthropic_messages": (ANTHROPIC_MESSAGES_PATH, _observe_anthropic_sse),
        "openai_responses": (RESPONSES_PATH, _observe_responses_sse),
        "openai_chat_completions": (CHAT_COMPLETIONS_PATH, _observe_chat_sse),
    }

    async def start():
        await startup()
        clients = {id(client): client for name in ("http_client", "http_client_h1")
                   if (client := getattr(proxy, name, None)) is not None}
        for client in clients.values():
            # Bind each transport separately: ChatGPT can use the dedicated HTTP/1 client.
            async def observe(request, _send=client.send, **options):
                state = _request.get()
                trace = state.usage if state is not None and state.gate is gate else None
                if trace is None or request.method != "POST":
                    return await _send(request, **options)
                transport = transports.get(trace.surface)
                if transport is None or not request.url.path.endswith(transport[0]):
                    return await _send(request, **options)
                attempt = trace.begin_attempt()
                response = await _send(request, **options)
                if attempt is None:
                    return response
                attempt["status"] = response.status_code
                if not response.is_success or response.history:
                    return response
                media_type = response.headers.get("content-type", "").split(";", 1)[0].strip().lower()
                if media_type == "application/json" and response.is_stream_consumed:
                    try:
                        if trace.surface == "anthropic_messages":
                            attempt["usage"] = _anthropic_json_usage(response.content)
                        elif len(response.content) <= MAX_PAYLOAD:
                            parse = _chat_usage if trace.surface == "openai_chat_completions" else _responses_usage
                            attempt["usage"] = parse(json.loads(response.content))
                    except (ValueError, RecursionError, response_not_read):
                        pass
                elif (media_type == "text/event-stream"
                      or trace.surface == "openai_responses" and options.get("stream") is True):
                    read = response.aiter_bytes
                    observer = transport[1]

                    def observed_read(*args, **kwargs):
                        return observer(read(*args, **kwargs), attempt, parse_events)

                    response.aiter_bytes = observed_read
                return response

            client.send = observe

    async def record(outcome):
        state = _request.get()
        owned = state is not None and state.gate is gate
        websocket = owned and state.kind == "responses_ws"
        ledger = state.ws_usage if websocket else None
        trace = state.usage if owned else None
        if websocket:
            if trace is not None:
                state.usage = None
            elif ledger is not None:
                trace = ledger.take()
        if trace is not None:
            # The engine can log the outcome and then be cancelled during stream cleanup.
            request_id = outcome.request_id
            if not isinstance(request_id, str) or not re.fullmatch(r"hr_[0-9]{1,20}_[0-9]{6,20}", request_id):
                trace.conflicting_ids = True
            elif trace.request_id is None:
                trace.request_id = request_id
            elif trace.request_id != request_id:
                trace.conflicting_ids = True
        try:
            await record_outcome(outcome)
        except BaseException:
            if ledger is not None:
                ledger.uncertain = True
            raise
        if trace is None:
            return
        if websocket and trace.request_id and not trace.conflicting_ids:
            with gate.lock:
                gate.usage_records.append(trace.snapshot(None))

    proxy.startup = start
    proxy._record_request_outcome = record


def _install_responses_ccr_errors(proxy, gate):
    retry = proxy._retry_request

    async def checked(*args, **kwargs):
        response = await retry(*args, **kwargs)
        state = _request.get()
        if (state is not None and state.gate is gate and state.kind == "responses"
                and kwargs.get("forwarder_name") == "openai_responses_ccr_continuation"
                and not response.is_success):
            # Its CCR handler catches exceptions and can return the preceding tool call.
            state.responses_ccr_failed = True
        return response

    proxy._retry_request = checked


class WebSocketUsage:
    """Only ids and bounded counters survive until their exact outcome is recorded."""
    def __init__(self):
        self.active = {}
        self.seen = deque(maxlen=64)
        self.ready = deque()
        self.pending_failed = []
        self.failed = False
        self.uncertain = False

    def observe(self, event):
        kind = event.get("type") if isinstance(event, dict) else None
        if not isinstance(kind, str):
            self.uncertain = True
            return
        if kind == "error":
            self.uncertain = self.failed = True
            return
        if kind not in {"response.created", "response.completed", "response.failed", "response.incomplete"}:
            return
        response = event.get("response")
        identity = response.get("id") if isinstance(response, dict) else None
        if not isinstance(identity, str) or not 0 < len(identity) <= 128:
            self.uncertain = True
            return
        if kind == "response.created":
            if identity in self.active or identity in self.seen or len(self.active) >= MAX_USAGE_ATTEMPTS:
                self.uncertain = True
            else:
                self.active[identity] = {"completed": False, "usage": None}
            return
        attempt = self.active.pop(identity, None)
        if attempt is None or identity in self.seen:
            self.uncertain = True
        self.seen.append(identity)
        if kind == "response.completed":
            if attempt is not None:
                attempt.update(completed=True, usage=_responses_usage(response))
            trace = UsageTrace(surface="openai_responses_ws", truncated=self.uncertain or bool(self.pending_failed), failed=self.failed)
            trace.attempts = (self.pending_failed + ([attempt] if attempt is not None else []))[:MAX_USAGE_ATTEMPTS]
            self.pending_failed = []
            self.failed = False
            if len(self.ready) >= MAX_USAGE_ATTEMPTS:
                self.uncertain = True
                self.ready.clear()
            self.ready.append(trace)
        else:
            self.failed |= kind == "response.failed"
            if attempt is not None and len(self.pending_failed) < MAX_USAGE_ATTEMPTS:
                self.pending_failed.append(attempt)
            else:
                self.uncertain = True

    def take(self):
        if self.ready:
            trace = self.ready.popleft()
            trace.truncated |= self.uncertain
            return trace
        if not (self.active or self.pending_failed or self.uncertain):
            return None
        trace = UsageTrace(surface="openai_responses_ws", truncated=True, failed=self.failed)
        trace.attempts = (self.pending_failed + list(self.active.values()))[:MAX_USAGE_ATTEMPTS]
        self.active.clear()
        self.pending_failed = []
        self.failed = False
        return trace


def _install_ws_fallback_usage(proxy, gate):
    fallback = proxy._ws_http_fallback

    async def observe(*args, **kwargs):
        state = _request.get()
        if state is not None and state.gate is gate and state.kind == "responses_ws":
            prior = state.ws_usage
            partial = prior is not None and bool(prior.active or prior.seen or prior.pending_failed or prior.ready or prior.uncertain)
            state.ws_usage = None
            state.usage = UsageTrace(surface="openai_responses", truncated=partial)
        return await fallback(*args, **kwargs)

    proxy._ws_http_fallback = observe


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


def _responses_call_counts(items):
    return Counter(item["call_id"] for item in items
                   if isinstance(item, dict) and isinstance(item.get("call_id"), str))


def _restore_ambiguous_responses_references(original, forwarded, previous):
    if not previous or len(original) != len(forwarded):
        return forwarded
    old, sent = previous
    restored = forwarded
    counts = None
    for index, item in enumerate(forwarded):
        if not isinstance(item, dict) or item.get("type") != "function_call_output":
            continue
        text = item.get("output")
        if not isinstance(text, str) or not text.startswith(REPEAT_START) or not text.endswith(REPEAT_END):
            continue
        source = text[len(REPEAT_START):-len(REPEAT_END)]
        if not REPEAT_ID.fullmatch(source) or item == original[index]:
            continue
        if counts is None:
            counts = _responses_call_counts(original), _responses_call_counts(forwarded)
            # Reuse the prior proof only while both versions of its complete prefix still match.
            stable = 0
            for raw, wire, prior_raw, prior_wire in zip(original, forwarded, old, sent):
                if raw != prior_raw or wire != prior_wire:
                    break
                stable += 1
        target = item.get("call_id")
        if index < stable and isinstance(target, str) and all(seen[source] == seen[target] == 2 for seen in counts):
            continue
        if restored is forwarded:
            restored = list(forwarded)
        restored[index] = original[index]
    return restored


def _repeat_response_outputs(original, forwarded, previous, protected_ids, tool_excluded, count_text):
    """Replace a new whole result only when its source is in the forwarded prefix."""
    if not previous or len(original) != len(forwarded):
        return forwarded, 0
    old, sent = previous
    boundary = len(old)
    if (not 0 < boundary < len(original) or len(sent) != boundary
            or original[:boundary] != old or forwarded[:boundary] != sent):
        return forwarded, 0

    uses = Counter()
    owners = {}
    for index, item in enumerate(original):
        identity = item.get("call_id") if isinstance(item, dict) else None
        if not isinstance(identity, str) or not REPEAT_ID.fullmatch(identity):
            continue
        uses[identity] += 1
        if item.get("type") == "function_call":
            owners[identity] = (index, item)
    forwarded_uses = _responses_call_counts(forwarded)

    sources = {}
    repeated = forwarded
    attempted = 0
    for index, item in enumerate(original):
        if not isinstance(item, dict) or item.get("type") != "function_call_output" or forwarded[index] != item:
            continue
        identity, value = item.get("call_id"), item.get("output")
        if (not isinstance(identity, str) or uses[identity] != 2 or forwarded_uses[identity] != 2
                or identity not in owners or identity in protected_ids
                or set(item) - {"type", "call_id", "id", "status", "output"} or item.get("status") not in (None, "completed")
                or not isinstance(value, str) or value.count("\n") < 2 or value.lstrip().startswith(("{", "[", "<"))
                or REPEAT_STATUS_TEXT.search(value) or "<<ccr:" in value or ("Retrieve " in value and "hash=" in value)):
            continue
        owner_index, owner = owners[identity]
        name, arguments = owner.get("name"), owner.get("arguments")
        if (owner_index >= index or forwarded[owner_index] != owner or owner.get("status") not in (None, "completed")
                or set(owner) - {"type", "call_id", "id", "status", "name", "arguments"}
                or not isinstance(name, str) or not name or tool_excluded(name) or not isinstance(arguments, str)):
            continue
        key = (name, arguments, value)
        if index < boundary:
            sources.setdefault(key, identity)
        elif (source := sources.get(key)) is not None:
            pointer = f"{REPEAT_START}{source}{REPEAT_END}"
            tokens = count_text(value)
            if len(pointer.encode()) < len(value.encode()) and count_text(pointer) < tokens:
                if repeated is forwarded:
                    repeated = list(forwarded)
                repeated[index] = {**item, "output": pointer}
                attempted += tokens
    return repeated, attempted


class Gate:
    def __init__(self, generation: str, secret: str, prefix: Any):
        self.generation, self._secret, self.prefix = generation, secret, prefix
        self.enabled, self.revision, self.history_safe = True, 0, True
        self.active_http = 0
        self.usage_records = deque(maxlen=64)
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

    def _authorized(self, headers: dict) -> bool:
        return hmac.compare_digest(headers.get("x-headroom-desktop-control", "").encode("latin1"),
                                   self.gate._secret.encode("ascii"))

    async def control(self, scope: dict, receive: Any, send: Any, headers: dict):
        if scope.get("method") != "POST" or not self._authorized(headers):
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
            elif (scope["type"] == "http" and path == USAGE_PATH and scope.get("method") == "GET"
                  and self._authorized(headers)):
                with self.gate.lock:
                    records = list(self.gate.usage_records)
                await self._reply(send, 200, {"schema_version": 2, "runtime_generation": self.gate.generation,
                                              "surface": "provider_usage", "records": records})
            elif scope["type"] == "http": await self._reply(send, 404, {"error": "not_found"})
            else: await send({"type": "websocket.close", "code": 1008})
            return
        kind = "responses_ws" if scope["type"] == "websocket" else "responses" if path.endswith("/responses") else "anthropic" if path.endswith("/messages") else "other"
        route_headers = ("authorization", "x-api-key", "chatgpt-account-id", "openai-organization", "openai-project", "x-headroom-project-id", "x-headroom-cwd", "x-headroom-base-url", "x-headroom-original-path", "anthropic-beta", "anthropic-version", *RESPONSES_SESSION_HEADERS)
        state = RequestState(self.gate, self.gate.enabled, kind,
                             headers.get("x-headroom-bypass", "").lower() == "true" or headers.get("x-headroom-mode", "").lower() == "passthrough",
                              _digest([path, "responses" if kind == "responses_ws" else kind, [(k, headers.get(k)) for k in route_headers]]),
                              explicit_session=any(headers.get(key, "").strip() for key in RESPONSES_SESSION_HEADERS))
        post = scope["type"] == "http" and scope.get("method") == "POST"
        if post and path == ANTHROPIC_MESSAGES_PATH:
            state.usage = UsageTrace()
        elif post and path.endswith(RESPONSES_PATH):
            state.usage = UsageTrace(surface="openai_responses")
        elif post and path == "/v1" + CHAT_COMPLETIONS_PATH:
            state.usage = UsageTrace(surface="openai_chat_completions")
        elif scope["type"] == "websocket" and path.endswith(RESPONSES_PATH):
            state.ws_usage = WebSocketUsage()
        token = _request.set(state)
        counted = post and (kind in {"responses", "anthropic"} or path.endswith(CHAT_COMPLETIONS_PATH))
        started = False
        completed = False
        status = None
        rejected = False
        async def observed_send(message):
            nonlocal started, status, rejected
            if message["type"] == "http.response.start":
                started = True
                status = message["status"]
                if state.responses_ccr_failed:
                    rejected = True
                    status = 502
                    await self._reply(send, status, {"error": {"type": "upstream_error",
                        "message": "Unable to complete CCR retrieval."}})
                    return
            if rejected:
                return
            await send(message)
        with self.gate.lock:
            if counted: self.gate.active_http += 1
        try:
            await self.app(scope, receive, observed_send)
            completed = True
        except BaseException:
            if not state.replay_failed or started or scope["type"] != "http": raise
            await self._reply(send, 503, {"error": {"type": "desktop_pause_unavailable", "message": "Safe prefix replay is unavailable."}})
        finally:
            with self.gate.lock:
                if counted: self.gate.active_http -= 1
                usage = state.usage
                if (status is not None and usage is not None
                        and (completed or 200 <= status < 300 and usage.has_verified_responses_stream_usage())
                        and usage.request_id and not usage.conflicting_ids and usage.attempts):
                    self.gate.usage_records.append(usage.snapshot(status))
            _request.reset(token)


def install(generation: str, secret: str) -> dict[str, str]:
    import httpx
    import headroom._version
    import headroom.cache.prefix_tracker as prefix
    import headroom.proxy.helpers as helpers
    import headroom.proxy.handlers.openai as openai
    import headroom.proxy.server as server
    from cache_control_shim import _has_client_markers
    from headroom.config import DEFAULT_BYTE_EXACT_EXCLUDE_TOOLS, DEFAULT_EXCLUDE_TOOLS, DEFAULT_VERBATIM_EXCLUDE_TOOLS, is_tool_excluded
    from headroom.proxy.sse_byte_buffer_policy import parse_sse_events_from_byte_buffer

    root = Path(server.__file__).resolve().parents[1]
    expected = json.loads(Path(__file__).with_name("optimization-sources.json").read_text(encoding="utf-8"))
    if headroom._version.__version__ != "0.44.0" or any(hashlib.sha256((root / p).read_bytes()).hexdigest() != h for p, h in expected.items()):
        raise RuntimeError("optimization-source-mismatch")
    if len(generation) != 32 or len(secret) != 32 or any(c not in "0123456789abcdef" for c in generation + secret) or int(generation, 16) == 0 or int(secret, 16) == 0:
        raise RuntimeError("optimization-control-invalid")
    if getattr(server.create_app, "__desktop_control__", False):
        raise RuntimeError("optimization-already-installed")
    original_create = server.create_app
    original_resolve = prefix.SessionTrackerStore.resolve_tracker
    original_normalize = prefix.normalize_message_cache_control
    original_read = helpers.read_request_json_with_bytes
    original_payload = openai.OpenAIHandlerMixin._compress_openai_responses_payload
    original_executor = openai.OpenAIHandlerMixin._compress_openai_responses_payload_in_executor
    original_ws_usage = openai._extract_responses_usage
    repeat_protection = contextvars.ContextVar("headroom_desktop_repeat_protection", default=None)
    protected_tools = DEFAULT_EXCLUDE_TOOLS | DEFAULT_VERBATIM_EXCLUDE_TOOLS | DEFAULT_BYTE_EXACT_EXCLUDE_TOOLS

    def defer_responses_repeat(items, output_types, count_tokens=None, protected_call_ids=None):
        # The vendor invokes this only with dedup enabled. Use its protected IDs after replay.
        pending = repeat_protection.get()
        if pending is not None:
            pending.append(set(protected_call_ids or ()))
        return 0, 0

    def websocket_usage(event):
        result = original_ws_usage(event)
        state = _request.get()
        if state is not None and state.kind == "responses_ws" and state.ws_usage is not None:
            state.ws_usage.observe(event)
        return result

    def resolve(store, *args, **kwargs):
        tracker = original_resolve(store, *args, **kwargs)
        state = _request.get()
        if state is not None and store is state.gate.proxy_store and state.kind == "anthropic":
            state.previous = (tracker.get_last_original_messages(), tracker.get_last_forwarded_messages())
        return tracker

    async def read_request(request):
        body, raw = await original_read(request)
        state = _request.get()
        if (state is not None and state.kind == "anthropic" and not state.bypass
                and len(raw) <= helpers.request_limit_policy.COMPRESSION_BODY_MAX_BYTES):
            # An automatic directive owns the final eligible block and uses a breakpoint slot.
            state.skip_anthropic_generated_marker = isinstance(body.get("cache_control"), dict) or (
                helpers.count_cache_breakpoints(body.get("system"), body.get("messages"), body.get("tools"))["total"] >= 4
            )
        return body, raw

    def normalize(messages, previous_forwarded_messages=None, client_messages=None):
        state = _request.get()
        if state is not None and state.kind == "anthropic" and not state.bypass:
            if not state.enabled:
                try:
                    messages = _preserve(client_messages or [], messages, state.previous, prefix)
                except ReplayUnavailable:
                    state.replay_failed = True
                    with state.gate.lock: state.gate.history_safe = False
                    raise
            paused_prefix = not state.enabled and state.previous and state.previous[0]
            if (state.skip_anthropic_generated_marker or paused_prefix) and not _has_client_markers(client_messages):
                if state.skip_anthropic_generated_marker:
                    # Drop a generated marker carried forward by replay before the request is recorded.
                    return prefix.mirror_client_message_cache_control(messages, client_messages)
                return messages
        return original_normalize(messages, previous_forwarded_messages, client_messages)

    def payload(proxy, body, **kwargs):
        state = _request.get()
        if state is None or state.bypass:
            return original_payload(proxy, body, **kwargs)
        gate = state.gate
        key, tracker, previous = gate.prepare(body, state)
        stateless = body.get("previous_response_id") is None and body.get("conversation") is None
        protection = [] if state.explicit_session and stateless and key == "input" and previous else None
        if state.enabled:
            token = repeat_protection.set(protection)
            try:
                result = original_payload(proxy, body, **kwargs)
            finally:
                repeat_protection.reset(token)
        else:
            size = len(_encoded(body))
            result = (body, False, 0, [], "desktop_paused", size, size, 0)
        if tracker is None: return result
        candidate, *rest = result
        original_items = body[key]
        try:
            forwarded = _preserve(original_items, candidate[key], previous, prefix)
        except ReplayUnavailable:
            state.replay_failed = True
            with gate.lock: gate.history_safe = False
            raise
        if key == "input":
            forwarded = _restore_ambiguous_responses_references(original_items, forwarded, previous)
        candidate = {**candidate, key: forwarded}
        if previous and previous[0] and hasattr(tracker, "_desktop_forwarded_tools"):
            old_tools = tracker._desktop_forwarded_tools
            if old_tools is not None: candidate["tools"] = copy.deepcopy(old_tools)
        # Count only the final payload delta; replay does not book fresh compression attempts.
        transforms = list(rest[2]) if state.enabled else []
        reason, attempted = rest[3], rest[6] if state.enabled else 0
        try:
            tokenizer = proxy.openai_provider.get_token_counter(kwargs["model"])
            count = tokenizer.count_text
            before, after = _encoded(body), _encoded(candidate)
            before_tokens, after_tokens = count(before.decode()), count(after.decode())
        except Exception:
            state.replay_failed = True
            raise ReplayUnavailable() from None
        if protection and len(protection) == 1:
            try:
                blocked = protected_tools | set(proxy.config.exclude_tools or ())
                repeated, extra = _repeat_response_outputs(original_items, forwarded, previous, protection[0],
                                                            lambda name: is_tool_excluded(name, blocked), count)
                if extra:
                    compacted = _encoded({**candidate, key: repeated})
                    compacted_tokens = count(compacted.decode())
                    if compacted_tokens < after_tokens and len(compacted) < len(after):
                        forwarded, candidate[key], after, after_tokens = repeated, repeated, compacted, compacted_tokens
                        transforms.append("router:responses_exact_repeat")
                        # The router may already have counted these units. Use a lower bound on the union.
                        attempted = max(attempted, extra)
                        reason = None
            except (TypeError, ValueError):
                # Optional repeat detection can leave the previously validated payload in place.
                pass
        gate.record(tracker, original_items, forwarded, candidate.get("tools"), state)
        return candidate, candidate != body, before_tokens - after_tokens, transforms, reason, len(before), len(after), attempted

    async def executor(proxy, body, **kwargs):
        state = _request.get()
        if state is None: return await original_executor(proxy, body, **kwargs)
        frame = RequestState(state.gate, state.gate.enabled, state.kind, state.bypass, state.scope_key,
                             explicit_session=state.explicit_session) if state.kind == "responses_ws" else state
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
        _install_usage(proxy, gate, parse_sse_events_from_byte_buffer, httpx.ResponseNotRead)
        _install_responses_ccr_errors(proxy, gate)
        _install_ws_fallback_usage(proxy, gate)
        app.add_middleware(ControlMiddleware, gate=gate)
        app.state.desktop_control = gate
        return app

    create.__desktop_control__ = True
    prefix.SessionTrackerStore.resolve_tracker = resolve
    prefix.normalize_message_cache_control = normalize
    helpers.read_request_json_with_bytes = read_request
    openai.OpenAIHandlerMixin._compress_openai_responses_payload = payload
    openai.OpenAIHandlerMixin._compress_openai_responses_payload_in_executor = executor
    openai._dedup_responses_output_items = defer_responses_repeat
    openai._extract_responses_usage = websocket_usage
    server.create_app = create
    return {"active": "true", "version": "0.44.0", "source_sha256": hashlib.sha256(_encoded(expected)).hexdigest(),
            "module_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "sources_sha256": hashlib.sha256(Path(__file__).with_name("optimization-sources.json").read_bytes()).hexdigest()}
