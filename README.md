# Parda Bench

**Does your PII masking tool actually protect Indian personal data in an LLM app?** Parda Bench measures it.

Many teams put a masking layer in front of an LLM. It replaces values like Aadhaar numbers, PAN, phone numbers and names with placeholders such as `<PERSON_1>` before the prompt leaves the app. It then puts the real values back into the model's reply. Parda Bench tests both halves:

1. **Detection:** does the tool find the personal data? It runs the tool over about 3,000 labeled Indian texts: English, Hinglish, Devanagari and mixed script, plus noisy and OCR-style text. It also includes "hard negatives" that only *look* like personal data, such as a train PNR shaped like a mobile number. It reports precision, recall and F1 per entity type, and latency.
2. **Restore:** does the round trip work? It runs scripted conversations through the tool against a fake LLM and checks two things: did any real value reach the LLM, and did the app get the real values back? The scenarios cover streaming, tool calls, multi-turn chats, models that alter placeholders (`<person_1>`), JSON output and LLM errors. They run through real client libraries: the OpenAI SDK, the Anthropic SDK, LangChain, LiteLLM and plain HTTP.

The result is a report in Markdown and HTML that compares tools side by side.

> Results describe this dataset and these scenarios only. They are not a verdict on overall tool quality and not a compliance certification.
>
> The dataset is synthetic: texts are generated from templates or written by an LLM. The Hindi, Hinglish and mixed-script texts have not yet been reviewed by native speakers, and there are no human-written samples yet.

## Do I need to know Rust?

No. The `parda-bench` command is written in Rust, but you only build and run it. The tool you test runs as a **separate program in its own language**: Python, Node, Go, a Docker container, anything. The two talk over a simple JSON protocol. Python and Node helpers are included, so an adapter is usually 20 lines.

## Which situation are you in?

| You want to… | Go to |
|---|---|
| See how existing tools compare (Presidio, LangChain's reversible anonymizer, the built-in baselines) | [Try it in 5 minutes](#try-it-in-5-minutes) |
| Benchmark **your own** masking library or service | [Test your own tool](#test-your-own-tool) |
| Test a masking **proxy** your app sends OpenAI or Anthropic traffic through | [Proxies](#a-proxy-openai-or-anthropic-compatible) |
| Check masking used inside a **LangChain** (or LiteLLM) app | [LangChain and other frameworks](#langchain-and-other-frameworks) |

## Install

You need:

- **Linux or macOS.** Windows is not supported.
- **Rust**, installed with [rustup](https://rustup.rs). The right version is picked up automatically from `rust-toolchain.toml`.
- **[uv](https://docs.astral.sh/uv/getting-started/installation/)**, a fast Python package manager. It also installs Python if you don't have it.
- **Node.js 18+**, only if you write a Node adapter.

```sh
git clone https://github.com/rht-21/parda-bench.git
cd parda-bench
cargo build --release              # takes a few minutes the first time
alias pb=target/release/parda-bench
pb --help
```

## Try it in 5 minutes

The dataset is already in the repo (`data/v0.2.0`). Benchmark the built-in regex tool and Microsoft Presidio:

```sh
pb detect --tool tools/regex-baseline --dataset data/v0.2.0
pb detect --tool tools/presidio --dataset data/v0.2.0
```

The first Presidio run downloads its language model (about 400 MB), so it is slow once. Each run prints a one-line summary:

```
presidio: strict F1 43.2, relaxed F1 51.6, 0 failed; results in results/20261002T050645Z-presidio-default-detect
```

Then run the restore tests and build the report:

```sh
pb restore --tool tools/regex-baseline                   # a library tool
pb restore --tool tools/regex-proxy --driver langchain   # a proxy, tested through LangChain
pb report
open reports/report.html                                 # or xdg-open on Linux
```

The report shows:
- per-entity scores
- scores by language, difficulty and source
- how often each tool flagged the look-alikes
- a pass/fail matrix for every restore scenario
- the reason for each failure

## Tools included

| Tool | What it is | Detection | Restore suite |
|---|---|---|---|
| `tools/regex-baseline` | Reference: regular expressions and check digits, no ML | Yes | Library: mask, unmask, streaming |
| `tools/regex-proxy` | Reference: the same engine as an OpenAI/Anthropic-compatible proxy | No | Proxy, through every driver |
| `tools/presidio` | Microsoft Presidio with its default analyzer | Yes | Not applicable: it redacts but cannot restore |
| `tools/langchain-presidio` | LangChain's `PresidioReversibleAnonymizer` (`langchain-experimental`), used as a chain would | No: it has no detection API | Library: mask, unmask; no streaming |

With its default settings, `langchain-presidio` swaps values for realistic fakes, for example "Rahul Sharma" becomes "Jonathan Johnson", rather than placeholder tokens. Aadhaar, PAN and Devanagari names pass through unmasked.

## Test your own tool

Every tool lives in its own folder under `tools/` with a `tool.toml` file that tells Parda Bench how to start it and talk to it. Pick the row that matches your tool:

| Your tool is… | Transport | You write |
|---|---|---|
| A Python library | `stdio` | `tool.toml` and a small adapter using `sdk/python` |
| A JavaScript or TypeScript library | `stdio` | `tool.toml` and a small adapter using `sdk/node` |
| A library in any other language | `stdio` | `tool.toml` and a program that reads and writes JSON lines |
| An HTTP service with its own API | `http-api` | `tool.toml` and an endpoint that accepts the JSON protocol |
| An OpenAI or Anthropic-compatible proxy | `http-proxy` | `tool.toml` only |

### A Python library

Say your package has `find_pii(text)`, `mask(text)` and `unmask(text)`. Create `tools/my-tool/`:

`tools/my-tool/pyproject.toml`: your tool's own environment, separate from everything else.

```toml
[project]
name = "my-tool-adapter"
version = "0.1.0"
requires-python = ">=3.10"
dependencies = ["parda-sdk", "my-masking-package==1.2.0"]

[tool.uv]
package = false

[tool.uv.sources]
parda-sdk = { path = "../../sdk/python", editable = true }
```

`tools/my-tool/adapter.py`: implement only what your tool supports.

```python
from parda_sdk import Adapter, Entity, run
import my_masking_package as mp

class MyTool(Adapter):
    tool_version = "1.2.0"

    def __init__(self) -> None:
        self.sessions: dict[str, mp.Masker] = {}   # one masker per conversation

    def detect(self, text: str) -> list[Entity]:
        return [Entity(f.start, f.end, f.label, f.score) for f in mp.find_pii(text)]

    def mask(self, session: str, text: str) -> str:
        return self.sessions.setdefault(session, mp.Masker()).mask(text)

    def unmask(self, session: str, text: str) -> str:
        return self.sessions[session].unmask(text)

run(MyTool())
```

`tools/my-tool/tool.toml`:

```toml
name = "my-tool"
version = "1.2.0"
commit = "v1.2.0"
license = "MIT"
language = "python"
homepage = "https://github.com/you/my-masking-package"
capabilities = ["detect", "mask", "unmask"]     # must match the methods you implemented
transport = { kind = "stdio", command = ["uv", "run", "--quiet", "python", "adapter.py"] }

[entity_map]          # your labels -> Parda entity types
AADHAAR_NO = "AADHAAR"
PERSON = "PERSON_NAME"
MOBILE = "PHONE"

[configs.default]
```

Then run it:

```sh
pb detect --tool tools/my-tool --dataset data/v0.2.0 --limit 20    # quick smoke test
pb detect --tool tools/my-tool --dataset data/v0.2.0
pb restore --tool tools/my-tool
pb report
```

Good to know:
- **Offsets** are character positions in the text, which is what Python string indices already are.
- **`print()` is safe:** while the adapter runs, the SDK redirects it to stderr.
- **Debugging:** your tool's errors and log output go to `results/<run>/tool.log`.
- **Unmapped labels:** labels your `entity_map` doesn't cover are listed in the report, so you can see what is missing.
- **Detect-only tools:** if your tool can only detect or redact, leave out `unmask`. The restore tests are then reported as "not applicable".
- **Streaming:** to be tested on streamed replies, also implement `unmask_stream(session, chunk, is_final)`, which receives the reply chunk by chunk.

`tools/regex-baseline` is a complete working example.

### A JavaScript or TypeScript library

Use `sdk/node`. One rule: JavaScript string positions count UTF-16 units, so pass every span through `toCodePointSpan`:

```js
import { runAdapter, toCodePointSpan } from "../../sdk/node/index.mjs";
import { findPii } from "my-masking-package";

await runAdapter({
  toolVersion: "1.0.0",
  detect: (text) => findPii(text).map((f) => ({ ...toCodePointSpan(text, f.start, f.end), label: f.type })),
});
```

In `tool.toml`, set `language = "javascript"` and `transport = { kind = "stdio", command = ["node", "adapter.mjs"] }`.

### Any other language

Read one JSON request per line on stdin and write one JSON response per line on stdout:

```
→ {"id": 1, "op": "hello"}
← {"id": 1, "status": "ok", "elapsed_ns": 900, "result": {"op": "hello", "tool_version": "1.0", "capabilities": ["detect"]}}
→ {"id": 2, "op": "detect", "text": "PAN ABCPS1234K"}
← {"id": 2, "status": "ok", "elapsed_ns": 41000, "result": {"op": "detect", "entities": [{"start": 4, "end": 14, "label": "PAN", "score": 0.9}]}}
```

The exact message shapes are JSON Schemas in `spec/adapter-request.schema.json` and `spec/adapter-response.schema.json`.

### A proxy (OpenAI or Anthropic-compatible)

No adapter is needed. Describe how to start your proxy and which environment variable sets its upstream URL:

```toml
name = "my-proxy"
version = "2.0.0"
commit = "v2.0.0"
license = "Apache-2.0"
language = "go"
homepage = "https://github.com/you/my-proxy"
capabilities = ["proxy"]
transport = { kind = "http-proxy", start = ["./my-proxy", "--port", "8080"], base_url = "http://127.0.0.1:8080", upstream_env = "MY_PROXY_UPSTREAM" }

[entity_map]

[configs.default]
```

Following the official SDKs, the drivers call `{base_url}/chat/completions` for OpenAI-style traffic and `{base_url}/v1/messages` for Anthropic-style traffic, so your proxy should serve those paths.

Parda Bench starts the proxy, points its upstream at a fake LLM, and sends conversations through it with each client library:

```sh
for driver in raw_http openai_sdk anthropic_sdk langchain litellm; do
  pb restore --tool tools/my-proxy --driver $driver
done
```

`tools/regex-proxy` is a complete working example.

## LangChain and other frameworks

There are two different ways LangChain (or LiteLLM, or the OpenAI and Anthropic SDKs) can come into it:

**1. Your app uses LangChain, and masking happens in a proxy.** Use `--driver langchain`. Parda Bench then sends every scenario through a real LangChain `ChatOpenAI` or `ChatAnthropic`, including tool calling and streaming, exactly as your app would. Compare it with `--driver raw_http` to see whether a failure comes from the proxy or from how the client library handles it. Run `pb restore --control --driver langchain` to check the client library on its own, with no tool in between.

**2. Masking happens inside your LangChain chain**, for example with LangChain's `PresidioReversibleAnonymizer` or your own runnable that masks before the model call. Wrap the same masking object in a Python adapter, as above. The harness calls `mask` before the model and `unmask` after it, which is what your chain does. `tools/langchain-presidio` is a working example built on `PresidioReversibleAnonymizer`:

```sh
pb restore --tool tools/langchain-presidio
```

Available drivers: `raw_http` (built in), `openai_sdk`, `anthropic_sdk`, `langchain` and `litellm`, listed in `drivers/drivers.toml`. To add another framework, see [docs/restore-suite.md](docs/restore-suite.md#drivers).

## Reading the results

- **Strict vs relaxed F1.** A strict match needs exactly the right characters, for example `+91 98765 43210` including the `+91`. A relaxed match needs only an overlap with the right type.
- **Hard-negative rate.** This is how often the tool flagged a look-alike, such as an order number it took for an Aadhaar. Lower is better.
- **Source slice.** `template` samples are generated from patterns; `llm` samples are free-form texts. A tool that scores much worse on `llm` is probably tuned to the templates.
- **Restore failures.** Each failure names the scenario, the check and what went wrong, for example `no_leak_upstream: upstream received pii[0] "Ananya Iyer"`.
- **Where the numbers come from.** Every number traces back to files in `results/<run>/`. `pb report` re-scores them without rerunning any tool.

## Troubleshooting

| Problem | Fix |
|---|---|
| `uv: command not found` | Install uv (see [Install](#install)) and open a new terminal |
| The first run of a tool is slow | uv is building that tool's environment and downloading models; later runs are fast |
| `capabilities ... differ from manifest` | `capabilities` in `tool.toml` must list exactly the methods your adapter implements |
| A tool fails or prints nothing useful | Read `results/<run>/tool.log`, which holds the tool's own output |
| `did not accept connections` for a proxy | Check that `base_url` in `tool.toml` uses the port your proxy listens on |
| `a run with this id already exists` | Two runs of the same tool started within one second; run it again |

## Commands

| Command | Does |
|---|---|
| `pb detect --tool DIR --dataset DIR [--config NAME] [--limit N]` | Run a tool's detector over a dataset |
| `pb restore --tool DIR [--driver NAME]` | Run the restore scenarios against a tool |
| `pb restore --control --driver NAME` | Run the scenarios with no tool, to check a driver |
| `pb report` | Score stored runs and write `reports/report.md` and `report.html` |
| `pb data build` / `validate DIR` / `stats DIR` | Build, check or describe the dataset |
| `pb schema` | Regenerate the JSON Schemas in `spec/` |

## Learn more

- [docs/adding-a-tool.md](docs/adding-a-tool.md): the full `tool.toml` reference and tool configurations
- [docs/restore-suite.md](docs/restore-suite.md): how scenarios, checks and drivers work, and how to write new ones
- [docs/dataset.md](docs/dataset.md): what is in the dataset, its labeling rules and its versions
- [docs/architecture.md](docs/architecture.md): how the pieces fit together, for contributors

## Contributing

Run `git config core.hooksPath .githooks` once after cloning. A pre-commit hook then blocks commits with unformatted Rust code, the most common reason CI fails.

## License

Code: Apache-2.0 (see `LICENSE`). Dataset: CC BY 4.0 (to be confirmed before release).
