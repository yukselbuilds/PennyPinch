"""Five-minute, paced local A/B load; not a multi-hour or live-provider claim."""
from contextlib import ExitStack
from itertools import cycle, product
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import os
import site
import sys
import threading
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("stability_surfaces", HERE / "surfaces.py")
surfaces = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = surfaces
spec.loader.exec_module(surfaces)
replay = surfaces.replay


class Handler(surfaces.SurfaceHandler):
    def do_POST(self):
        if self.path == "/v1/responses":
            replay.FixtureHandler.do_POST(self)
        else:
            super().do_POST()


def body_outputs(body, surface):
    return replay.fixtures.outputs(body) if surface == "responses_http_sse" else surfaces.outputs(body, surface)


def execute(plan, run, controller, supervisor, payload, tokenizer):
    from websockets.sync.client import connect
    server = surfaces.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    server.daemon_threads = True
    server.capture_lock, server.captured = threading.Lock(), {}
    threading.Thread(target=server.serve_forever, daemon=True).start()
    engine = replay.Engine(controller, supervisor, payload, run / "engine", server.server_port, plan)
    result = {"passed": False, "rows": [], "warmup_rows": [], "engine": {}}
    sampler = None
    try:
        result["engine"].update(engine.start())
        sequence, pairs, started, deadline, next_pair = 0, 0, None, None, None
        combinations = list(product(plan["surfaces"], plan["cases"]))
        for phase in ("warmup", "load"):
            if phase == "load":
                sampler = replay.Resources([os.getpid()] + engine.pids, plan["memory_sample_interval_ms"])
                baseline = sampler.samples[0]["engine_private_bytes"]
                started = next_pair = time.perf_counter()
                deadline = started + plan["duration_seconds"]
            for surface, case in combinations if phase == "warmup" else cycle(combinations):
                if phase == "load" and time.perf_counter() >= deadline:
                    break
                model = plan["model"] if surface == "responses_http_sse" else plan["models"][surface]
                bodies = (replay.fixtures.conversation(case, model, plan["reasoning_effort"], plan["conversation_turns"])
                          if surface == "responses_http_sse" else
                          surfaces.conversation(surface, case, model, plan["conversation_turns"]))
                scope = f"{phase}-c{sequence}-{surface}-{case}"
                sequence += 1
                previous, sockets = None, {}
                with ExitStack() as stack:
                    if surface == "responses_websocket":
                        for arm in ("control", "headroom"):
                            port = engine.port if arm == "headroom" else server.server_port
                            sockets[arm] = stack.enter_context(connect(
                                f"ws://127.0.0.1:{port}/v1/responses", legacy=False,
                                additional_headers={"Authorization": "Bearer " + replay.SENTINEL,
                                                    "X-Proof-Case": scope + "-" + arm,
                                                    "x-headroom-session-id": scope + "-" + arm},
                                compression=None, proxy=None, max_size=2 * 1024 * 1024,
                                open_timeout=30, close_timeout=5))
                    for turn, body in enumerate(bodies):
                        if phase == "load":
                            while time.perf_counter() < min(next_pair, deadline):
                                time.sleep(max(0, min(.1, min(next_pair, deadline) - time.perf_counter())))
                            if time.perf_counter() >= deadline:
                                break
                        arms = ("control", "headroom") if (sequence + turn) % 2 == 0 else ("headroom", "control")
                        for arm in arms:
                            key = scope + f"-{arm}-t{turn}"
                            resources = replay.Resources([os.getpid()] + (engine.pids if arm == "headroom" else []),
                                                         plan["memory_sample_interval_ms"])
                            try:
                                port = engine.port if arm == "headroom" else server.server_port
                                if sockets:
                                    outcome = surfaces.websocket_request(sockets[arm], body, 30)
                                elif surface == "responses_http_sse":
                                    outcome = replay.request(port, body, key, 30)
                                else:
                                    outcome = surfaces.http_request(port, body, key, surface, 30)
                            finally:
                                resource_row = resources.finish()
                            with server.capture_lock:
                                captured = server.captured.pop(key, [])
                            replay.require(len(captured) == 1 and captured[0]["auth_preserved"], "stability_upstream_identity")
                            forwarded = captured[0]["body"]
                            if arm == "control":
                                replay.require(forwarded == body, "stability_control_changed")
                            else:
                                validate = replay.validate_body if surface == "responses_http_sse" else (
                                    lambda old, new, prior, protected: surfaces.validate(old, new, prior, surface, protected))
                                validate(body, forwarded, previous, case in replay.fixtures.PROTECTED)
                                previous = forwarded
                            retrieved = 0
                            for call_id, content in body_outputs(forwarded, surface).items():
                                for content_hash in set(replay.re.findall(
                                        r"(?:Retrieve more: hash=|<<ccr:)([a-fA-F0-9]{12,24})(?=[^a-fA-F0-9]|$)", content)):
                                    replay.require(arm == "headroom", "stability_control_ccr")
                                    replay.retrieval(engine.port, content_hash, body_outputs(body, surface)[call_id], 30)
                                    retrieved += 1
                            before = len(tokenizer.encode(replay.encoded(body).decode(), disallowed_special=()))
                            after = len(tokenizer.encode(replay.encoded(forwarded).decode(), disallowed_special=()))
                            row = {"phase": phase, "surface": surface, "case": case, "conversation": sequence,
                                   "turn": turn, "arm": arm, "input_sha256": hashlib.sha256(replay.encoded(body)).hexdigest(),
                                   "forwarded_sha256": hashlib.sha256(replay.encoded(forwarded)).hexdigest(),
                                   "original_tokens": before, "forwarded_tokens": after, "saved_tokens": before - after,
                                   "saved_percent": 100 * (before - after) / before, "ccr_retrievals_verified": retrieved,
                                   "provider_cached_input_tokens": None, "invoice_savings": None, "response_quality": None,
                                   "prefix_and_integrity": "pass", **outcome, **resource_row}
                            target = result["rows"] if phase == "load" else result["warmup_rows"]
                            target.append(row)
                            replay.write_new(run / f"{phase}-row-{len(target):04}.json", row)
                        if phase == "load":
                            pairs += 1
                            next_pair = started + pairs * plan["pair_interval_seconds"]
                            if pairs % 60 == 0:
                                print(json.dumps({"phase": phase, "pairs": pairs,
                                                  "elapsed_seconds": round(time.perf_counter() - started, 2)}), flush=True)
            if phase == "load":
                elapsed = time.perf_counter() - started
                finished_sampler = sampler
                sampler = None
                result["load_resources"] = finished_sampler.finish()
                final_private = finished_sampler.samples[-1]["engine_private_bytes"]
        groups = {}
        for row in result["rows"]:
            groups.setdefault((row["conversation"], row["turn"]), {})[row["arm"]] = row
        extra = [pair["headroom"]["total_ms"] - pair["control"]["total_ms"] for pair in groups.values()]
        cpu = [row["engine_cpu_ms"] + row["harness_cpu_ms"] for row in result["rows"] if row["arm"] == "headroom"]
        private_growth = result["load_resources"]["observed_engine_private_bytes"] - baseline
        combined_cpu = sum(result["load_resources"][key] for key in ("engine_cpu_ms", "harness_cpu_ms")) / (elapsed * 1000) * 100
        counts = {surface: sum(row["surface"] == surface and row["arm"] == "headroom" for row in result["rows"])
                  for surface in plan["surfaces"]}
        limits = plan["thresholds"]
        result.update(elapsed_seconds=elapsed, completed_pairs=pairs, pairs_per_surface=counts,
                      engine_private_after_warmup_bytes=baseline, engine_private_at_end_bytes=final_private,
                      engine_private_growth_after_warmup_bytes=private_growth,
                      combined_load_cpu_one_core_percent=combined_cpu,
                      warm_added_p95_ms=replay.percentile(extra), warm_cpu_p95_ms=replay.percentile(cpu))
        result["checks"] = {
            "duration": elapsed >= plan["duration_seconds"],
            "load": pairs >= limits["minimum_completed_pairs"] and all(n >= limits["minimum_pairs_per_surface"] for n in counts.values()),
            "memory": result["load_resources"]["observed_engine_private_bytes"] <= limits["engine_observed_private_bytes"]
                      and private_growth <= limits["engine_private_growth_after_warmup_bytes"],
            "cpu": combined_cpu <= limits["combined_load_cpu_one_core_percent"] and replay.percentile(cpu) <= limits["warm_cpu_p95_ms"],
            "latency": replay.percentile(extra) <= limits["warm_added_p95_ms"],
        }
        result["passed"] = all(result["checks"].values())
    except BaseException as error:
        result.update(passed=False, error_class=type(error).__name__,
                      error_code=str(error) if isinstance(error, AssertionError) else "bounded_stability_failed")
    finally:
        try:
            if sampler is not None:
                result["load_resources"] = sampler.finish()
        finally:
            try:
                engine.close()
                result["engine"]["stop"] = engine.stopped
            except BaseException as error:
                result.update(passed=False, stop_error_class=type(error).__name__,
                              stop_error_code=str(error) if isinstance(error, AssertionError) else "bounded_stability_stop_failed")
            finally:
                server.shutdown()
                server.server_close()
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true")
    args = parser.parse_args()
    replay.require(args.run and os.name == "nt" and Path.cwd().resolve() == ROOT, "explicit_windows_run_required")
    plan = json.loads((HERE / "plan.json").read_text())
    plan.update(json.loads((HERE / "surfaces-plan.json").read_text()))
    original_thresholds = plan["thresholds"]
    plan.update(json.loads((HERE / "stability-plan.json").read_text()))
    plan["thresholds"] = {**original_thresholds, **plan["thresholds"]}
    controller = replay.load_module("stability_controller", ROOT / "runtime/desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(ROOT / "runtime/manifest.json")
    replay.require(replay.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "stability_pinned_python")
    supervisor_path = payload / manifest["engine"]["runner"]
    supervisor = replay.load_module("stability_supervisor", supervisor_path)
    cache = controller.verify_tokenizer_cache(payload)
    os.environ["TIKTOKEN_CACHE_DIR"] = str(cache)
    site.addsitedir(str(payload / "Lib/site-packages"))
    import tiktoken
    tokenizer = tiktoken.get_encoding(plan["token_encoding"])
    run = ROOT / "work/proofs/optimization/stability" / str(uuid.uuid4())
    run.mkdir(parents=True)
    sources = [supervisor_path, Path(sys.executable)]
    sources.extend(HERE.glob("*.py"))
    sources.extend(HERE.glob("*.json"))
    sources.extend((ROOT / "runtime").glob("*.py"))
    sources.extend((ROOT / "runtime").glob("*.json"))
    sources.extend((payload / "Lib/site-packages/headroom").rglob("*.py"))
    sources.extend((payload / "Lib/site-packages/websockets").rglob("*.py"))
    sources.extend(path for path in cache.iterdir() if path.is_file())
    sources.extend(replay.prepare_fixture_runtime(run, payload))
    before = replay.fingerprints(sources)
    replay.write_new(run / "plan.json", plan)
    result = {"passed": False}
    try:
        result = execute(plan, run, controller, supervisor, payload, tokenizer)
    except BaseException as error:
        result.update(error_class=type(error).__name__,
                      error_code=str(error) if isinstance(error, AssertionError) else "bounded_stability_driver_failed")
    result.update(before=before, after=replay.fingerprints(sources), run=run.relative_to(ROOT).as_posix(),
                  fixture_network=replay.fixture_network_evidence(run),
                  environment={"windows_build": list(sys.getwindowsversion()[:3]), "python": sys.version,
                               "logical_cpus": os.cpu_count()}, external_provider_calls=0,
                  token_basis=plan["token_basis"], token_encoding=plan["token_encoding"],
                  stderr_status="controller_redirected_to_null_contents_unknown")
    result["inputs_unchanged"] = result["before"] == result["after"]
    result["passed"] = (result["passed"] and result["inputs_unchanged"]
                        and result["fixture_network"]["huggingface_hub_payload_bytes"] == 0)
    replay.write_new(run / "result.json", result)
    print(json.dumps({key: value for key, value in result.items() if key not in ("before", "after", "rows", "warmup_rows", "engine")}))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
