import json

import pytest
from pydantic import TypeAdapter, ValidationError

from parda_bench.spec.manifest import ToolManifest
from parda_bench.spec.protocol import AdapterResponse, MaskRequest
from parda_bench.spec.sample import Labels, Sample
from parda_bench.spec.scenario import LiteralReply, ReplySegment, SlotReply
from parda_bench.spec.schemas import schemas


def test_hard_negative_cannot_carry_spans_and_positive_needs_them() -> None:
    with pytest.raises(ValidationError):
        TypeAdapter(Labels).validate_python({"kind": "hard_negative", "decoy": "PHONE", "spans": []})
    with pytest.raises(ValidationError):
        TypeAdapter(Labels).validate_python({"kind": "positive"})


def test_sample_round_trips_with_wire_names() -> None:
    wire = {
        "id": "s1",
        "text": "PAN ABCPS1234K",
        "labels": {"kind": "positive", "spans": [{"start": 4, "end": 14, "entity": "PAN"}]},
        "lang": "hi-Latn",
        "difficulty": "easy",
        "source": "template",
        "generator_version": "g",
    }
    assert Sample.model_validate(wire).model_dump(mode="json") == wire


def test_adapter_messages_are_flat_with_op_tag() -> None:
    assert MaskRequest(id=3, session="s", text="t").model_dump() == {
        "id": 3,
        "op": "mask",
        "session": "s",
        "text": "t",
    }
    assert TypeAdapter(AdapterResponse).validate_python({"id": 1, "status": "error", "message": "boom"}).id == 1


def test_reply_segments_distinguish_literal_and_fragment() -> None:
    seg: TypeAdapter[ReplySegment] = TypeAdapter(ReplySegment)
    assert seg.validate_python({"text": "hi"}) == LiteralReply(text="hi")
    assert seg.validate_python({"slot": 0, "to": 3}) == SlotReply(slot=0, to=3)
    with pytest.raises(ValidationError):
        seg.validate_python({"slot": 0, "form": 3})


def test_unknown_manifest_key_is_rejected() -> None:
    manifest = {
        "name": "t",
        "version": "1",
        "commit": "c",
        "license": "MIT",
        "language": "python",
        "homepage": "https://example.org",
        "capabilities": ["detect"],
        "transport": {"kind": "stdio", "command": ["x"]},
        "entity_map": {},
        "configs": {"default": {}},
        "entitymap": {},
    }
    with pytest.raises(ValidationError):
        ToolManifest.model_validate(manifest)


def test_schemas_are_deterministic() -> None:
    assert json.dumps(schemas()) == json.dumps(schemas()) and len(schemas()) == 10
