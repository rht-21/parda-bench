import io
import json
import sys
import unittest
from typing import Any

sys.path.insert(0, __file__.rsplit("/tests/", 1)[0] + "/src")

from parda_sdk import Adapter, Entity, run  # noqa: E402


class Upper(Adapter):
    tool_version = "1.2"

    def detect(self, text: str) -> list[Entity]:
        if text == "boom":
            raise ValueError("cannot parse")
        print("library noise that must not reach the protocol stream")
        return [Entity(0, len(text), "WORD", 0.5)]


def exchange(adapter: Adapter, *requests: dict[str, Any]) -> list[dict[str, Any]]:
    stdin = io.StringIO("".join(json.dumps(r) + "\n" for r in requests))
    stdout = io.StringIO()
    run(adapter, stdin, stdout)
    return [json.loads(line) for line in stdout.getvalue().splitlines()]


class RunTest(unittest.TestCase):
    def test_hello_reports_only_overridden_capabilities(self) -> None:
        [reply] = exchange(Upper(), {"id": 1, "op": "hello"})
        self.assertEqual(reply["result"], {"op": "hello", "tool_version": "1.2", "capabilities": ["detect"]})

    def test_detect_counts_offsets_in_code_points(self) -> None:
        [reply] = exchange(Upper(), {"id": 7, "op": "detect", "text": "नमस्ते"})
        self.assertEqual(reply["id"], 7)
        self.assertEqual(reply["status"], "ok")
        self.assertEqual(reply["result"]["entities"], [{"start": 0, "end": 6, "label": "WORD", "score": 0.5}])
        self.assertIsInstance(reply["elapsed_ns"], int)

    def test_tool_exception_becomes_error_reply_and_loop_continues(self) -> None:
        replies = exchange(Upper(), {"id": 1, "op": "detect", "text": "boom"}, {"id": 2, "op": "hello"})
        self.assertEqual(replies[0], {"id": 1, "status": "error", "message": "ValueError: cannot parse"})
        self.assertEqual(replies[1]["status"], "ok")

    def test_unsupported_op_is_an_error(self) -> None:
        [reply] = exchange(Upper(), {"id": 3, "op": "mask", "session": "s", "text": "x"})
        self.assertEqual(reply["status"], "error")
        self.assertIn("NotImplementedError", reply["message"])

    def test_shutdown_replies_then_stops(self) -> None:
        replies = exchange(Upper(), {"id": 1, "op": "shutdown"}, {"id": 2, "op": "hello"})
        self.assertEqual(replies, [{"id": 1, "status": "ok", "elapsed_ns": replies[0]["elapsed_ns"], "result": {"op": "shutdown"}}])


if __name__ == "__main__":
    unittest.main()
