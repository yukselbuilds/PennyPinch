"""A real disposable Codex GUI turn through the packaged Headroom application."""

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import sqlite3
import stat
import subprocess
import sys
import time
import uuid


ROOT = Path(__file__).resolve().parents[1]


def load(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


desktop = load("_desktop_traffic_preflight", "scripts/probe-macos-desktop.py")
core = desktop.core


class ControlledTurn:
    def __init__(self, fixture, endpoint, *, via_headroom=True):
        self.fixture, self.endpoint = fixture, endpoint
        self.headroom_route = "controlled_fixture" if via_headroom else "not_tested_direct_fixture"

    def prepare(self, state, config):
        auth = state / "codex/auth.json"
        core.require(not auth.exists() and not auth.is_symlink(), "test_auth_already_exists")
        workspace = state / "workspace"
        workspace.mkdir(mode=0o700)
        fields = (f'model = {json.dumps(self.fixture.model)}\n'
                  f'model_reasoning_effort = {json.dumps(self.fixture.reasoning_effort)}\n'
                  f'[projects.{json.dumps(str(workspace))}]\ntrust_level = "trusted"\n')
        text = config.read_text(encoding="utf-8")
        core.require(text.count("[analytics]") == 1, "unexpected_test_config")
        config.write_text(text.replace("[analytics]", fields + "[analytics]"), encoding="utf-8")
        with auth.open("x", encoding="utf-8") as stream:
            json.dump({"auth_mode": "apikey", "OPENAI_API_KEY": self.fixture.api_key}, stream)
        auth.chmod(0o600)

    def exercise(self, page):
        # Only ordinary rendered controls are used; no app-server turn or app internals.
        for _ in range(8):
            observation = page.observe()
            print("headroom-desktop-turn-ui=" + json.dumps(observation, separators=(",", ":")), flush=True)
            if observation["editors"] == 1:
                break
            if observation["api_key_inputs"] == 1:
                page.insert_text(self.fixture.api_key, "api_key")
                core.require(page.click("Continue"), "api_key_continue_missing")
            else:
                choices = ("Sign in another way", "Use an API key", "Skip for now", "Skip", "Not now", "Get started", "Continue", "New thread", "New chat")
                label = next((item for item in choices if item in observation["known_buttons"]), None)
                core.require(label is not None and page.click(label), "composer_navigation_unknown")
            time.sleep(1)
        core.require(page.observe()["editors"] == 1, "composer_not_ready")
        page.insert_text(f"Reply briefly without using tools. Correlation token: {self.fixture.nonce}")
        if not any(page.click(label) for label in ("Send", "Send message", "Submit")):
            page.enter()
        core.wait_for(lambda: self.fixture.report()["partial_sent"], "fixture_request_not_received", seconds=60)
        core.wait_for(lambda: page.output_visible(self.fixture.partial_marker), "partial_not_rendered", seconds=30)
        core.require(not self.fixture.report()["terminal_sent"], "terminal_sent_before_gui_partial")
        self.fixture.release_terminal()
        core.wait_for(lambda: page.output_visible(self.fixture.final_marker), "final_not_rendered", seconds=30)
        report = self.fixture.report()
        core.require(report["terminal_sent"] and report["accepted_response_count"] == 1,
                     "fixture_terminal_missing")
        core.require(all(report[field] >= 1 for field in ("auth_match_count", "model_match_count",
                     "reasoning_match_count", "input_nonce_match_count")), "fixture_contract_not_matched")
        print("headroom-desktop-phase=controlled_gui_partial_and_final_rendered", flush=True)
        return {"partial_visible_before_terminal": True, "final_visible": True, "fixture": report,
                "auth_source": "disposable_synthetic_api_key", "live_provider": "not_tested"}


def metrics_observed(state, model):
    # The real collector authenticates over Unix and persists content-free events.
    # Read only counts; do not bypass its private /stats control header.
    database = state / "metrics/metrics.sqlite3"
    if not database.exists():
        return None
    info = database.lstat()
    core.require(database.resolve(strict=True) == database and stat.S_ISREG(info.st_mode)
                 and info.st_uid == os.geteuid() and info.st_nlink == 1
                 and info.st_size <= 64 * 1024 * 1024, "unsafe_metrics_database")
    try:
        with sqlite3.connect(database.as_uri() + "?mode=ro", uri=True, timeout=1) as connection:
            total, matched = connection.execute(
                "SELECT count(*), coalesce(sum(json_extract(payload, '$.attribution.model') = ?), 0) FROM events",
                (model,)).fetchone()
    except sqlite3.OperationalError:
        return None
    return {"model_matches": True, "matched_rows": matched, "stored_events": total,
            "source": "actual_headroom_collector"} if matched else None


def fixture():
    module = load("_desktop_controlled_provider", "scripts/macos-desktop-fixture.py")
    return module.DesktopResponsesFixture("sk-local-fixture-only-" + uuid.uuid4().hex,
        "gpt-5.2-codex", "medium", uuid.uuid4().hex)


def direct_diagnostic(vendor):
    provider = fixture()
    try:
        desktop.run(vendor, ControlledTurn(provider, provider.base_url, via_headroom=False))
    finally:
        core.require(provider.close(), "fixture_cleanup_incomplete")
        print("headroom-desktop-fixture=" + json.dumps(provider.report(), separators=(",", ":")), flush=True)


def run(package, vendor):
    core.require(sys.platform == "darwin" and os.environ.get("GITHUB_ACTIONS") == "true", "ci_macos_required")
    core.require(package.is_absolute() and package.resolve(strict=True) == package
                 and package.parent == (ROOT / "work").resolve(strict=True)
                 and package.name.startswith("macos-package."), "invalid_package_root")
    core.require(package.stat().st_uid == os.geteuid() and package.stat().st_mode & 0o777 == 0o700,
                 "unsafe_package_root")
    bundle = package / "Headroom.app"
    with (bundle / "Contents/Info.plist").open("rb") as stream:
        core.require(plistlib.load(stream)["CFBundleIdentifier"] == "dev.headroom.desktop.h048c5", "invalid_qa_bundle")
    executable = (bundle / "Contents/MacOS/headroom-desktop").resolve(strict=True)
    runtime = bundle / "Contents/Resources/runtime"
    state = package / "state"
    info = state.lstat()
    core.require(state.resolve(strict=True) == state and stat.S_ISDIR(info.st_mode)
                 and info.st_uid == os.geteuid() and stat.S_IMODE(info.st_mode) == 0o700
                 and not list(state.iterdir()), "qa_state_not_clean")
    before = core.bundle_inventory(bundle, Path(os.environ["HEADROOM_MACOS_RUNTIME_ROOT"]))
    binary_before = hashlib.sha256(executable.read_bytes()).hexdigest()
    provider = fixture()
    app, engine, launcher, port, result = None, None, None, None, None
    try:
        # The normal pinned settings store supplies the operator's local upstream.
        # No production runtime shim or user's client configuration is modified.
        (state / "runtime").mkdir(mode=0o700)
        workspace = state / "runtime/workspace"
        workspace.mkdir(mode=0o700)
        settings = workspace / "settings.json"
        settings.write_text(json.dumps({"openai_base_url": provider.origin}) + "\n", encoding="utf-8")
        settings.chmod(0o600)
        core.require(core.find_app(executable) is None, "qa_app_already_open")
        launcher = subprocess.Popen(["/usr/bin/open", "-n", "-W", "-a", str(bundle)], cwd=package,
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        app = core.wait_for(lambda: core.find_app(executable), "headroom_launch_timeout")
        core.run_ui("open")
        port = core.wait_for(lambda: core.endpoint_owned(app, state), "headroom_endpoint_timeout")
        core.run_ui("start")
        engine = core.wait_for(lambda: core.find_engine(app, runtime), "headroom_engine_missing")
        core.require(not core.lock_available(state / "runtime/.engine.lock"), "headroom_engine_unowned")
        print("headroom-desktop-phase=packaged_headroom_ready", flush=True)
        result = desktop.run(vendor, ControlledTurn(provider, f"http://127.0.0.1:{port}/v1"))
        metrics = core.wait_for(lambda: metrics_observed(state, provider.model), "headroom_request_metrics_missing")
        core.run_ui("exit")
        core.wait_for(lambda: not core.matches_process(app), "headroom_exit_timeout")
        core.wait_for(lambda: not core.matches_process(engine), "headroom_engine_left")
        core.wait_for(lambda: core.port_released(port), "headroom_listener_left")
        core.require(launcher.wait(timeout=10) == 0, "headroom_launcher_unclean")
        core.require(core.lock_available(state / "runtime/.engine.lock")
                     and core.lock_available(state / "runtime/.ingress.lock"), "headroom_lock_left")
        after = core.bundle_inventory(bundle, Path(os.environ["HEADROOM_MACOS_RUNTIME_ROOT"]))
        core.require(before == after and hashlib.sha256(executable.read_bytes()).hexdigest() == binary_before,
                     "headroom_package_changed")
        result.update(headroom_metrics=metrics, packaged_headroom_normal_exit=True, headroom_engine_exited=True,
                      headroom_listener_released=True, headroom_bundle_before=before, headroom_bundle_after=after,
                      model="gpt-5.2-codex", reasoning_effort="medium")
    finally:
        if app and core.matches_process(app) and engine is None:
            engine = core.find_engine(app, runtime)
        for owned in (app, engine):
            if owned and core.matches_process(owned):
                os.kill(owned[0], 15)
                try:
                    core.wait_for(lambda: not core.matches_process(owned), "owned_cleanup_timeout", seconds=10)
                except core.ProbeError:
                    if core.matches_process(owned):
                        os.kill(owned[0], 9)
                        core.wait_for(lambda: not core.matches_process(owned), "owned_forced_cleanup_timeout", seconds=5)
        if launcher and launcher.poll() is None:
            launcher.terminate()
            launcher.wait(timeout=5)
        core.require(provider.close(), "fixture_cleanup_incomplete")
        print("headroom-desktop-fixture=" + json.dumps(provider.report(), separators=(",", ":")), flush=True)
    print("headroom-desktop-traffic=" + json.dumps(result, separators=(",", ":")), flush=True)


if __name__ == "__main__":
    try:
        core.require(len(sys.argv) == 3, "package_and_desktop_roots_required")
        if sys.argv[1] == "--direct-fixture":
            direct_diagnostic(Path(sys.argv[2]))
        else:
            run(Path(sys.argv[1]), Path(sys.argv[2]))
    except (core.ProbeError, desktop.cdp.CdpError) as error:
        print(f"headroom-desktop-error={error}", file=sys.stderr)
        sys.exit(2)
    except (OSError, ValueError, KeyError, subprocess.SubprocessError):
        print("headroom-desktop-error=controlled_probe_failed", file=sys.stderr)
        sys.exit(2)
