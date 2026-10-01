"""Test service for the http-api transport: each POSTed adapter request gets an adapter response."""

import json
from http.server import BaseHTTPRequestHandler, HTTPServer


class Handler(BaseHTTPRequestHandler):
    def do_POST(self) -> None:  # noqa: N802 - http.server naming
        request = json.loads(self.rfile.read(int(self.headers["content-length"])))
        op = request["op"]
        if op == "hello":
            result = {"op": op, "tool_version": "0.0.0", "capabilities": ["detect"]}
        elif op == "detect" and request["text"] == "error":
            self._reply({"id": request["id"], "status": "error", "message": "tool rejected input"})
            return
        elif op == "detect":
            result = {"op": op, "entities": [{"start": 0, "end": len(request["text"]), "label": "WORD", "score": None}]}
        else:
            result = {"op": op}
        self._reply({"id": request["id"], "status": "ok", "elapsed_ns": 1000, "result": result})

    def _reply(self, body: dict) -> None:
        payload = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, *args: object) -> None:
        pass


HTTPServer(("127.0.0.1", 18432), Handler).serve_forever()
