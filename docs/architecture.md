# Architecture

## Principles

1. **Everything external runs out of process.** Benchmarked tools and LLM client frameworks are separate processes behind language-neutral protocols, so the Rust core never links tool code. This also gives each tool its own isolated environment.
2. **The Rust types are the contract.** `crates/parda-spec` defines every data shape. `parda-bench schema` generates `spec/*.schema.json` from those types, and CI fails if the checked-in schemas drift. Authors working in other languages only need `spec/`. Authored files (samples, scenarios, `tool.toml`) reject unknown fields, so a typo is an error, not a silent default.
3. **Results are files.** Every run writes immutable files under `results/<run-id>/`. Reports are built only from those files, so every published number can be traced back to one.

## Crate dependencies

```
parda-spec  <-  parda-data, parda-eval, parda-adapters  <-  parda-restore  <-  parda-report  <-  parda-cli
```

- `parda-spec` has no I/O.
- Process, network and file I/O live in `parda-adapters`, `parda-restore`'s mock server and drivers, `parda-data::io`, `parda-report::runs` and `parda-cli`.
- `parda-cli` is the only composition root, and the only crate that uses `anyhow`. The library crates use `thiserror`.

## Text offsets

All spans count **Unicode scalar values**, end exclusive. That is a Rust `char` index or a Python `str` index; it is not bytes and not UTF-16 units. Node adapters must convert JavaScript string indices before replying (`toCodePointSpan` in `sdk/node`). This rule matters for Devanagari text.

## Tool transports (`tools/<name>/tool.toml`, schema `spec/tool-manifest.schema.json`)

| Kind | Used for | Mechanism |
|---|---|---|
| `stdio` | Library tools, any language | Long-lived worker: one JSON object per line. Requests follow `adapter-request`, responses follow `adapter-response` |
| `http-proxy` | OpenAI/Anthropic-compatible proxies | The harness starts the tool with `upstream_env` set to the mock upstream URL, then sends traffic through `base_url` |
| `http-api` | Tools with their own HTTP service | Each `adapter-request` is POSTed as JSON to `base_url`; the response body is the `adapter-response` |

Adapter ops are `hello`, `detect`, `mask`, `unmask`, `unmask_stream` and `shutdown`. The harness starts every tool with a `hello` handshake and fails if the capabilities the worker reports differ from the manifest. Each reply carries an `elapsed_ns` measured inside the tool. The harness also records wall time, so latency can be reported both with and without IPC overhead.

A tool's named configuration (`configs.<name>`) supplies environment variables, and `PARDA_TOOL_CONFIG` holds the absolute path of its `file` when it has one. Commands run with the tool directory as working directory. Each tool runs in its own process group, and the whole group is killed when the harness is done with it, because launchers such as `uv run` keep the real tool as a child process.

`entity_map` translates a tool's labels into Parda entity types. Labels with no mapping are kept and reported as unmapped; they are never dropped.

During detection, a tool-reported error becomes a `failed` record. A transport failure (crash, timeout, garbled reply) also restarts the worker, so one bad input cannot sink the rest of the run.

## Detection scoring

`parda-eval` matches predicted spans one-to-one against gold spans of the same entity type:

- **Strict:** identical boundaries.
- **Relaxed:** any overlap; pairs with the largest overlap are matched first.

The report gives per-entity precision, recall and F1, micro-averaged totals, slices by language and difficulty, the hard-negative rate (decoy texts where the decoy's own type was flagged), unmapped labels, and p50/p95/p99 latency. A failed sample counts all its gold spans as misses. Scoring happens at report time against the dataset, whose SHA-256 is checked against the one recorded in the run.

## Restore suite

**Scenarios.** Each scenario is a TOML file under `scenarios/<category>/`, following schema `scenario`. Real values live in `pii`, and both input and reply segments refer to them by `slot` index. See [restore-suite.md](restore-suite.md).

**Mock upstream.** It works out which placeholder the tool chose for each slot by aligning the masked request it receives against the scenario's literal input segments. It never assumes a placeholder format. It then replies with the script, which may include:
- placeholder fragments, through `from`/`to` char ranges
- placeholder mangling, through `mangle`
- exact SSE chunk boundaries (one scripted chunk is one delta event)
- an HTTP error

The mock records every request it receives, so the restore suite runs two kinds of check:
- `no_leak_upstream`: the upstream never received an original value.
- restore checks: the client got the original values back.

**Drivers** are stdio workers too. They follow `driver-request` and `driver-response`, receive turns that contain the real values, and report what the application saw. `raw_http` is native Rust; the OpenAI SDK, Anthropic SDK, LangChain and LiteLLM drivers are in `drivers/python` and listed in `drivers/drivers.toml`.

**Test matrix:**
- **Proxy tools:** every driver × every scenario. A driver that does not speak a scenario's API marks it `not_applicable`.
- **Library tools:** the harness calls `mask`, plays the upstream in process, then calls `unmask` or `unmask_stream`. It never adds its own buffering. A tool without `unmask_stream` gets `not_applicable` for streaming scenarios, and failure-behaviour scenarios do not apply to library tools.
- **Redact-only tools:** the whole suite is `not_applicable`.
- **Control runs** (`--control`): every driver straight against the mock with no tool. Only `no_leak_upstream` should fail. Anything else points at the driver or its client library, and the report lists it under driver checks.

## Results layout

```
results/<run-id>/
  meta.json          # run-meta: tool, config, hardware, suite (dataset digest or driver)
  detections.jsonl   # detection-record per sample, or
  restore.jsonl      # restore-record per scenario
  tool.log           # the tool's stderr (and a service's stdout)
```

Records are appended and flushed one at a time, so an interrupted run keeps what it finished. `parda-bench report` uses the newest run for each (suite, tool, configuration, driver or dataset).

## Build order

All six steps are implemented:

1. `parda-spec` contracts and schema generation
2. `parda-data`: identifier generators and validators, templates (including the Devanagari slice), seeded build, `validate`, `stats`
3. `parda-adapters`: stdio and HTTP transports, `sdk/python`, `sdk/node`, and the Presidio adapter
4. `parda-eval`: matching, metrics, latency
5. `parda-restore`: mock upstream, scenarios, checks, `raw_http` driver, the Python drivers
6. `parda-report` and the full `parda-bench` CLI
