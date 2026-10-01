import assert from "node:assert/strict";
import { test } from "node:test";

import { capabilities, handle, toCodePointSpan } from "../index.mjs";

test("code-point span skips the extra unit of an astral character", () => {
  const text = "😀 PAN ABCPS1234K";
  const start = text.indexOf("ABCPS1234K");
  assert.deepEqual(toCodePointSpan(text, start, start + 10), { start: 6, end: 16 });
});

test("Devanagari offsets are unchanged", () => {
  assert.deepEqual(toCodePointSpan("मेरा नाम राहुल", 9, 14), { start: 9, end: 14 });
});

test("hello lists implemented operations", async () => {
  const adapter = { toolVersion: "2.0", detect: () => [], unmaskStream: () => "" };
  assert.deepEqual(capabilities(adapter), ["detect", "unmask_stream"]);
  const [reply] = await handle(adapter, { id: 1, op: "hello" });
  assert.deepEqual(reply.result, { op: "hello", tool_version: "2.0", capabilities: ["detect", "unmask_stream"] });
});

test("detect reply carries entities and elapsed time", async () => {
  const adapter = { detect: async () => [{ start: 0, end: 3, label: "X" }] };
  const [reply, done] = await handle(adapter, { id: 4, op: "detect", text: "abc" });
  assert.equal(done, false);
  assert.equal(reply.status, "ok");
  assert.deepEqual(reply.result.entities, [{ start: 0, end: 3, label: "X", score: null }]);
  assert.equal(typeof reply.elapsed_ns, "number");
});

test("a throwing tool yields an error reply", async () => {
  const adapter = { mask: () => { throw new TypeError("bad input"); } };
  const [reply] = await handle(adapter, { id: 2, op: "mask", session: "s", text: "x" });
  assert.deepEqual(reply, { id: 2, status: "error", message: "TypeError: bad input" });
});

test("shutdown ends the loop", async () => {
  const [, done] = await handle({}, { id: 9, op: "shutdown" });
  assert.equal(done, true);
});
