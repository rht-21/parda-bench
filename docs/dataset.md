# Dataset

The detection dataset is generated from templates in `crates/parda-data/templates/` and published under `data/<version>/`:

- `samples.jsonl`: one `sample` per line (schema `spec/sample.schema.json`).
- `manifest.json`: version, seed, samples per template, generator version, sample count and the SHA-256 of `samples.jsonl`.

Samples come from two sources, recorded in each sample's `source` field and reported as separate score slices:

- `template`: the templates are rendered many times each (25 by default) with generated values.
- `llm`: free-form texts in `templates/freeform/`, each rendered once. They were written by an LLM (Claude) to vary sentence structure beyond what templates reach. `handwritten` is reserved for texts a person writes.

`parda-bench data build` is deterministic: each sample's random generator is seeded from the build seed, the template id and the sample index. Editing or adding one template never changes another template's samples. A published version is immutable: rebuilding under the same version must give byte-identical samples, or the build fails, and CI enforces this. Any change to the output needs a new `--version`.

## Labels

A sample is either **positive**, with zero or more gold spans, or a **hard negative**: text with exactly one decoy that resembles an entity but is not personal data, such as an order number shaped like an Aadhaar or a railway PNR shaped like a mobile number. A tool should flag nothing in a hard negative.

`data validate` checks the digest, span bounds and order, that spans carry no surrounding whitespace, and that every labeled identifier passes its structural validator:
- Verhoeff for Aadhaar and VID
- Luhn for cards
- mod-36 check character for GSTIN
- formats for PAN, IFSC, UPI, phone, email, passport, voter ID and vehicle registration

## Templates

```toml
[[template]]
id = "en-kyc-01"          # unique across all template files; becomes the sample id prefix
lang = "en"               # en | hi-Latn | hi-Deva | mixed
difficulty = "easy"       # easy | medium | hard
text = "Please update my KYC. Name: {name}, Aadhaar: {aadhaar:spaced}, PAN: {pan}."
```

`noisy = true` damages the text around entities at render time: OCR confusions (O/0, l/1, S/5), dropped or doubled spaces and dropped punctuation. Entity values are never altered, and each piece of text keeps its first and last character, so the separator next to an entity survives.

A file may set `source = "llm"` (default `template`); its entries then render once each. Free-form files list entries compactly as `template = [ { id = "...", lang = "...", difficulty = "...", text = "..." }, ... ]`.

A filler is `{kind}` or `{kind:style}`; write `{{` and `}}` for literal braces. Without a style, a random one is chosen per sample. Names, addresses and dates default to Devanagari in `hi-Deva` templates.

Each sample has one persona, so fillers in the same text agree: `{phone}` and `{upi:phone}` use the same number, `{email}` and `{upi:name}` are built from `{name}`, and the PAN's fifth character is the surname's initial. `{other_name}` is a second person.

| Filler | Styles | Entity |
|---|---|---|
| `aadhaar`, `vid` | `plain`, `spaced`, `hyphen` | `AADHAAR`, `AADHAAR_VID` |
| `pan`, `gstin`, `ifsc` | `upper` (default), `lower` | `PAN`, `GSTIN`, `IFSC` |
| `upi` | `name`, `phone` | `UPI_ID` |
| `phone` | `intl`, `intl_hyphen`, `zero`, `plain`, `split` | `PHONE` |
| `email` | `lower` (default), `title` | `EMAIL` |
| `card` | `plain`, `spaced`, `hyphen` | `PAYMENT_CARD` |
| `passport`, `voter` | none | `PASSPORT`, `VOTER_ID` |
| `vehicle` | `plain`, `spaced`, `hyphen` | `VEHICLE_REGISTRATION` |
| `name`, `other_name` | `full`, `first`, `upper`, `deva`, `deva_first` | `PERSON_NAME` |
| `address` | `latin`, `deva` | `ADDRESS` |
| `dob` | `slashed`, `hyphen`, `dotted`, `long`, `month_first`, `iso`, `deva` | `DATE_OF_BIRTH` |

Hand-labeled text, mostly for free-form samples:

- `{=ENTITY:text}` labels `text` as `ENTITY` (a wire name such as `PERSON_NAME`). The text must pass the entity's structural validator, so checksummed identifiers should come from fillers instead.
- `{!ENTITY:text}` is a literal decoy, such as a toll-free number written as `{!PHONE:1800 425 3800}`.

Labeling conventions:

- Titles and honorifics stay outside name spans: `Mr. {=PERSON_NAME:Fernandes}`, `{=PERSON_NAME:Sunita} ji`.
- A date is `DATE_OF_BIRTH` only when it is someone's birth date. Festival and appointment dates are decoys.
- Organizations, toll-free numbers and places named after people are not personal data.

Decoys (hard-negative templates hold exactly one and no other fillers):

| Decoy | Looks like | Renders |
|---|---|---|
| `decoy_aadhaar[:plain\|spaced]` | `AADHAAR` | 12 digits with a failing Verhoeff digit |
| `decoy_card[:plain\|spaced\|hyphen]` | `PAYMENT_CARD` | 16 digits failing Luhn |
| `decoy_pan` | `PAN` | PAN shape with an impossible holder-type letter |
| `decoy_pnr` | `PHONE` | 10-digit railway PNR |
| `decoy_date[:style]` | `DATE_OF_BIRTH` | a recent date that is not a birth date |
| `decoy_place` | `PERSON_NAME` | a place named after a person (Nehru Place, Rajiv Chowk) |
| `decoy_passport` | `PASSPORT` | a booking reference in passport format |
| `decoy_gstin` | `GSTIN` | 15-character code with a failing check character |
| `decoy_ifsc` | `IFSC` | IFSC-shaped product or coupon code with no bank's prefix |
| `decoy_handle` | `UPI_ID` | social-media handle such as `@rahul.sharma` |
| `decoy_service_account` | `EMAIL` | machine account such as `root@localhost` |

## Versions

| Version | Samples | Hard negatives | Sources | Notes |
|---|---|---|---|---|
| `v0.1.0` | 2,225 | 450 (7 decoy types) | template | 89 templates |
| `v0.2.0` | 2,925 | 685 (11 decoy types) | 2,625 template, 300 llm | Adds 8 noisy templates, 8 hard-negative templates for the new decoys and 300 free-form texts. Every `v0.1.0` sample is included unchanged |

Both are built with seed 42 and 25 samples per template. Run `parda-bench data stats data/<version>` for the full breakdown by entity, language, difficulty and source.

The free-form texts in Hindi, Hinglish and mixed script have not yet been reviewed by a native speaker.
