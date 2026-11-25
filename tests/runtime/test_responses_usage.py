"""Responses counters and byte-preserving observation; no account or model calls."""
import asyncio
import importlib.util
import json
from pathlib import Path
import sys
from types import SimpleNamespace
import unittest
from collections import deque
import threading

import httpx

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


control = load("responses_usage_control", ROOT / "runtime/optimization_control.py")
manifest = json.loads((ROOT / "runtime/manifest.json").read_bytes())
payload = (ROOT / "runtime" / manifest["engine"]["payload_root"]).resolve()
parser = load("responses_usage_sse", payload / "Lib/site-packages/headroom/proxy/sse_byte_buffer_policy.py")


def response(**changes):
    return {"id": "resp-fixture", "object": "response", "status": "completed", "output": [],
            "usage": {"input_tokens": 1000, "output_tokens": 6,
                      "input_tokens_details": {"cached_tokens": 900, "cache_write_tokens": 100}}, **changes}


def event(kind, **fields):
    return ("event: " + kind + "\r\ndata: " + json.dumps({"type": kind, **fields}) + "\r\n\r\n").encode()


class ResponsesShellStatusTests(unittest.TestCase):
    def setUp(self):
        self.call = {"type": "function_call", "name": "exec_command", "call_id": "call-shell"}
        self.output = {"type": "function_call_output", "call_id": "call-shell"}
        self.summary = "Some test lines were compressed.\nRetrieve more: hash=0123456789abcdef"

    def original(self, code=0):
        header = f"Chunk ID: abc123\r\nWall time: 1.2 seconds\r\nProcess exited with code {code}\r\nOutput:\r\n"
        text = header + "A previous test ran.\r\n" * 10 + "Check complete.\r\n"
        return [self.call, {**self.output, "output": text}]

    def test_preserves_success_status_and_retrieval_marker(self):
        original = self.original()
        candidate = [self.call, {**self.output, "output": self.summary}]
        forwarded = control._preserve_responses_shell_status(original, candidate)
        self.assertEqual(forwarded[1]["output"], "Process exited with code 0\r\n" + self.summary)
        self.assertEqual(candidate[1]["output"], self.summary)
        self.assertIs(control._preserve_responses_shell_status(original, forwarded), forwarded)

    def test_keeps_failed_output_exact(self):
        original = self.original(-1)
        candidate = [self.call, {**self.output, "output": self.summary}]
        self.assertEqual(control._preserve_responses_shell_status(original, candidate), original)

    def test_restores_original_if_summary_has_a_conflicting_status(self):
        original = self.original()
        candidate = [self.call, {**self.output, "output": "Process exited with code 1\r\n" + self.summary}]
        self.assertEqual(control._preserve_responses_shell_status(original, candidate), original)

    def test_ignores_unrelated_tools_and_status_text_inside_output(self):
        candidate = [self.call, {**self.output, "output": self.summary}]
        original = self.original()
        original[0] = {**self.call, "name": "read_report"}
        self.assertIs(control._preserve_responses_shell_status(original, candidate), candidate)
        original = self.original()
        original[1]["output"] = "Output:\r\n" + original[1]["output"]
        self.assertIs(control._preserve_responses_shell_status(original, candidate), candidate)

    def test_does_not_assume_an_owner_for_reused_call_ids(self):
        original = self.original()
        candidate = [self.call, {**self.output, "output": self.summary}, self.call]
        self.assertIs(control._preserve_responses_shell_status(original + [self.call], candidate), candidate)


class ResponsesUsageTests(unittest.IsolatedAsyncioTestCase):
    def test_total_input_includes_cache_and_missing_details_remain_unknown(self):
        value = response()
        counters = control._responses_usage(value)
        trace = control.UsageTrace(surface="openai_responses")
        trace.begin_attempt().update(status=200, usage=counters)
        trace.begin_attempt().update(status=200, usage=control._responses_usage(response(
            usage={"input_tokens": 1400, "output_tokens": 23, "input_tokens_details": {"cached_tokens": 1100}})))
        usage = trace.snapshot(200)["usage"]
        self.assertEqual(usage["total_input_tokens"], 2400)
        self.assertEqual(usage["total_tokens"], 2429)
        self.assertEqual(usage["cache_read_input_tokens"], 2000)
        self.assertIsNone(usage["cache_creation_input_tokens"])
        for state in ("in_progress", "incomplete", "failed"):
            self.assertIsNone(control._responses_usage(response(status=state)))
        self.assertEqual(value, response())

    def test_zero_invalid_and_failed_attempts(self):
        zero = response(usage={"input_tokens": 0, "output_tokens": 0,
                               "input_tokens_details": {"cached_tokens": 0, "cache_write_tokens": 0}})
        self.assertEqual(set(control._responses_usage(zero).values()), {0})
        for invalid in (True, -1, 1.5, 2**53):
            value = response(usage={"input_tokens": invalid, "output_tokens": invalid})
            self.assertIsNone(control._responses_usage(value)["input_tokens"])
            self.assertIsNone(control._responses_usage(value)["output_tokens"])
        self.assertIsNone(control._responses_usage(response(usage={
            "input_tokens": 1, "output_tokens": 0, "input_tokens_details": {"cached_tokens": 2}})))
        trace = control.UsageTrace(surface="openai_responses")
        trace.begin_attempt().update(status=429, usage=control._responses_usage(response()))
        trace.begin_attempt().update(status=200, usage=control._responses_usage(zero))
        self.assertTrue(all(value is None for value in trace.snapshot(200)["usage"].values()))
        for _ in range(control.MAX_USAGE_ATTEMPTS):
            trace.begin_attempt()
        self.assertTrue(trace.truncated)

    async def inspect(self, raw, chunk_size=71):
        async def chunks():
            for index in range(0, len(raw), chunk_size):
                yield raw[index:index + chunk_size]
        attempt = {"usage": None}
        actual = [chunk async for chunk in control._observe_responses_sse(
            chunks(), attempt, parser.parse_sse_events_from_byte_buffer)]
        self.assertEqual(b"".join(actual), raw)
        return attempt["usage"]

    async def test_split_sse_is_byte_exact_and_uses_only_completed_response(self):
        created = event("response.created", response=response(status="in_progress", usage=None))
        final = event("response.completed", response=response())
        for size in (1, 71, 4096):
            self.assertEqual(await self.inspect(created + final + b"data: [DONE]\n\n", size),
                             control._responses_usage(response()))
        self.assertIsNone(await self.inspect(created))
        self.assertIsNone(await self.inspect(final[:-4]))

    async def test_completed_event_with_optional_status(self):
        created = event("response.created", response=response(status="in_progress", usage=None))
        expected = control._responses_usage(response())
        missing = response()
        del missing["status"]
        for completed in (missing, response(status=None)):
            with self.subTest(status_present="status" in completed):
                final = event("response.completed", response=completed)
                self.assertEqual(await self.inspect(created + final), expected)
                self.assertIsNone(await self.inspect(final))
                self.assertIsNone(control._responses_usage(completed))
        for completed in (response(status="incomplete"), response(status="failed"),
                          response(id="another", status=None)):
            self.assertIsNone(await self.inspect(created + event("response.completed", response=completed)))

    async def test_invalid_sse_never_reports_partial_or_unbounded_usage(self):
        created = event("response.created", response=response(status="in_progress", usage=None))
        final = event("response.completed", response=response())
        invalid = [event("response.failed", response=response(status="failed")),
                   event("response.incomplete", response=response(status="incomplete")),
                   event("response.completed", response=response(id="another")),
                   b'event: response.completed\ndata: {"type":[]}\n\n']
        for broken in invalid:
            self.assertIsNone(await self.inspect(created + broken + final))
        self.assertIsNone(await self.inspect(final + final))
        self.assertIsNone(await self.inspect(b"x" * (control.MAX_USAGE_EVENT_BYTES + 1)))

    async def test_both_http_transports_and_request_context_are_preserved(self):
        calls = []
        async def handler(request):
            calls.append((request.url.path, request.headers["authorization"], request.content))
            return httpx.Response(200, json=response())
        async def startup():
            pass
        async def record(_outcome):
            pass
        async with httpx.AsyncClient(transport=httpx.MockTransport(handler)) as client, \
                httpx.AsyncClient(transport=httpx.MockTransport(handler)) as h1:
            proxy = SimpleNamespace(startup=startup, _record_request_outcome=record,
                                    http_client=client, http_client_h1=h1)
            gate = object()
            control._install_usage(proxy, gate, parser.parse_sse_events_from_byte_buffer, httpx.ResponseNotRead)
            await proxy.startup()
            state = control.RequestState(gate, True, "responses", False, "scope",
                                         usage=control.UsageTrace(surface="openai_responses"))
            token = control._request.set(state)
            try:
                for transport in (client, h1):
                    result = await transport.post("https://fixture.invalid/backend-api/codex/responses",
                                                  headers={"Authorization": "Bearer fixture"}, content=b"original")
                    self.assertEqual(result.json(), response())
                await proxy._record_request_outcome(SimpleNamespace(request_id="hr_1_000001"))
            finally:
                control._request.reset(token)
            self.assertEqual(len(state.usage.attempts), 2)
            self.assertEqual(state.usage.request_id, "hr_1_000001")
            self.assertEqual(state.usage.snapshot(200)["usage"]["total_input_tokens"], 2000)
            await h1.post("https://fixture.invalid/responses", headers={"Authorization": "Bearer fixture"}, content=b"outside")
            self.assertEqual(len(state.usage.attempts), 2)
            self.assertEqual(calls[:2], [("/backend-api/codex/responses", "Bearer fixture", b"original")] * 2)

    async def test_streamed_responses_usage_with_nonstandard_content_type(self):
        created = event("response.created", response=response(status="in_progress", usage=None))
        terminal = event("response.completed", response=response())
        cases = ((None, created + terminal), ("application/octet-stream", created + terminal),
                 ("text/plain; charset=utf-8", created + terminal), (None, created),
                 ("text/plain", created + event("response.completed", response=response(id="conflict"))))

        class Stream(httpx.AsyncByteStream):
            async def __aiter__(self):
                for index in range(0, len(raw), 71):
                    yield raw[index:index + 71]

        async def handler(request):
            return httpx.Response(200, headers={"content-type": media} if media else {}, stream=Stream())

        async def no_op(*args):
            pass

        async with httpx.AsyncClient(transport=httpx.MockTransport(handler)) as client, \
                httpx.AsyncClient(transport=httpx.MockTransport(handler)) as h1:
            proxy = SimpleNamespace(startup=no_op, _record_request_outcome=no_op,
                                    http_client=client, http_client_h1=h1)
            gate = object()
            control._install_usage(proxy, gate, parser.parse_sse_events_from_byte_buffer, httpx.ResponseNotRead)
            await proxy.startup()
            for transport in (client, h1):
                for media, raw in cases:
                    with self.subTest(media=media, valid=raw == created + terminal):
                        state = control.RequestState(gate, True, "responses", False, "scope",
                                                     usage=control.UsageTrace(surface="openai_responses"))
                        token = control._request.set(state)
                        try:
                            request = transport.build_request("POST", "https://fixture.invalid/backend-api/codex/responses",
                                                              content=b"original")
                            result = await transport.send(request, stream=True)
                            self.assertEqual(b"".join([chunk async for chunk in result.aiter_bytes()]), raw)
                            await result.aclose()
                        finally:
                            control._request.reset(token)
                        expected = control._responses_usage(response()) if raw == created + terminal else None
                        self.assertEqual(state.usage.attempts[0]["usage"], expected)

    async def test_ccr_failure_is_flagged_without_repeating_request(self):
        calls = []
        async def retry(*args, **kwargs):
            calls.append(kwargs["forwarder_name"])
            return httpx.Response(400, json={"error": {"message": "fixture"}},
                                  request=httpx.Request("POST", "https://fixture.invalid/responses"))
        proxy = SimpleNamespace(_retry_request=retry)
        gate = object()
        control._install_responses_ccr_errors(proxy, gate)
        state = control.RequestState(gate, True, "responses", False, "scope")
        token = control._request.set(state)
        try:
            result = await proxy._retry_request(forwarder_name="openai_responses")
            self.assertEqual(result.status_code, 400)
            result = await proxy._retry_request(forwarder_name="openai_responses_ccr_continuation")
            self.assertEqual(result.status_code, 400)
            self.assertTrue(state.responses_ccr_failed)
        finally:
            control._request.reset(token)
        self.assertEqual(calls, ["openai_responses", "openai_responses_ccr_continuation"])

    async def test_ccr_failure_replaces_headers_and_discards_unresolved_body(self):
        async def app(scope, receive, send):
            control._request.get().responses_ccr_failed = True
            await send({"type": "http.response.start", "status": 200, "headers": []})
            await send({"type": "http.response.body", "body": b"UNRESOLVED_CALL", "more_body": False})
        gate = SimpleNamespace(enabled=True, lock=threading.RLock(), active_http=0, usage_records=deque())
        messages = []
        async def send(message):
            messages.append(message)
        await control.ControlMiddleware(app, gate=gate)(
            {"type": "http", "method": "POST", "path": "/v1/responses", "headers": []}, None, send)
        self.assertEqual(messages[0]["status"], 502)
        self.assertNotIn(b"UNRESOLVED_CALL", messages[1]["body"])
        self.assertIn("error", json.loads(messages[1]["body"]))
        self.assertEqual(gate.active_http, 0)

    async def test_completed_responses_usage_survives_cancelled_cleanup(self):
        created = event("response.created", response=response(status="in_progress", usage=None))
        terminal = event("response.completed", response=response())
        missing = response()
        del missing["status"]
        scope = {"type": "http", "method": "POST", "path": "/v1/responses", "headers": []}
        for raw, conflicting, truncated, expected in (
            (created + terminal, False, False, True),
            (created + event("response.completed", response=missing), False, False, True),
            (created, False, False, False),
            (terminal, False, False, False),
            (created + terminal + terminal, False, False, False),
            (created + terminal, True, False, False),
            (created + terminal, False, True, False),
        ):
            with self.subTest(expected_record=expected, conflicting=conflicting, truncated=truncated):
                async def interrupted(_outcome):
                    raise asyncio.CancelledError

                gate = SimpleNamespace(enabled=True, lock=threading.RLock(), active_http=0, usage_records=deque())
                proxy = SimpleNamespace(startup=None, _record_request_outcome=interrupted)
                control._install_usage(proxy, gate, parser.parse_sse_events_from_byte_buffer, httpx.ResponseNotRead)

                async def app(_scope, _receive, send):
                    trace = control._request.get().usage
                    trace.truncated = truncated
                    if conflicting:
                        trace.request_id = "hr_1_000002"
                    attempt = trace.begin_attempt()
                    attempt["status"] = 200

                    async def chunks():
                        yield raw

                    async for _ in control._observe_responses_sse(chunks(), attempt, parser.parse_sse_events_from_byte_buffer):
                        pass
                    await send({"type": "http.response.start", "status": 200, "headers": []})
                    await proxy._record_request_outcome(SimpleNamespace(request_id="hr_1_000001"))

                async def send(_message):
                    pass

                with self.assertRaises(asyncio.CancelledError):
                    await control.ControlMiddleware(app, gate=gate)(scope, None, send)
                self.assertEqual(gate.active_http, 0)
                if expected:
                    self.assertEqual(len(gate.usage_records), 1)
                    recorded = gate.usage_records[0]
                    self.assertEqual(recorded["request_id"], "hr_1_000001")
                    self.assertEqual(recorded["attempts"], [{"status": 200, "usage": control._responses_usage(response())}])
                else:
                    self.assertFalse(gate.usage_records)


if __name__ == "__main__":
    unittest.main()
