// NDJSON adapter loop (schemas spec/adapter-request.schema.json and spec/adapter-response.schema.json).
//
// The protocol counts offsets in Unicode code points. JavaScript string indices count UTF-16 code units, so
// spans from JS libraries must go through `toCodePointSpan` before they are returned from `detect`.

import { createInterface } from "node:readline";

const OPS = ["detect", "mask", "unmask", "unmask_stream"];

/** Converts a UTF-16 `[start, end)` range of `text` to code-point offsets. */
export function toCodePointSpan(text, start, end) {
  const count = (s) => {
    let n = 0;
    for (const _ of s) n += 1;
    return n;
  };
  const before = count(text.slice(0, start));
  return { start: before, end: before + count(text.slice(start, end)) };
}

/** Operations the adapter object implements, in protocol names. */
export function capabilities(adapter) {
  const method = { detect: "detect", mask: "mask", unmask: "unmask", unmask_stream: "unmaskStream" };
  return OPS.filter((op) => typeof adapter[method[op]] === "function");
}

/** Handles one request object and returns `[response, isShutdown]`. */
export async function handle(adapter, request) {
  const { id, op } = request;
  const started = process.hrtime.bigint();
  let result;
  try {
    switch (op) {
      case "hello":
        result = { tool_version: adapter.toolVersion ?? "unknown", capabilities: capabilities(adapter) };
        break;
      case "detect":
        result = { entities: (await call(adapter, "detect", request.text)).map(entity) };
        break;
      case "mask":
        result = { text: await call(adapter, "mask", request.session, request.text) };
        break;
      case "unmask":
        result = { text: await call(adapter, "unmask", request.session, request.text) };
        break;
      case "unmask_stream":
        result = { text: await call(adapter, "unmaskStream", request.session, request.chunk, request.is_final) };
        break;
      case "shutdown":
        result = {};
        break;
      default:
        return [{ id, status: "error", message: `unknown op ${JSON.stringify(op)}` }, false];
    }
  } catch (e) {
    return [{ id, status: "error", message: `${e?.name ?? "Error"}: ${e?.message ?? e}` }, false];
  }
  const elapsed_ns = Number(process.hrtime.bigint() - started);
  return [{ id, status: "ok", elapsed_ns, result: { op, ...result } }, op === "shutdown"];
}

async function call(adapter, method, ...args) {
  if (typeof adapter[method] !== "function") {
    throw new Error(`tool does not implement ${method}`);
  }
  return adapter[method](...args);
}

function entity(e) {
  return { start: e.start, end: e.end, label: e.label, score: e.score ?? null };
}

/** Serves requests on stdin until `shutdown`. `console.log` goes to stderr so it cannot corrupt the protocol. */
export async function runAdapter(adapter) {
  const write = process.stdout.write.bind(process.stdout);
  console.log = console.error;
  const lines = createInterface({ input: process.stdin, crlfDelay: Infinity });
  for await (const line of lines) {
    if (!line.trim()) continue;
    let request;
    try {
      request = JSON.parse(line);
    } catch (e) {
      write(JSON.stringify({ id: 0, status: "error", message: `malformed request: ${e.message}` }) + "\n");
      continue;
    }
    const [response, done] = await handle(adapter, request);
    write(JSON.stringify(response) + "\n");
    if (done) break;
  }
  lines.close();
}
