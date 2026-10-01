import json
import sys
import unittest
from pathlib import Path

root = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(root))
sys.path.insert(0, str(root.parent / "regex-baseline"))

import proxy  # noqa: E402


class ProxyTest(unittest.TestCase):
    def setUp(self) -> None:
        proxy.vault = proxy.Vault()

    def test_masks_message_text_but_not_protocol_fields(self) -> None:
        body = {"model": "m", "messages": [{"role": "user", "content": "PAN ABCPS1234K"}]}
        masked = proxy.mask_request(body)
        self.assertEqual(masked["messages"][0], {"role": "user", "content": "PAN [PAN_1]"})

    def test_restores_split_placeholder_in_openai_stream(self) -> None:
        proxy.mask_request({"messages": [{"role": "user", "content": "PAN ABCPS1234K"}]})
        r = proxy.StreamRestorer(anthropic=False)
        texts = []
        for content, finish in (("Your [PA", None), ("N_1]", None), (None, "stop")):
            delta = {"content": content} if content is not None else {}
            for e in r.events({"choices": [{"index": 0, "delta": delta, "finish_reason": finish}]}):
                texts.append(e["choices"][0]["delta"].get("content", ""))
        self.assertEqual("".join(texts), "Your ABCPS1234K")

    def test_flushes_held_text_before_anthropic_block_stop(self) -> None:
        r = proxy.StreamRestorer(anthropic=True)
        first = r.events({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "a [b"}})
        stop = r.events({"type": "content_block_stop", "index": 0})
        self.assertEqual(first[0]["delta"]["text"], "a ")
        self.assertEqual([e["type"] for e in stop], ["content_block_delta", "content_block_stop"])
        self.assertEqual(stop[0]["delta"]["text"], "[b")

    def test_parses_sse_lines(self) -> None:
        lines = [b"event: x\n", b'data: {"a": 1}\n', b"\n", b"data: [DONE]\n", b"\n"]
        self.assertEqual(list(proxy.sse_events(iter(lines))), [("x", json.dumps({"a": 1})), (None, "[DONE]")])


if __name__ == "__main__":
    unittest.main()
