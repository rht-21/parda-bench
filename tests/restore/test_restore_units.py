import tomllib

import pytest

from parda_bench.restore.align import (
    AlignError,
    align,
    apply_mangle,
    render_expected,
    render_input,
    render_upstream_chunk,
)
from parda_bench.restore.checks import evaluate
from parda_bench.restore.driver.sse import Event, SseParser
from parda_bench.restore.engine import Engine, MockError, TextScript, ToolCallScript, UpstreamLog
from parda_bench.restore.scenarios import check
from parda_bench.restore.wire import api_for_path, parse_request, response_body, sse_events
from parda_bench.spec.protocol import Completed, Errored, Observation, ObservedToolCall
from parda_bench.spec.record import Fail, Pass, RestoreOutcome
from parda_bench.spec.scenario import (
    Api,
    CheckKind,
    InputSegment,
    LiteralInput,
    LiteralReply,
    Mangle,
    ReplySegment,
    Scenario,
    SlotInput,
    SlotReply,
)


def lit(t: str) -> LiteralInput:
    return LiteralInput(text=t)


def slot(n: int) -> SlotInput:
    return SlotInput(slot=n)


def test_finds_placeholders_between_literals_and_at_the_ends() -> None:
    segs: list[InputSegment] = [lit("Aadhaar "), slot(0), lit(" and PAN "), slot(1), lit(".")]
    assert align(segs, "Aadhaar <AADHAAR_1> and PAN [[PAN]].", []) == [(0, "<AADHAAR_1>"), (1, "[[PAN]]")]
    is_: list[InputSegment] = [slot(0), lit(" is "), slot(1)]
    assert align(is_, "P1 is P2", []) == [(0, "P1"), (1, "P2")]
    hi: list[InputSegment] = [lit("Hi "), slot(0)]
    assert align(hi, "Hi Rahul Sharma", []) == [(0, "Rahul Sharma")]
    deva: list[InputSegment] = [lit("मेरा नाम "), slot(0), lit(" है।")]
    assert align(deva, "मेरा नाम <PERSON_1> है।", []) == [(0, "<PERSON_1>")]


def test_backtracks_when_a_placeholder_contains_the_next_literal() -> None:
    assert align([lit("My email is "), slot(0), lit(".")], "My email is blair@example.com.", []) == [
        (0, "blair@example.com")
    ]
    segs: list[InputSegment] = [lit("Name: "), slot(0), lit(", email "), slot(1), lit(".")]
    assert align(segs, "Name: Rhonda Smith, email r.smith@example.org.", []) == [
        (0, "Rhonda Smith"),
        (1, "r.smith@example.org"),
    ]


def test_unmasked_value_containing_the_next_literal_is_taken_whole() -> None:
    segs: list[InputSegment] = [lit("IDs: "), slot(0), lit(" "), slot(1)]
    pii = ["2345 6789 0124", "ABCPS1234K"]
    assert align(segs, "IDs: 2345 6789 0124 ABCPS1234K", pii) == [(0, pii[0]), (1, pii[1])]
    assert align(segs, "IDs: [AADHAAR_1] [PAN_1]", pii)[0] == (0, "[AADHAAR_1]")


@pytest.mark.parametrize(
    ("segs", "masked"),
    [
        ([lit("My PAN "), slot(0)], "my PAN X"),
        ([lit("hello")], "hello world"),
        ([slot(0), slot(1)], "AB"),
        ([lit("a "), slot(0), lit(" b")], "a  b"),
    ],
)
def test_alignment_failures(segs: list[InputSegment], masked: str) -> None:
    with pytest.raises(AlignError):
        align(segs, masked, [])


def test_mangles_like_models_do() -> None:
    assert apply_mangle("<PERSON_1>", Mangle.LOWERCASE) == "<person_1>"
    assert apply_mangle("[[pan_2]]", Mangle.UPPERCASE) == "[[PAN_2]]"
    assert apply_mangle("<<PERSON_1>>", Mangle.STRIP_DELIMITERS) == "PERSON_1"
    assert apply_mangle("<PERSON_1>", Mangle.SPACE_PADDED) == "< PERSON_1 >"
    assert apply_mangle("PERSON_1", Mangle.SPACE_PADDED) == " PERSON_1 "


def test_split_placeholder_renders_fragments_and_expects_value_once() -> None:
    chunks: list[list[ReplySegment]] = [
        [LiteralReply(text="PAN: "), SlotReply(slot=0, to=3)],
        [SlotReply(slot=0, from_=3), LiteralReply(text=".")],
    ]
    placeholders = {0: "<PAN_1>"}
    assert [render_upstream_chunk(c, placeholders) for c in chunks] == ["PAN: <PA", "N_1>."]
    assert render_expected(chunks, ["ABCPS1234K"]) == "PAN: ABCPS1234K."
    assert render_upstream_chunk([SlotReply(slot=0, to=9, from_=5)], {0: "P1"}) == ""
    with pytest.raises(AlignError):
        render_upstream_chunk([SlotReply(slot=3)], {})
    assert render_input([lit("Call "), slot(0)], ["+91 98765 43210"]) == "Call +91 98765 43210"


BASIC = """
id = "s"
category = "basic"
description = "d"
api = "openai_chat"
stream = false
pii = ["ABCPS1234K"]
checks = ["no_leak_upstream", "restored_exact"]

[[turns]]
[turns.input]
kind = "user"
message = [{ text = "PAN " }, { slot = 0 }]
[turns.reply]
kind = "text"
chunks = [[{ text = "Got " }, { slot = 0, to = 3 }], [{ slot = 0, from = 3, mangle = "lowercase" }]]
"""


def parse(src: str) -> Scenario:
    return Scenario.model_validate(tomllib.loads(src))


def test_scenario_rules() -> None:
    assert check(parse(BASIC)) is None
    two = BASIC.replace('pii = ["ABCPS1234K"]', 'pii = ["A", "B"]')
    for bad in [
        BASIC.replace("{ slot = 0 }]", "{ slot = 1 }]"),
        two.replace('message = [{ text = "PAN " }, { slot = 0 }]', "message = [{ slot = 0 }, { slot = 1 }]"),
        BASIC.replace('"restored_exact"]', '"fails_closed"]'),
        two.replace('[{ text = "Got " }, { slot = 0, to = 3 }]', '[{ text = "Got " }, { slot = 1 }]'),
    ]:
        assert check(parse(bad)) is not None


def test_misspelled_fragment_bound_is_rejected() -> None:
    with pytest.raises(ValueError):
        parse(BASIC.replace("from = 3", "form = 3"))


MULTI = """
id = "s"
category = "multi_turn"
description = "d"
api = "openai_chat"
stream = true
pii = ["Ananya Iyer"]
checks = ["restored_exact"]
[[turns]]
input = { kind = "user", message = [{ text = "I am " }, { slot = 0 }] }
reply = { kind = "text", chunks = [[{ text = "Hi " }, { slot = 0, to = 4 }], [{ slot = 0, from = 4 }, { text = "!" }]] }
[[turns]]
input = { kind = "user", message = [{ text = "Again: " }, { slot = 0 }] }
reply = { kind = "text", chunks = [[{ slot = 0, mangle = "uppercase" }]] }
"""


def test_engine_learns_placeholders_and_reports_script_errors() -> None:
    e = Engine(parse(MULTI))
    assert e.reply(0, "I am <person_1>") == TextScript(["Hi <per", "son_1>!"])
    assert e.reply(1, "Again: <person_2>") == TextScript(["<PERSON_1>"])
    assert e.log.placeholders()[0] == "<person_1>"
    assert isinstance(e.reply(0, "Changed text"), MockError)
    assert isinstance(e.reply(5, "I am X"), MockError)
    assert len(e.log.errors) == 2


def test_wire_routing_parsing_and_bodies() -> None:
    assert api_for_path("/v1/chat/completions") is Api.OPENAI_CHAT
    assert api_for_path("/v1/v1/messages") is Api.ANTHROPIC_MESSAGES
    assert api_for_path("/v1/embeddings") is None
    body = {
        "model": "m",
        "stream": True,
        "messages": [
            {"role": "system", "content": "be nice"},
            {"role": "user", "content": "first"},
            {"role": "assistant", "content": None},
            {
                "role": "tool",
                "tool_call_id": "c1",
                "content": [{"type": "text", "text": "tool "}, {"type": "text", "text": "out"}],
            },
        ],
    }
    r = parse_request(Api.OPENAI_CHAT, body)
    assert (r.turn, r.latest_input, r.stream) == (1, "tool out", True)
    anthropic = {
        "messages": [
            {"role": "user", "content": "hi"},
            {
                "role": "user",
                "content": [
                    {
                        "type": "tool_result",
                        "tool_use_id": "c1",
                        "content": [{"type": "text", "text": "result"}],
                    }
                ],
            },
        ]
    }
    assert parse_request(Api.ANTHROPIC_MESSAGES, anthropic).latest_input == "result"
    events = sse_events(Api.OPENAI_CHAT, TextScript(["a", "b"]), "m")
    assert len(events) == 5 and '"content":"a"' in events[1] and events[-1] == "data: [DONE]\n\n"
    call = ToolCallScript("c1", "lookup", ['{"pan": ', '"<PAN_1>"}'])
    assert response_body(Api.ANTHROPIC_MESSAGES, call, "m")["content"][0]["input"]["pan"] == "<PAN_1>"


def scenario(checks: str, reply: str) -> Scenario:
    return parse(f"""
id = "s"
category = "basic"
description = "d"
api = "openai_chat"
stream = false
pii = ["2345 6789 0124"]
checks = [{checks}]
tools = [{{ name = "verify", description = "d", parameters = {{}} }}]
[[turns]]
input = {{ kind = "user", message = [{{ text = "Aadhaar " }}, {{ slot = 0 }}] }}
reply = {reply}
""")


TEXT = '{ kind = "text", chunks = [[{ text = "Saved " }, { slot = 0 }]] }'
ALL = '"no_leak_upstream", "restored_exact", "no_placeholder_fragment"'


def log_with(payload: str, placeholder: str) -> UpstreamLog:
    return UpstreamLog(
        payloads=[{"messages": [{"role": "user", "content": payload}]}], alignments=[(0, [(0, placeholder)])]
    )


def completed(text: str) -> Completed:
    return Completed(text=text)


def failed(outcome: RestoreOutcome) -> list[CheckKind]:
    return [f.check for f in outcome.failures] if isinstance(outcome, Fail) else []


def test_checks() -> None:
    s = scenario(ALL, TEXT)
    assert isinstance(evaluate(s, log_with("Aadhaar <A_1>", "<A_1>"), [completed("Saved 2345 6789 0124")]), Pass)
    leaked = evaluate(s, log_with("Aadhaar 234567890124", "234567890124"), [completed("Saved 2345 6789 0124")])
    assert failed(leaked) == [CheckKind.NO_LEAK_UPSTREAM]
    half = evaluate(s, log_with("Aadhaar <AADHAAR_1>", "<AADHAAR_1>"), [completed("Saved <AADHA")])
    assert failed(half) == [CheckKind.RESTORED_EXACT, CheckKind.NO_PLACEHOLDER_FRAGMENT]


def test_tool_call_arguments_compare_as_json() -> None:
    reply = """{ kind = "tool_call", call_id = "c1", name = "verify", argument_chunks = [[{ text = '{"id": "' }, { slot = 0 }, { text = '"}' }]] }"""
    s = scenario('"restored_exact", "json_valid"', reply)
    obs: list[Observation] = [
        Completed(
            text="",
            tool_calls=[ObservedToolCall(id="c1", name="verify", arguments='{"id":"2345 6789 0124"}')],
        )
    ]
    assert isinstance(evaluate(s, log_with("x", "<A>"), obs), Pass)


def test_fails_closed_stability_and_mock_errors() -> None:
    s = scenario('"fails_closed"', '{ kind = "http_error", status = 500, body = "{}" }')
    assert failed(evaluate(s, log_with("x", "<A>"), [completed("All good!")])) == [CheckKind.FAILS_CLOSED]
    assert isinstance(evaluate(s, log_with("x", "<A>"), [Errored(message="500", http_status=500)]), Pass)
    stable = scenario('"placeholder_stable_across_turns"', TEXT)
    log = log_with("x", "<A1>")
    log.alignments.append((1, [(0, "<A2>")]))
    assert len(failed(evaluate(stable, log, [completed("Saved 2345 6789 0124")]))) == 1
    log = log_with("x", "<A>")
    log.errors.append("turn 0: literal not found")
    assert failed(evaluate(scenario('"no_leak_upstream"', TEXT), log, [])) == [CheckKind.RESTORED_EXACT] * 2


def test_sse_parser_handles_split_reads() -> None:
    p = SseParser()
    assert p.push('event: message_start\r\ndata: {"a"') == []
    assert p.push(":1}\r\n\r\ndata: [DONE]\n\n: comment\n\n") == [
        Event("message_start", '{"a":1}'),
        Event(None, "[DONE]"),
    ]
