"""Metadata-only investigation of the failed stability memory limit; no acceptance PASS."""
from pathlib import Path
import argparse
import ctypes
import http.client
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
spec = importlib.util.spec_from_file_location("memory_stability", HERE / "stability.py")
stability = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = stability
spec.loader.exec_module(stability)
surfaces, replay = stability.surfaces, stability.replay


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true")
    args = parser.parse_args()
    replay.require(args.run and os.name == "nt" and Path.cwd().resolve() == ROOT, "explicit_windows_run_required")
    plan = json.loads((HERE / "plan.json").read_text())
    plan.update(json.loads((HERE / "surfaces-plan.json").read_text()))
    thresholds = plan["thresholds"]
    plan.update(json.loads((HERE / "stability-plan.json").read_text()))
    plan["thresholds"] = {**thresholds, **plan["thresholds"]}
    # A shorter diagnostic run is distinct from the unchanged 300-second acceptance plan.
    plan["duration_seconds"] = 120
    controller = replay.load_module("memory_controller", ROOT / "runtime/desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(ROOT / "runtime/manifest.json")
    replay.require(replay.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "diagnostic_pinned_python")
    supervisor_path = payload / manifest["engine"]["runner"]
    supervisor = replay.load_module("memory_supervisor", supervisor_path)
    cache = controller.verify_tokenizer_cache(payload)
    os.environ["TIKTOKEN_CACHE_DIR"] = str(cache)
    site.addsitedir(str(payload / "Lib/site-packages"))
    import tiktoken
    tokenizer = tiktoken.get_encoding(plan["token_encoding"])
    observations = []
    base_engine = replay.Engine

    class ProbedEngine(base_engine):
        def start(self):
            info = super().start()
            self.probe_stop = threading.Event()
            self.probe_started = time.perf_counter()
            self.probe_seen_modules = set()
            self.snapshot()

            def collect():
                while not self.probe_stop.wait(5):
                    self.snapshot()

            self.probe_thread = threading.Thread(target=collect, daemon=True)
            self.probe_thread.start()
            return info

        def snapshot(self):
            row = {"elapsed_seconds": time.perf_counter() - self.probe_started}
            connection = http.client.HTTPConnection("127.0.0.1", self.port, timeout=2)
            try:
                connection.request("GET", "/debug/memory")
                response = connection.getresponse()
                replay.require(response.status == 200, "diagnostic_memory_status")
                report = json.loads(response.read(128 * 1024))
                allowed = ("entry_count", "size_bytes", "budget_bytes", "hits", "misses", "evictions")
                row["components"] = {
                    name: {key: value[key] for key in allowed if key in value}
                    for name, value in report["components"].items()
                    if name in ("compression_store", "semantic_cache", "request_logger", "batch_context_store")}
                row["total_tracked_bytes"] = report["total_tracked_bytes"]
                row["processes"] = {}
                for label, pid in zip(("controller", "helper", "engine"), self.pids, strict=True):
                    resource = replay.Resources([pid], 50)
                    try:
                        info = resource.samples[0]
                        row["processes"][label] = {"private": info["engine_private_bytes"], "rss": info["working_set_bytes"]}
                        if label == "engine":
                            kernel, handle = resource.kernel, resource.handles[0][1]
                            kernel.K32EnumProcessModules.argtypes = [replay.wintypes.HANDLE, ctypes.c_void_p,
                                                                    replay.wintypes.DWORD, ctypes.POINTER(replay.wintypes.DWORD)]
                            kernel.K32GetModuleFileNameExW.argtypes = [replay.wintypes.HANDLE, ctypes.c_void_p,
                                                                     replay.wintypes.LPWSTR, replay.wintypes.DWORD]
                            addresses, needed = (ctypes.c_void_p * 2048)(), replay.wintypes.DWORD()
                            replay.require(kernel.K32EnumProcessModules(handle, addresses, ctypes.sizeof(addresses), ctypes.byref(needed))
                                           and needed.value <= ctypes.sizeof(addresses), "diagnostic_module_enumeration")
                            modules = set()
                            for address in addresses[:needed.value // ctypes.sizeof(ctypes.c_void_p)]:
                                name = ctypes.create_unicode_buffer(4096)
                                replay.require(kernel.K32GetModuleFileNameExW(handle, address, name, len(name)), "diagnostic_module_name")
                                modules.add(Path(name.value).name.lower())
                            row["new_module_basenames"] = sorted(modules - self.probe_seen_modules)
                            self.probe_seen_modules = modules
                    finally:
                        resource.finish()
            except BaseException as error:
                row["error_class"] = type(error).__name__
                row["error_code"] = str(error) if isinstance(error, AssertionError) else "bounded_memory_probe_failed"
            finally:
                connection.close()
                observations.append(row)

        def close(self):
            if hasattr(self, "probe_stop"):
                self.probe_stop.set()
                self.probe_thread.join(3)
                self.snapshot()
            super().close()

    replay.Engine = ProbedEngine
    run = ROOT / "work/proofs/optimization/memory-diagnosis" / str(uuid.uuid4())
    run.mkdir(parents=True)
    sources = [Path(sys.executable), supervisor_path]
    sources.extend(HERE.glob("*.py"))
    sources.extend(HERE.glob("*.json"))
    sources.extend((ROOT / "runtime").glob("*.py"))
    sources.extend((ROOT / "runtime").glob("*.json"))
    sources.extend((payload / "Lib/site-packages/headroom").rglob("*.py"))
    sources.extend(replay.prepare_fixture_runtime(run, payload))
    before = replay.fingerprints(sources)
    replay.write_new(run / "diagnostic-plan.json", {**plan, "diagnostic_only": True, "observation_interval_seconds": 5})
    result = stability.execute(plan, run, controller, supervisor, payload, tokenizer)
    result.update(diagnostic_only=True, performance_acceptance="not_evaluated", observations=observations,
                  fixture_network=replay.fixture_network_evidence(run),
                  before=before, after=replay.fingerprints(sources), run=run.relative_to(ROOT).as_posix())
    result["inputs_unchanged"] = result["before"] == result["after"]
    replay.write_new(run / "result.json", result)
    print(json.dumps({"diagnostic_only": True, "observations": len(observations), "run": result["run"],
                      "inputs_unchanged": result["inputs_unchanged"], "performance_acceptance": "not_evaluated"}))
    return 0 if result["inputs_unchanged"] and observations and all("error_code" not in row for row in observations) else 1


if __name__ == "__main__":
    raise SystemExit(main())
