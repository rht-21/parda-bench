"""LangChain chat models (`ChatOpenAI` / `ChatAnthropic`) with tools bound the LangChain way."""

from __future__ import annotations

import json
from typing import Any

from langchain_anthropic import ChatAnthropic
from langchain_core.language_models import BaseChatModel
from langchain_core.messages import AIMessage, BaseMessage, HumanMessage, ToolMessage
from langchain_openai import ChatOpenAI

from parda_drivers.protocol import Completed, ToolCall, openai_tools, serve


def model_for(request: dict[str, Any]) -> BaseChatModel:
    if request["api"] == "openai_chat":
        return ChatOpenAI(base_url=request["base_url"], api_key="parda-bench", model=request["model"], max_retries=0)
    return ChatAnthropic(base_url=request["base_url"], api_key="parda-bench", model=request["model"], max_retries=0)


def text_of(message: BaseMessage) -> str:
    if isinstance(message.content, str):
        return message.content
    return "".join(b.get("text", "") for b in message.content if isinstance(b, dict) and b.get("type") == "text")


def run_turn(request: dict[str, Any], history: list[Any], turn: dict[str, Any]) -> Completed:
    if turn["kind"] == "user":
        history.append(HumanMessage(turn["text"]))
    else:
        history.append(ToolMessage(turn["content"], tool_call_id=turn["call_id"]))
    model: Any = model_for(request)
    if request["tools"]:
        model = model.bind_tools(openai_tools(request))
    if request["stream"]:
        reply: Any = None
        for chunk in model.stream(history):
            reply = chunk if reply is None else reply + chunk
    else:
        reply = model.invoke(history)
    calls = [ToolCall(c["id"] or "", c["name"], json.dumps(c["args"], ensure_ascii=False)) for c in reply.tool_calls]
    history.append(AIMessage(content=reply.content, tool_calls=reply.tool_calls))
    return Completed(text_of(reply), calls)


if __name__ == "__main__":
    serve(run_turn)
