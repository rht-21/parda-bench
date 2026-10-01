use parda_spec::protocol::adapter::{AdapterOp, AdapterRequest, AdapterResponse};
use parda_spec::sample::{Labels, Sample};
use parda_spec::scenario::ReplySegment;
use serde_json::json;

#[test]
fn hard_negative_cannot_carry_spans() {
    let labels = json!({"kind": "hard_negative", "decoy": "PHONE", "spans": []});
    assert!(serde_json::from_value::<Labels>(labels).is_err());
}

#[test]
fn positive_labels_require_spans() {
    assert!(serde_json::from_value::<Labels>(json!({"kind": "positive"})).is_err());
}

#[test]
fn sample_round_trips_with_wire_names() {
    let wire = json!({
        "id": "s1",
        "text": "PAN ABCPS1234K",
        "labels": {"kind": "positive", "spans": [{"start": 4, "end": 14, "entity": "PAN"}]},
        "lang": "hi-Latn",
        "difficulty": "easy",
        "source": "template",
        "generator_version": "parda-data/0.1.0"
    });
    let sample: Sample = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(&sample).unwrap(), wire);
}

#[test]
fn adapter_request_is_flat_with_op_tag() {
    let request = AdapterRequest {
        id: 3,
        op: AdapterOp::Mask {
            session: "s".to_owned(),
            text: "t".to_owned(),
        },
    };
    assert_eq!(
        serde_json::to_value(&request).unwrap(),
        json!({"id": 3, "op": "mask", "session": "s", "text": "t"})
    );
}

#[test]
fn adapter_error_response_parses() {
    let response: AdapterResponse =
        serde_json::from_value(json!({"id": 1, "status": "error", "message": "boom"})).unwrap();
    assert_eq!(response.id, 1);
}

#[test]
fn reply_segment_distinguishes_literal_and_fragment() {
    let literal: ReplySegment = serde_json::from_value(json!({"text": "hi"})).unwrap();
    let fragment: ReplySegment = serde_json::from_value(json!({"slot": 0, "to": 3})).unwrap();
    assert_eq!(
        literal,
        ReplySegment::Literal {
            text: "hi".to_owned()
        }
    );
    assert_eq!(
        fragment,
        ReplySegment::Slot {
            slot: 0,
            from: None,
            to: Some(3),
            mangle: None
        }
    );
}

#[test]
fn schemas_are_deterministic() {
    let render = || -> Vec<String> {
        parda_spec::schemas()
            .into_iter()
            .map(|(_, s)| serde_json::to_string(&s).unwrap())
            .collect()
    };
    assert_eq!(render(), render());
    assert_eq!(parda_spec::schemas().len(), 10);
}

#[test]
fn misspelled_fragment_bound_is_rejected() {
    assert!(serde_json::from_value::<ReplySegment>(json!({"slot": 0, "form": 3})).is_err());
}

#[test]
fn unknown_manifest_key_is_rejected() {
    let manifest = json!({
        "name": "t", "version": "1", "commit": "c", "license": "MIT", "language": "python",
        "homepage": "https://example.org", "capabilities": ["detect"],
        "transport": {"kind": "stdio", "command": ["x"]},
        "entity_map": {}, "configs": {"default": {}}, "entitymap": {}
    });
    assert!(serde_json::from_value::<parda_spec::manifest::ToolManifest>(manifest).is_err());
}
