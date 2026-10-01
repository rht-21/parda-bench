# Parda Bench

A benchmark and test harness for Indian personal-data masking in LLM pipelines.

- **Detection benchmark:** a synthetic, labeled dataset of Indian-context text (Aadhaar, PAN, GSTIN, UPI, phone numbers, names, addresses, Hinglish, Devanagari, noisy text, and hard negatives that only look like personal data). Any masking tool can be scored on precision, recall and latency per entity type.
- **Restore test suite:** deterministic round-trip tests against a mock LLM server. They cover streaming (placeholders split across chunks), tool calls, multi-turn conversations, models that alter placeholders, Devanagari text, JSON output and upstream failures. The tests run through real client stacks: raw HTTP, the OpenAI and Anthropic SDKs, LangChain and LiteLLM.

Results describe this dataset and these scenarios only. They are not a verdict on overall tool quality and not a compliance certification.

## Requirements

- Rust (the version is pinned in `rust-toolchain.toml`; `rustup` installs it).
- [uv](https://docs.astral.sh/uv/) and Python 3.10 or newer, for the tools and drivers in this repo. Each one keeps its own environment, created on first use.
- Linux or macOS. Tool processes are managed as Unix process groups.
- Node.js 18 or newer, only for the Node adapter SDK.

## Quick start

```sh
cargo build --release
alias pb=target/release/parda-bench

pb data build                                   # generate data/v0.1.0 (a no-op if it already holds the same samples)
pb data validate data/v0.1.0
pb detect --tool tools/regex-baseline --dataset data/v0.1.0
pb detect --tool tools/presidio --dataset data/v0.1.0   # first run downloads spaCy's en_core_web_lg

pb restore --tool tools/regex-baseline                  # library tool: the harness calls mask/unmask
pb restore --tool tools/regex-proxy --driver openai_sdk # proxy tool: traffic goes through a real client
pb restore --control --driver langchain                 # no tool: checks the driver itself

pb report                                       # reports/report.md and reports/report.html
```

Every run writes `results/<run-id>/` (`meta.json`, the records, the tool's log). `pb report` rescores the newest run of each tool, configuration and driver from those files, so changing the scoring never needs a rerun.

## Commands

| Command | Does |
|---|---|
| `schema` | Regenerate `spec/*.schema.json` from the Rust types |
| `data build` / `validate` / `stats` | Build the dataset from the templates; check digests, spans and check digits; print its composition |
| `detect` | Run a tool's detector over a dataset (`--limit N` for a quick check) |
| `restore` | Run the restore scenarios against a tool, or against no tool with `--control` |
| `report` | Score the stored runs and write the Markdown and HTML report |

## Tools included

| Tool | Kind | Notes |
|---|---|---|
| `tools/regex-baseline` | Library (detect, mask, unmask, unmask_stream) | Reference: regular expressions and check digits, no ML |
| `tools/regex-proxy` | OpenAI/Anthropic-compatible proxy | Reference: the same engine as a proxy |
| `tools/presidio` | Detect and redact | Microsoft Presidio with its default analyzer |

To benchmark another tool, see [docs/adding-a-tool.md](docs/adding-a-tool.md).

## Layout

| Path | Purpose |
|---|---|
| `crates/parda-spec` | Data contracts (dataset, manifests, wire protocols, scenarios, results) |
| `crates/parda-data` | Identifier generators and validators, templates, seeded dataset build, validation |
| `crates/parda-adapters` | Transports to tools: stdio NDJSON workers, HTTP proxies, HTTP APIs |
| `crates/parda-eval` | Span matching, per-entity metrics, latency |
| `crates/parda-restore` | Mock upstream, scenarios, restore checks, drivers, runner |
| `crates/parda-report` | Markdown/HTML report from stored results |
| `crates/parda-cli` | `parda-bench` binary |
| `spec/` | Generated JSON Schemas: the contract for adapters and drivers in other languages |
| `tools/` | One isolated environment per benchmarked tool |
| `drivers/` | Client-framework drivers (`drivers.toml` lists them) |
| `sdk/` | Adapter helpers for Python and Node |
| `scenarios/` | Restore test cases (TOML) |
| `data/` | Generated, versioned dataset |
| `results/`, `reports/` | Run outputs and generated reports |

Further reading: [architecture](docs/architecture.md), [dataset](docs/dataset.md), [restore suite](docs/restore-suite.md).

## License

Code: Apache-2.0 (see `LICENSE`). Dataset: CC BY 4.0 (to be confirmed before release).
