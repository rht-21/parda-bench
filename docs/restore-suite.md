# Restore suite

A restore scenario sends real personal data through a tool to a scripted mock LLM. It then checks two things: what reached the upstream, and what the application got back.

## Scenario format

`scenarios/<category>/<id>.toml`; the file name is the id and the directory is the category (schema `spec/scenario.schema.json`).

```toml
id = "split-openai-two-chunks"
category = "streaming_split"
description = "A placeholder split across two SSE chunks after its third character."
api = "openai_chat"            # or anthropic_messages
stream = true
pii = ["ABCPS1234K"]           # real values; segments refer to them by index
checks = ["no_leak_upstream", "restored_exact", "no_placeholder_fragment"]

[[turns]]
input = { kind = "user", message = [{ text = "My PAN is " }, { slot = 0 }, { text = "." }] }
reply = { kind = "text", chunks = [[{ text = "Your PAN is " }, { slot = 0, to = 3 }], [{ slot = 0, from = 3 }, { text = "." }]] }
```

- **Inputs** are `user` messages or `tool_result`s (with the `call_id` of the previous turn's tool call). The driver sends them with the real values filled in.
- **Replies** are `text` (one inner list per streamed chunk), `tool_call` (`call_id`, `name`, `argument_chunks`; the tool must be declared in `tools`), or `http_error` (`status`, `body`).
- A reply slot renders the placeholder the tool chose for that value, optionally cut to the character range `from..to` (clamped to the placeholder's length) or altered by `mangle`: `lowercase`, `uppercase`, `strip_delimiters` (`<PERSON_1>` becomes `PERSON_1`) or `space_padded` (`< PERSON_1 >`).

**Placeholder discovery.** The mock finds each slot's placeholder by anchoring the turn's literal input segments in the masked text it receives. So:

- Two slots need literal text between them, even a single space.
- A tool that alters the literal text makes the turn fail with a mock error. That counts as a `restored_exact` failure.
- A value that reaches the upstream unmasked is its own "placeholder", so the leak check catches it and restoring it is trivially exact.

A reply may only use slots that appeared in an input in this turn or an earlier one; `scenarios::check` rejects the scenario otherwise.

## Checks

| Check | Passes when |
|---|---|
| `no_leak_upstream` | No `pii` value appears in any request the upstream received, verbatim or with separators and case removed (values of 6+ letters and digits) |
| `restored_exact` | Each turn the client saw equals the script with real values, once per value (fragments of a split placeholder yield the value once). Tool-call arguments compare as JSON |
| `no_placeholder_fragment` | The client saw no placeholder, its alphanumeric core, or either half of it, unless the expected text itself contains that string |
| `placeholder_stable_across_turns` | Every occurrence of a value, in one message or across turns, got the same placeholder |
| `json_valid` | Tool-call arguments, and text replies that are JSON as scripted, still parse after restoring |
| `fails_closed` | An upstream error reaches the client as an error with the same status, not as a made-up completion |

`fails_closed` goes with, and only with, an `http_error` reply.

## Categories

`basic`, `streaming_split`, `tool_calls`, `multi_turn`, `placeholder_mangling`, `repeats_adjacency`, `unicode`, `structured_output`, `failure_behaviour`.

## Drivers

| Driver | Speaks | Notes |
|---|---|---|
| `raw_http` | both APIs | Built in; plain HTTP and SSE parsing, no client library |
| `openai_sdk` | OpenAI | `openai` Python SDK |
| `anthropic_sdk` | Anthropic | `anthropic` Python SDK; streaming via `messages.stream` |
| `langchain` | both | `ChatOpenAI` / `ChatAnthropic` with `bind_tools` |
| `litellm` | both | `litellm.completion`, which returns OpenAI format for both upstreams |

Each driver keeps the conversation history as an application would. It appends the restored assistant reply before the next turn, which is what makes placeholder stability across turns observable. Retries are disabled so that one scripted error is seen once.

To add a driver, write a stdio worker that follows `spec/driver-request.schema.json` and `spec/driver-response.schema.json` and add it to `drivers/drivers.toml`. Then check it with `parda-bench restore --control --driver <name>`: every failure other than `no_leak_upstream` is the driver's or its library's, and the report lists those under driver checks. For example, LiteLLM currently reports an upstream 529 to the application as 500.
