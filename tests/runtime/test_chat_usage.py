"""Regression checks for provider counters and Chat Completions stream boundaries."""
import json
import runpy
import unittest
from pathlib import Path


helpers = runpy.run_path(str(Path(__file__).with_name("test_responses_usage.py")))
control = helpers["control"]
parse_events = helpers["parser"].parse_sse_events_from_byte_buffer
COUNTERS = {"input_tokens": 1000, "cache_read_input_tokens": 900,
            "cache_creation_input_tokens": None, "output_tokens": 6}
USAGE = {"prompt_tokens": 1000, "completion_tokens": 6, "total_tokens": 1006,
         "prompt_tokens_details": {"cached_tokens": 900}}
DONE = b"data: [DONE]\r\n\r\n"


def frame(choices, *, usage=None, identity="chatcmpl-fixture"):
    return ("data: " + json.dumps({"object": "chat.completion.chunk", "choices": choices,
                                     "id": identity, "usage": usage}, ensure_ascii=False) + "\r\n\r\n").encode()


class RealChatInferenceSmokeTests(unittest.TestCase):
    def test_usage_requires_complete_answer_and_keeps_absent_cache_unknown(self):
        root = Path(__file__).resolve().parents[2]
        inspect = runpy.run_path(str(root / "bench/optimization/chat_real_inference.py"))["inspect_stream"]
        answer = frame([{"delta": {"content": '{"answer":4}'}, "finish_reason": "stop"}])
        usage = frame([], usage={"prompt_tokens": 30, "completion_tokens": 6, "total_tokens": 36})
        row = inspect(answer + usage + DONE)
        self.assertEqual(row, {"input_tokens": 30, "output_tokens": 6,
                               "cache_read_input_tokens": None, "cache_creation_input_tokens": None})
        for incomplete in (answer + usage, answer + DONE, answer + usage + usage + DONE):
            with self.assertRaises(AssertionError):
                inspect(incomplete)


class ChatUsageTests(unittest.IsolatedAsyncioTestCase):
    def test_cache_is_inclusive_and_missing_counts_stay_unknown(self):
        result = control._chat_usage({"object": "chat.completion", "usage": USAGE})
        self.assertEqual(result, COUNTERS)
        trace = control.UsageTrace(surface="openai_chat_completions")
        trace.begin_attempt().update(status=200, usage=result)
        trace.begin_attempt().update(status=200, usage=result)
        self.assertEqual(trace.snapshot(200)["usage"], {
            "input_tokens": 2000, "cache_read_input_tokens": 1800,
            "cache_creation_input_tokens": None, "output_tokens": 12,
            "total_input_tokens": 2000, "total_tokens": 2012,
        })
        trace.attempts[0]["status"] = 429
        self.assertEqual(set(trace.snapshot(200)["usage"].values()), {None})

    def test_zero_invalid_and_conflicting_counters(self):
        parse = lambda usage: control._chat_usage({"object": "chat.completion", "usage": usage})
        self.assertEqual(set(parse({"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0,
                                    "prompt_tokens_details": {"cached_tokens": 0, "cache_write_tokens": 0}}).values()), {0})
        for invalid in (True, -1, 1.5, "1", 2**53):
            self.assertIsNone(parse({"prompt_tokens": invalid})["input_tokens"])
        self.assertIsNone(parse({**USAGE, "total_tokens": 1007}))
        self.assertIsNone(parse({**USAGE, "prompt_tokens_details": {"cached_tokens": 900, "cache_write_tokens": 101}}))
        self.assertIsNone(parse(None))
        self.assertIsNone(control._chat_usage({"object": "response", "usage": USAGE}))

    async def inspect(self, raw, size=71):
        async def chunks():
            for offset in range(0, len(raw), size):
                yield raw[offset:offset + size]

        attempt = {"usage": None}
        observer = control._observe_chat_sse(chunks(), attempt, parse_events)
        self.assertEqual(b"".join([chunk async for chunk in observer]), raw)
        return attempt["usage"]

    async def test_usage_does_not_change_utf8_stream_chunks(self):
        raw = b": keepalive\r\n\r\n" + frame([{"delta": {"content": "sonuç λ"}}]) + frame([], usage=USAGE) + DONE
        for size in (1, 71, len(raw)):
            self.assertEqual(await self.inspect(raw, size), COUNTERS)

    async def test_incomplete_mixed_and_oversized_streams_remain_unknown(self):
        start, usage = frame([{"delta": {"content": "result"}}]), frame([], usage=USAGE)
        invalid = (start + DONE, start + usage, start + usage + DONE[:-1],
                   start + frame([], usage=USAGE, identity="another") + DONE,
                   start + usage + usage + DONE, start + usage + DONE + start,
                   start + usage + b'event: error\ndata: {"error":{}}\n\n' + DONE,
                   frame([{"delta": {}}], usage=USAGE) + DONE)
        for raw in invalid:
            self.assertIsNone(await self.inspect(raw))
        self.assertIsNone(await self.inspect(b"x" * (control.MAX_USAGE_EVENT_BYTES + 1)))
