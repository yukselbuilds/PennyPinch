"""Usage limits belong to SSE events, independent of transport chunking."""
from pathlib import Path
import runpy
import unittest


HERE = Path(__file__).resolve().parent
responses = runpy.run_path(str(HERE / "test_responses_usage.py"))
anthropic = runpy.run_path(str(HERE / "anthropic_usage_capture.py"))
chat = runpy.run_path(str(HERE / "test_chat_usage.py"))
control = responses["control"]
parse = responses["parser"].parse_sse_events_from_byte_buffer
limit = control.MAX_USAGE_EVENT_BYTES


def usage_cases():
    counts = anthropic["SSE_CASES"]["sse-a"]
    frames = anthropic["sse_frames"]("fixture", counts)
    event = responses["event"]
    response = responses["response"]
    return (
        ("anthropic", control._observe_anthropic_sse, frames[:2],
         lambda text: anthropic["sse_event"]("content_block_delta", index=0, delta={"type": "text_delta", "text": text}),
         frames[3:], dict(zip(control.USAGE_FIELDS, counts, strict=True))),
        ("responses", control._observe_responses_sse, [event("response.created", response=response(status="in_progress", usage=None))],
         lambda text: event("response.output_text.delta", delta=text).replace(b"\r\n", b"\n"),
         [event("response.completed", response=response()), b"data: [DONE]\n\n"],
         control._responses_usage(response())),
        ("chat", control._observe_chat_sse, [],
         lambda text: chat["frame"]([{"index": 0, "delta": {"content": text}}]),
         [chat["frame"]([], usage=chat["USAGE"]), chat["DONE"]], chat["COUNTERS"]),
    )


class SseUsageEventTests(unittest.IsolatedAsyncioTestCase):
    async def inspect(self, observer, parts):
        async def chunks():
            for part in parts:
                yield part
        attempt = {"usage": None}
        forwarded = [part async for part in observer(chunks(), attempt, parse)]
        self.assertTrue(forwarded == parts, "The observer changed the forwarded bytes or chunk order")
        return attempt

    async def test_coalesced_events_collect_the_same_usage_as_separate_chunks(self):
        for surface, observer, start, delta, end, expected in usage_cases():
            frame = delta("x" * (limit // 2))
            frames = [*start, frame, frame, *end]
            wire = b"".join(frames)
            self.assertLess(max(map(len, frames)), limit)
            self.assertGreater(len(wire), limit)
            for parts in (frames, [wire], [wire[:17], wire[17:]]):
                with self.subTest(surface=surface, chunks=len(parts)):
                    self.assertEqual((await self.inspect(observer, parts))["usage"], expected)

    async def test_split_frame_limit_accepts_exact_size_and_rejects_an_extra_byte(self):
        for surface, observer, start, delta, end, expected in usage_cases():
            for extra in (0, 1):
                with self.subTest(surface=surface, extra_bytes=extra):
                    frame = delta("x" * (limit - len(delta("")) + extra))
                    self.assertEqual(len(frame), limit + extra)
                    # Split inside the final LF or CRLF blank-line separator.
                    parts = [*start, frame[:-1], frame[-1:] + b"".join(end)]
                    attempt = await self.inspect(observer, parts)
                    self.assertEqual(attempt["usage"], None if extra else expected)

    async def test_responses_oversized_event_invalidates_a_previous_terminal(self):
        _, observer, start, delta, end, _ = usage_cases()[1]
        oversized = delta("x" * limit)
        attempt = await self.inspect(observer, [*start, end[0], oversized, end[1]])
        self.assertIsNone(attempt["usage"])
        self.assertNotIn("_verified_responses_terminal", attempt)


if __name__ == "__main__":
    unittest.main()
