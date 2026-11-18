"""Bounded WebSocket response ownership; no provider or account access."""
import asyncio
from collections import deque
from pathlib import Path
import runpy
import threading
from types import SimpleNamespace
import unittest

import httpx

ROOT = Path(__file__).resolve().parents[2]
helpers = runpy.run_path(str(ROOT / "tests/runtime/test_responses_usage.py"))
control, response, parser = (helpers[name] for name in ("control", "response", "parser"))


def created(ledger, identity):
    ledger.observe({"type": "response.created", "response": response(id=identity, status="in_progress", usage=None)})


def completed(ledger, identity, total=2400, cached=2000, written=100, output=29):
    details = {"cached_tokens": cached}
    if written is not None:
        details["cache_write_tokens"] = written
    ledger.observe({"type": "response.completed", "response": response(id=identity,
        usage={"input_tokens": total, "output_tokens": output, "input_tokens_details": details})})


class LiveWebSocketSmokeTests(unittest.TestCase):
    def test_websocket_probe_caps_generation_and_checks_model_and_effort(self):
        import json

        probe = runpy.run_path(str(ROOT / "bench/optimization/ws_live_smoke.py"))
        request_kind = probe["request_kind"]
        frame = {"type": "response.create", "model": "fixture-model", "reasoning": {"effort": "medium"}}

        def classify(message, sent=()):
            return request_kind(json.dumps(message), sent, "fixture-model", "medium")

        self.assertEqual(classify({**frame, "generate": False}), "warmup")
        self.assertEqual(classify(frame, ("warmup",)), "generate")
        self.assertIsNone(classify({**frame, "generate": False}, ("warmup",)))
        self.assertIsNone(classify(frame, ("generate",)))
        self.assertIsNone(classify({**frame, "generate": 0}))
        self.assertIsNone(classify({**frame, "reasoning": {"effort": "low"}}))
        self.assertIsNone(classify({**frame, "model": "different"}))

    def test_websocket_probe_needs_scope_and_cli_counter_agreement(self):
        probe = runpy.run_path(str(ROOT / "bench/optimization/ws_live_smoke.py"))
        matched_usage = probe["matched_usage"]
        usage = {"total_input_tokens": 100, "output_tokens": 5, "cache_read_input_tokens": 20, "total_tokens": 105}
        row = {"surface": "openai_responses_ws", "truncated": False, "failed": False,
               "attempts": [{"completed": True}], "usage": usage}
        cli = {"input_tokens": 100, "cached_input_tokens": 20, "output_tokens": 5}
        self.assertEqual(matched_usage([row], cli), usage)
        self.assertIsNone(matched_usage([row, row], cli))
        self.assertIsNone(matched_usage([{**row, "surface": "openai_responses"}], cli))
        self.assertIsNone(matched_usage([row], {**cli, "cached_input_tokens": 0}))
        self.assertIsNone(matched_usage([{**row, "usage": {**usage, "cache_read_input_tokens": None}}], cli))


class WebSocketUsageTests(unittest.IsolatedAsyncioTestCase):
    def test_sequential_zero_missing_and_unfinished_responses(self):
        ledger = control.WebSocketUsage()
        created(ledger, "one")
        completed(ledger, "one", 0, 0, 0, 0)
        trace = ledger.take()
        self.assertEqual(set(trace.snapshot(None)["usage"].values()), {0})
        self.assertEqual(trace.attempts, [{"completed": True, "usage": {
            "input_tokens": 0, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0, "output_tokens": 0}}])
        created(ledger, "two")
        completed(ledger, "two", written=None)
        usage = ledger.take().snapshot(None)["usage"]
        self.assertEqual(usage["total_input_tokens"], 2400)
        self.assertIsNone(usage["cache_creation_input_tokens"])
        created(ledger, "unfinished")
        self.assertTrue(ledger.take().truncated)
        self.assertIsNone(ledger.take())

    def test_interleaved_response_ids_do_not_borrow_active_counters(self):
        ledger = control.WebSocketUsage()
        created(ledger, "a")
        created(ledger, "b")
        completed(ledger, "b", total=800, cached=0, written=200, output=9)
        trace_b = ledger.take()
        self.assertEqual(len(trace_b.attempts), 1)
        self.assertEqual(trace_b.snapshot(None)["usage"]["total_input_tokens"], 800)
        self.assertEqual(set(ledger.active), {"a"})
        completed(ledger, "a")
        self.assertEqual(ledger.take().snapshot(None)["usage"]["total_tokens"], 2429)

    def test_failed_scope_is_unknown_and_next_independent_record_recovers(self):
        ledger = control.WebSocketUsage()
        created(ledger, "failed")
        ledger.observe({"type": "response.failed", "response": response(id="failed", status="failed")})
        created(ledger, "following")
        completed(ledger, "following")
        trace = ledger.take()
        self.assertTrue(trace.failed and trace.truncated)
        self.assertTrue(all(value is None for value in trace.snapshot(None)["usage"].values()))
        created(ledger, "fresh")
        completed(ledger, "fresh")
        self.assertEqual(ledger.take().snapshot(None)["usage"]["total_input_tokens"], 2400)

    def test_duplicate_unknown_id_and_bounded_state_never_claim_usage(self):
        for event in ({"type": []}, {"type": "response.completed", "response": response(id="unknown")},
                      {"type": "response.created", "response": response(id="x" * 129)}):
            ledger = control.WebSocketUsage()
            ledger.observe(event)
            self.assertTrue(ledger.take().truncated)
        ledger = control.WebSocketUsage()
        created(ledger, "same")
        completed(ledger, "same")
        ledger.take()
        completed(ledger, "same")
        self.assertTrue(ledger.take().truncated)
        ledger = control.WebSocketUsage()
        for n in range(100):
            created(ledger, str(n))
        self.assertEqual(len(ledger.active), control.MAX_USAGE_ATTEMPTS)
        self.assertTrue(ledger.take().truncated)
        ledger = control.WebSocketUsage()
        for n in range(100):
            created(ledger, str(n))
            completed(ledger, str(n))
            ledger.take()
        self.assertEqual(len(ledger.seen), 64)

    async def test_outcome_identity_is_selected_before_the_writer_awaits(self):
        entered, release = asyncio.Event(), asyncio.Event()
        async def record(outcome):
            if outcome.request_id == "hr_1_000001":
                entered.set()
                await release.wait()
        async def start():
            pass
        proxy = SimpleNamespace(startup=start, _record_request_outcome=record)
        gate = SimpleNamespace(lock=threading.RLock(), usage_records=deque())
        control._install_usage(proxy, gate, parser.parse_sse_events_from_byte_buffer, httpx.ResponseNotRead)
        ledger = control.WebSocketUsage()
        state = control.RequestState(gate, True, "responses_ws", False, "scope", ws_usage=ledger)
        token = control._request.set(state)
        try:
            created(ledger, "a")
            completed(ledger, "a", 100, 0, 0, 1)
            first = asyncio.create_task(proxy._record_request_outcome(SimpleNamespace(request_id="hr_1_000001")))
            await asyncio.wait_for(entered.wait(), 2)
            created(ledger, "b")
            completed(ledger, "b", 200, 0, 0, 2)
            await proxy._record_request_outcome(SimpleNamespace(request_id="hr_1_000002"))
            release.set()
            await first
        finally:
            release.set()
            control._request.reset(token)
        rows = {row["request_id"]: row for row in gate.usage_records}
        self.assertEqual(rows["hr_1_000001"]["usage"]["input_tokens"], 100)
        self.assertEqual(rows["hr_1_000002"]["usage"]["input_tokens"], 200)
        self.assertTrue(all(row["client_status"] is None for row in rows.values()))

    async def test_http_fallback_is_observed_once_and_partial_ws_stays_unknown(self):
        calls = []
        async def handler(request):
            calls.append(request.content)
            return httpx.Response(200, json=response())
        async def start():
            pass
        async def record(_outcome):
            pass
        async with httpx.AsyncClient(transport=httpx.MockTransport(handler)) as client:
            async def fallback():
                return await client.post("https://fixture.invalid/responses", content=b"original")
            proxy = SimpleNamespace(startup=start, _record_request_outcome=record,
                                    _ws_http_fallback=fallback, http_client=client)
            gate = SimpleNamespace(lock=threading.RLock(), usage_records=deque())
            control._install_usage(proxy, gate, parser.parse_sse_events_from_byte_buffer, httpx.ResponseNotRead)
            control._install_ws_fallback_usage(proxy, gate)
            await proxy.startup()
            state = control.RequestState(gate, True, "responses_ws", False, "scope", ws_usage=control.WebSocketUsage())
            token = control._request.set(state)
            try:
                await proxy._ws_http_fallback()
                self.assertIsNone(state.ws_usage)
                await proxy._record_request_outcome(SimpleNamespace(request_id="hr_1_000001"))
                ledger = control.WebSocketUsage()
                created(ledger, "partial")
                state.ws_usage = ledger
                await proxy._ws_http_fallback()
                await proxy._record_request_outcome(SimpleNamespace(request_id="hr_1_000002"))
            finally:
                control._request.reset(token)
        rows = list(gate.usage_records)
        self.assertEqual(rows[0]["surface"], "openai_responses")
        self.assertEqual(rows[0]["usage"]["input_tokens"], 1000)
        self.assertEqual(len(rows[0]["attempts"]), 1)
        self.assertTrue(rows[1]["truncated"])
        self.assertEqual(calls, [b"original", b"original"])

    async def test_failed_writer_invalidates_later_scopes(self):
        async def record(_outcome):
            raise RuntimeError("fixture")
        async def start():
            pass
        proxy = SimpleNamespace(startup=start, _record_request_outcome=record)
        gate = SimpleNamespace(lock=threading.RLock(), usage_records=deque())
        control._install_usage(proxy, gate, parser.parse_sse_events_from_byte_buffer, httpx.ResponseNotRead)
        ledger = control.WebSocketUsage()
        token = control._request.set(control.RequestState(gate, True, "responses_ws", False, "scope", ws_usage=ledger))
        try:
            created(ledger, "a")
            completed(ledger, "a")
            with self.assertRaisesRegex(RuntimeError, "fixture"):
                await proxy._record_request_outcome(SimpleNamespace(request_id="hr_1_000001"))
        finally:
            control._request.reset(token)
        created(ledger, "b")
        completed(ledger, "b")
        self.assertTrue(ledger.take().truncated)
        self.assertFalse(gate.usage_records)


if __name__ == "__main__":
    unittest.main()
