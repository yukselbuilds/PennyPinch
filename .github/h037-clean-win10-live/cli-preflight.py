from http import HTTPStatus
import json
import os
from pathlib import Path
import sys
import threading
import uuid

from websockets.exceptions import ConnectionClosed
from websockets.sync.server import serve

recipe = Path(__file__).resolve().parent
sys.path.insert(0, str(recipe))
from turn_diagnostics import run_turn


client, work = (Path(value).resolve(strict=True) for value in sys.argv[1:])
workspace = work / "smoke-workspace"
profile = work / "smoke-home"
workspace.mkdir()
profile.mkdir()
marker = "PP_SYNTHETIC_" + uuid.uuid4().hex[:10].upper()
probe_key = "pp-h037-local-fixture"
counts = {"turn": 0, "warmup": 0}
handshake = {"requests": 0, "accepted": 0}


def authorize(connection, request):
    handshake["requests"] += 1
    if request.path != "/v1/responses" or request.headers.get("authorization") not in (None, "Bearer " + probe_key):
        return connection.respond(HTTPStatus.BAD_REQUEST, "fixture_route_or_key_mismatch")
    handshake["accepted"] += 1
    return None


def handle(connection):
    try:
        for index in range(2):
            frame = json.loads(connection.recv(timeout=12))
            assert frame["type"] == "response.create" and frame["model"] == "gpt-5-codex"
            warmup = frame.get("generate") is False
            counts["warmup" if warmup else "turn"] += 1
            response = {"id": f"resp_pp_fixture_{index}", "object": "response", "model": frame["model"], "output": []}
            connection.send(json.dumps({"type": "response.created", "response": {**response, "status": "in_progress"}}))
            tokens, generated = (0, 0) if warmup else (42, 11)
            items = []
            if not warmup:
                message = {"id": "msg_pp_fixture", "type": "message", "role": "assistant", "status": "completed",
                           "content": [{"type": "output_text", "text": marker, "annotations": []}]}
                items.append(message)
                connection.send(json.dumps({"type": "response.output_item.done", "output_index": 0, "item": message}))
            usage = {"input_tokens": tokens, "output_tokens": generated, "total_tokens": tokens + generated,
                     "input_tokens_details": {"cached_tokens": 0}, "output_tokens_details": {"reasoning_tokens": 0}}
            connection.send(json.dumps({"type": "response.completed",
                                        "response": {**response, "status": "completed", "output": items, "usage": usage}}))
            if not warmup:
                return
    except (ConnectionClosed, TimeoutError):
        return


with serve(handle, "127.0.0.1", 0, max_size=1024 * 1024, process_request=authorize, ping_interval=None) as server:
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        authority = f"http://127.0.0.1:{server.socket.getsockname()[1]}"
        (profile / "config.toml").write_text(f'openai_base_url = "{authority}/v1"\nmodel = "gpt-5-codex"\n'
                                            'cli_auth_credentials_store = "file"\nweb_search = "disabled"\n', encoding="utf-8")
        env = {k: v for k, v in os.environ.items()
               if not k.upper().startswith(("CODEX_", "OPENAI_", "HEADROOM_", "ANTHROPIC_", "OTEL_"))
               and not k.upper().endswith("_PROXY")}
        env.update(CODEX_HOME=str(profile), CODEX_SQLITE_HOME=str(profile), OPENAI_API_KEY=probe_key,
                   HTTP_PROXY=authority, HTTPS_PROXY=authority, NO_PROXY="127.0.0.1,localhost")
        evidence, usage = run_turn(client, workspace, marker, env)
        facts = {"diagnostic": evidence, "loopback_frames": counts, "handshake": handshake,
                 "passed": evidence["outcome"] == "pass" and counts["turn"] == 1 and usage["input_tokens"] == 42
                 and handshake["accepted"] >= 1}
        (work / "synthetic-preflight.json").write_text(json.dumps(facts), encoding="utf-8")
        print(json.dumps(facts))
        raise SystemExit(0 if facts["passed"] else 1)
    finally:
        server.shutdown()
        thread.join(2)
