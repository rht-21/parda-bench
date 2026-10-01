# Adding a tool

A tool lives in `tools/<name>/`, with its own isolated environment and a `tool.toml` manifest (schema `spec/tool-manifest.schema.json`). The harness never imports tool code; it talks to the tool over one of three transports.

## 1. Write the manifest

```toml
name = "my-tool"
version = "1.4.0"                 # the tool's own version
commit = "1.4.0"                  # git tag or commit benchmarked
license = "MIT"
language = "python"
homepage = "https://example.org/my-tool"
capabilities = ["detect", "mask", "unmask", "unmask_stream"]
transport = { kind = "stdio", command = ["uv", "run", "--quiet", "--frozen", "python", "adapter.py"] }

[entity_map]                      # the tool's labels -> Parda entity types
AADHAAR_NUMBER = "AADHAAR"
PERSON = "PERSON_NAME"

[configs.default]                 # required: the tool as it ships
[configs.tuned]                   # optional: only a configuration the tool's own docs recommend
file = "tuned.yaml"               # absolute path passed in PARDA_TOOL_CONFIG
env = { MY_TOOL_MODE = "accurate" }
```

Rules the harness enforces:
- `configs.default` exists.
- The `proxy` capability goes with, and only with, the `http-proxy` transport.
- `unmask_stream` requires `mask`.
- The capabilities the running tool reports in `hello` equal the manifest's.

Map a label only when it means the same thing as the Parda type. Unmapped labels are reported per label, with how often they overlapped a gold span, so a missing mapping is visible rather than silently scored as noise. Where a mapping is approximate (for example a generic `LOCATION` to `ADDRESS`), say so in a comment, as `tools/presidio/tool.toml` does.

## 2. Write the adapter

**Python.** Subclass `parda_sdk.Adapter`, override what the tool supports, and call `run`. Capabilities are derived from the overrides. While the loop runs, `print` goes to stderr, so a chatty library cannot corrupt the protocol.

```python
from parda_sdk import Adapter, Entity, run

class MyTool(Adapter):
    tool_version = "1.4.0"
    def detect(self, text: str) -> list[Entity]:
        return [Entity(start, end, label, score) for ...]   # str indices

run(MyTool())
```

Add the SDK as a path dependency in the tool's `pyproject.toml`:

```toml
[tool.uv.sources]
parda-sdk = { path = "../../sdk/python", editable = true }
```

**Node.** `import { runAdapter, toCodePointSpan } from "../../sdk/node/index.mjs"`, implement `detect`, `mask`, `unmask` and `unmaskStream` as needed, and convert every span with `toCodePointSpan(text, start, end)`, because JavaScript indices count UTF-16 units.

**Any other language.** Read one JSON request per line from stdin and write one JSON response per line to stdout, following `spec/adapter-request.schema.json` and `spec/adapter-response.schema.json`. Measure `elapsed_ns` around the tool call only.

**Proxies** need no adapter: the harness starts `start`, sets `upstream_env` to the mock's URL, and sends OpenAI chat-completions and Anthropic messages traffic to `base_url`. Pass upstream HTTP errors through to the client unchanged.

**HTTP services** (`http-api`): POST each adapter request as JSON to `base_url` and return the adapter response as the body.

## 3. Run it

```sh
parda-bench detect --tool tools/my-tool --dataset data/v0.1.0 --limit 50   # smoke test
parda-bench detect --tool tools/my-tool --dataset data/v0.1.0
parda-bench restore --tool tools/my-tool                                    # library tool
parda-bench restore --tool tools/my-tool --driver openai_sdk                # proxy, once per driver
parda-bench report
```

The tool's stderr is in `results/<run-id>/tool.log`. Pin dependencies with a lock file (`uv.lock`) and run with `--frozen`, so a run can be reproduced.
