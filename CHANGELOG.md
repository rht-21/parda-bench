# Changelog

## 0.2.0 (2026-10-02)

First tagged release, for internal benchmarking.

### Detection benchmark
- `parda-bench detect`, which runs any tool over a labeled dataset and scores it.
- Strict and relaxed span matching, per-entity precision, recall and F1, and slices by language, difficulty and source.
- Hard-negative rates, unmapped tool labels, and latency both inside the tool and end to end.
- Dataset `v0.2.0`: 2,925 synthetic samples covering 15 entity types in English, Hinglish, Devanagari and code-mixed text.
  - Template samples, 300 LLM-written free-form texts, and noisy text with OCR-style damage.
  - 685 hard negatives across 11 look-alike types.
  - Every `v0.1.0` sample is included unchanged.
- Dataset `v0.1.0`: 2,225 template samples, kept for comparison.

### Restore suite
- `parda-bench restore`: 30 scripted scenarios against a mock LLM. They cover streaming splits, tool calls, multi-turn conversations, placeholder mangling, repeats, Unicode, JSON output and upstream errors.
- Six checks:
  - no leak upstream
  - exact restore
  - no placeholder fragments
  - stable placeholders
  - valid JSON
  - fails closed
- Drivers: `raw_http`, `openai_sdk`, `anthropic_sdk`, `langchain` and `litellm`, plus `--control` runs that check the drivers themselves.

### Tools
- `regex-baseline` and `regex-proxy`: reference tools built on regular expressions and check digits.
- `presidio`: Microsoft Presidio 2.2.358 with its default analyzer.
- `langchain-presidio`: LangChain's `PresidioReversibleAnonymizer` from `langchain-experimental` 0.4.2.

### Integration
- Tools run out of process over `stdio`, `http-api` or `http-proxy` transports.
- Python and Node adapter SDKs.
- JSON Schemas for every contract in `spec/`.
- `parda-bench report` writes a Markdown and HTML report from stored run files.

### Licensing
- Code: Apache-2.0. Dataset: CC BY 4.0.

### Known limitations
- The dataset is synthetic. The Hindi, Hinglish and mixed-script texts have not been reviewed by native speakers, and there are no human-written samples yet.
- Linux and macOS only.
