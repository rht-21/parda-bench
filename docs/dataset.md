# Dataset

The detection dataset is generated from templates in `crates/parda-data/templates/` and published under `data/<version>/`:

- `samples.jsonl`: one `sample` per line (schema `spec/sample.schema.json`).
- `manifest.json`: version, seed, samples per template, generator version, sample count and the SHA-256 of `samples.jsonl`.

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

## Composition of v0.1.0

Built with seed 42 and 25 samples per template from 89 templates: 2,225 samples, including 450 hard negatives and 125 texts with no personal data. Run `parda-bench data stats data/v0.1.0` for the full breakdown by entity, language and difficulty.
