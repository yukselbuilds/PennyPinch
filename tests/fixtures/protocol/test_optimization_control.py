"""Run with the project-local pinned engine runtime, never a provider account."""
import asyncio
import contextlib
import hashlib
import io
import json
import sys
import unittest
import uuid
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "runtime"))
import optimization_control as control
import headroom.cache.prefix_tracker as prefix


class CacheMarkerTests(unittest.TestCase):
    def test_runtime_shim_preserves_the_upstream_unmarked_policy(self):
        import cache_control_shim

        messages = [{"role": "user", "content": [{"type": "text", "text": "Fixture"}]}]
        with patch.object(prefix, "normalize_message_cache_control", prefix.normalize_message_cache_control):
            cache_control_shim.install()
            normalize = prefix.normalize_message_cache_control
            self.assertEqual(normalize(messages, client_messages=messages, place_unmarked=False), messages)
            first = normalize(messages, client_messages=messages)[0]["content"][0]
            self.assertEqual(first["cache_control"], {"type": "ephemeral"})


class PauseTests(unittest.TestCase):
    def gate(self): return control.Gate(uuid.uuid4().hex, uuid.uuid4().hex, prefix)
    def state(self, gate, enabled=True, kind="responses"):
        return control.RequestState(gate, enabled, kind, False, "opaque-test-scope")

    async def control_request(self, middleware, gate, raw):
        messages = []
        async def receive(): return {"type": "http.request", "body": raw, "more_body": False}
        async def send(message): messages.append(message)
        await middleware.control({"method": "POST"}, receive, send,
                                 {"x-headroom-desktop-control": gate._secret})
        return messages[0]["status"], json.loads(messages[1]["body"])

    def control_body(self, gate, action="set", **fields):
        value = {"schema_version": 1, "request_id": str(uuid.uuid4()),
                 "runtime_generation": gate.generation, "action": action}
        value.update(fields)
        return json.dumps(value, separators=(",", ":")).encode()

    def test_scoped_decision_is_frozen_and_response_adapter_is_retained(self):
        class Config:
            optimize = True
            image_optimize = True
            mode = "cache"
        gate = self.gate(); view = control.ScopedConfig(Config(), gate)
        state = self.state(gate, kind="anthropic"); token = control._request.set(state)
        try:
            gate.enabled = False
            self.assertTrue(view.optimize)
            state = self.state(gate, enabled=False, kind="anthropic"); control._request.set(state)
            self.assertFalse(view.optimize); self.assertFalse(view.image_optimize)
            state.kind = "responses"
            self.assertTrue(view.optimize)
        finally: control._request.reset(token)

    def test_prefix_keeps_earlier_compressed_and_paused_raw_items(self):
        old = [{"type":"function_call_output","call_id":"a","output":"original old output"}]
        sent = [{"type":"function_call_output","call_id":"a","output":"compressed"}]
        paused = [*old,{"type":"message","role":"user","content":"paused raw tail"}]
        result = control._preserve(paused, paused, (old,sent), prefix)
        self.assertEqual(result,[*sent,paused[-1]])
        current=[*paused,{"type":"message","role":"user","content":"resume"}]
        recompressed=[*sent,{**paused[-1],"content":"changed by compressor"},current[-1]]
        resumed=control._preserve(current,recompressed,(paused,result),prefix)
        self.assertEqual(resumed[:2],result)
        different=[{**old[0],"output":"different conversation"},current[-1]]
        self.assertEqual(control._preserve(different,different,(old,sent),prefix),different)

    def test_lineage_does_not_mix_sibling_tails_and_prepared_bytes(self):
        gate=self.gate(); state=self.state(gate)
        common=[{"type":"message","role":"user","content":"shared"}]
        body={"model":"any-model","input":common}
        key,tracker,previous=gate.prepare(body,state); self.assertEqual(previous,([],[]))
        gate.record(tracker,common,[{**common[0],"content":"first compressed"}],None,state)
        a={**body,"input":[*common,{"type":"message","role":"user","content":"branch-a"}]}
        key,ta,pa=gate.prepare(a,state); outa=control._preserve(a[key],a[key],pa,prefix);gate.record(ta,a[key],outa,None,state)
        b={**body,"input":[*common,{"type":"message","role":"user","content":"branch-b"}]}
        key,tb,pb=gate.prepare(b,state);outb=control._preserve(b[key],b[key],pb,prefix)
        self.assertEqual(outb[0]["content"],"first compressed");self.assertEqual(outb[1]["content"],"branch-b")
        self.assertNotIn("branch-a",json.dumps(outb))

    def test_limits_disable_pause_and_block_unprovable_paused_request(self):
        gate=self.gate();state=self.state(gate)
        gate.prepare({"input":[{"content":"x"*control.MAX_PAYLOAD}]},state)
        self.assertFalse(gate.state()["can_pause"])
        with self.assertRaises(control.ReplayUnavailable):gate.prepare({"input":[]},self.state(gate,False))

    def test_private_control_cas_and_secret_never_in_response(self):
        gate=self.gate(); middleware=control.ControlMiddleware(None,gate=gate)
        async def request(body,secret=None):
            messages=[]; received=False
            async def receive(): return {"type":"http.request","body":json.dumps(body).encode(),"more_body":False}
            async def send(m):messages.append(m)
            await middleware.control({"method":"POST"},receive,send,{"x-headroom-desktop-control":secret or gate._secret})
            return messages[0]["status"],json.loads(messages[1]["body"])
        body={"schema_version":1,"request_id":str(uuid.uuid4()),"runtime_generation":gate.generation,"action":"set","expected_revision":0,"enabled":False}
        status,value=asyncio.run(request(body));self.assertEqual(status,200);self.assertFalse(value["enabled"]);self.assertEqual(value["revision"],1)
        status,value=asyncio.run(request(body));self.assertEqual(status,409);self.assertFalse(value["enabled"])
        self.assertNotIn(gate._secret,json.dumps(value))
        status,_=asyncio.run(request(body,"wrong"));self.assertEqual(status,404)
        wrong={**body,"runtime_generation":uuid.uuid4().hex};status,_=asyncio.run(request(wrong));self.assertEqual(status,400)

    def test_pause_keeps_inflight_snapshot_and_closes_new_requests(self):
        class Config:
            optimize = True
            image_optimize = True
        async def scenario():
            gate = self.gate(); view = control.ScopedConfig(Config(), gate)
            entered = asyncio.Event(); release = asyncio.Event(); seen = []; calls = 0
            async def app(scope, receive, send):
                nonlocal calls
                calls += 1
                if calls == 1:
                    seen.append(("old-before", view.optimize))
                    entered.set(); await release.wait()
                    seen.append(("old-after", view.optimize))
                else:
                    seen.append(("new", view.optimize))
                await send({"type": "http.response.start", "status": 200, "headers": []})
                await send({"type": "http.response.body", "body": b"ok"})
            middleware = control.ControlMiddleware(app, gate=gate)
            async def request():
                async def receive(): return {"type": "http.request", "body": b"", "more_body": False}
                async def send(_message): pass
                await middleware({"type": "http", "path": "/v1/messages", "method": "POST", "headers": []}, receive, send)

            old = asyncio.create_task(request())
            await entered.wait()
            status, result = await self.control_request(
                middleware, gate,
                self.control_body(gate, expected_revision=0, enabled=False))
            self.assertEqual(status, 200); self.assertFalse(result["enabled"])
            await request()
            release.set(); await old
            self.assertEqual(seen, [("old-before", True), ("new", False), ("old-after", True)])
            self.assertEqual(gate.state()["active_http_requests"], 0)
        asyncio.run(scenario())

    def test_pause_is_scoped_to_its_gate(self):
        async def scenario():
            first, second = self.gate(), self.gate()
            middleware = control.ControlMiddleware(None, gate=first)
            status, result = await self.control_request(
                middleware, first,
                self.control_body(first, expected_revision=0, enabled=False))
            self.assertEqual(status, 200); self.assertFalse(result["enabled"])
            self.assertFalse(first.enabled); self.assertTrue(second.enabled)
            self.assertEqual(first.revision, 1); self.assertEqual(second.revision, 0)
        asyncio.run(scenario())

    def test_private_control_rejects_malformed_and_reserved_routes(self):
        async def scenario():
            gate = self.gate(); middleware = control.ControlMiddleware(None, gate=gate)
            read = self.control_body(gate, action="read")
            set_body = self.control_body(gate, expected_revision=0, enabled=False)
            duplicate = read.replace(b'"action":"read"', b'"action":"read","action":"read"')
            wrong_action = self.control_body(gate, action="resume")
            wrong_generation = json.loads(read)
            wrong_generation["runtime_generation"] = uuid.uuid4().hex
            wrong_generation = json.dumps(wrong_generation, separators=(",", ":")).encode()
            bool_revision = json.loads(set_body)
            bool_revision["expected_revision"] = True
            bool_revision = json.dumps(bool_revision, separators=(",", ":")).encode()
            for raw in (duplicate, wrong_action, wrong_generation, bool_revision, b" " * 2049):
                with self.subTest(payload_size=len(raw)):
                    status, result = await self.control_request(middleware, gate, raw)
                    self.assertEqual(status, 400)
                    self.assertEqual(result, {"error": "invalid_input"})
                    self.assertTrue(gate.enabled); self.assertEqual(gate.revision, 0)

            messages = []
            async def receive(): return {"type": "websocket.connect"}
            async def send(message): messages.append(message)
            await middleware({"type": "websocket", "path": control.CONTROL_PATH,
                              "headers": []}, receive, send)
            self.assertEqual(messages, [{"type": "websocket.close", "code": 1008}])
            self.assertTrue(gate.enabled); self.assertEqual(gate.revision, 0)
        asyncio.run(scenario())

    def test_paused_replay_failure_returns_one_fixed_503_without_upstream_retry(self):
        async def scenario():
            gate = self.gate(); gate.enabled = False
            calls = {"middleware": 0, "upstream": 0}
            async def app(scope, receive, send):
                calls["middleware"] += 1
                state = control._request.get()
                self.assertFalse(state.enabled)
                state.replay_failed = True
                raise control.ReplayUnavailable()
            middleware = control.ControlMiddleware(app, gate=gate)
            messages = []
            async def receive(): return {"type": "http.request", "body": b"", "more_body": False}
            async def send(message): messages.append(message)
            await middleware({"type": "http", "path": "/v1/messages", "method": "POST",
                              "headers": []}, receive, send)
            self.assertEqual(calls, {"middleware": 1, "upstream": 0})
            self.assertEqual([m["status"] for m in messages if m["type"] == "http.response.start"], [503])
            self.assertEqual(json.loads(messages[-1]["body"]), {
                "error": {"type": "desktop_pause_unavailable",
                          "message": "Safe prefix replay is unavailable."}})
            self.assertEqual(gate.state()["active_http_requests"], 0)
        asyncio.run(scenario())

    def test_executor_timeout_disables_pause_and_cannot_fail_open_while_paused(self):
        import cache_control_shim
        import headroom.proxy.handlers.openai as openai
        import headroom.proxy.helpers as helpers
        import headroom.proxy.server as server
        async def timeout(*args, **kwargs): raise TimeoutError()
        attributes = [(server, "create_app"), (prefix.SessionTrackerStore, "resolve_tracker"),
                      (prefix, "normalize_message_cache_control"),
                      (helpers, "read_request_json_with_bytes"),
                      (openai, "_dedup_responses_output_items"), (openai, "_extract_responses_usage"),
                      (openai.OpenAIHandlerMixin, "_compress_openai_responses_payload"),
                      (openai.OpenAIHandlerMixin, "_compress_openai_responses_payload_in_executor")]
        with contextlib.ExitStack() as stack:
            for obj, name in attributes: stack.enter_context(patch.object(obj, name, getattr(obj, name)))
            openai.OpenAIHandlerMixin._compress_openai_responses_payload_in_executor = timeout
            control.install(uuid.uuid4().hex, uuid.uuid4().hex)
            async def scenario():
                for kind in ("responses", "responses_ws"):
                    for enabled in (True, False):
                        gate = self.gate(); gate.enabled = enabled
                        state = self.state(gate, enabled, kind)
                        token = control._request.set(state)
                        try:
                            with self.assertRaises(TimeoutError if enabled else control.ReplayUnavailable):
                                await openai.OpenAIHandlerMixin._compress_openai_responses_payload_in_executor(None, {})
                            self.assertFalse(gate.state()["can_pause"])
                            self.assertIs(control._request.get(), state)
                        finally: control._request.reset(token)
            asyncio.run(scenario())

    def test_replay_scope_uses_the_actual_route_and_first_auth_header(self):
        async def scenario():
            gate = self.gate(); keys = []
            async def app(scope, receive, send): keys.append(control._request.get().scope_key)
            middleware = control.ControlMiddleware(app, gate=gate)
            async def request(headers=(), path="/v1/responses"):
                async def receive(): return {"type": "http.request", "body": b"", "more_body": False}
                async def send(message): pass
                await middleware({"type": "http", "method": "POST", "path": path, "headers": headers}, receive, send)
                return keys[-1]
            first = await request([(b"authorization", b"Bearer route-a")])
            duplicate = await request([(b"authorization", b"Bearer route-a"), (b"authorization", b"Bearer route-b")])
            self.assertEqual(first, duplicate)
            other_auth = await request([(b"authorization", b"Bearer route-b")])
            base_a = await request([(b"x-headroom-base-url", b"https://route-a.invalid/v1")])
            base_b = await request([(b"x-headroom-base-url", b"https://route-b.invalid/v1")])
            path_a = await request([(b"x-headroom-original-path", b"/api-a/responses")])
            path_b = await request([(b"x-headroom-original-path", b"/api-b/responses")])
            other_path = await request(path="/p/other/v1/responses")
            self.assertEqual(len({first, other_auth, base_a, base_b, path_a, path_b, other_path}), 7)
        asyncio.run(scenario())


if __name__ == "__main__": unittest.main()
