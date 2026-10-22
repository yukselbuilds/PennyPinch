# H005 — Headroom runtime proof

Status: PASS on the attached Windows 10 host, 2026-10-19. This is local development evidence, not a Windows 11 or clean-machine result.

The runner starts the real `headroom-ai[proxy]==0.41.0` CLI in a disposable project-owned environment, selects an unused IPv4 loopback port, and uses temporary HOME, workspace, config, and cache paths. It accepts readiness only when `/readyz` reports the pinned version and the exact `Popen` PID owns the `127.0.0.1` listener. `/stats` confirms local telemetry is enabled and anonymous beacon shipping is disabled. No client configuration or other project session is read or changed.

The proof also starts a real contender against a deliberately occupied port. The contender exits without replacing the test listener; that listener stays bound and answers `/readyz`. A healthy-looking listener with the same payload is rejected when its PID differs from the managed engine PID. On shutdown, the runner sends Ctrl+Break to its private process group; its bootstrap forwards that event to SIGINT so Uvicorn can complete its lifespan. Python documents the Windows process-group event contract for [`send_signal`](https://docs.python.org/3/library/subprocess.html#subprocess.Popen.send_signal) and [`SIGBREAK`](https://docs.python.org/3/library/signal.html#signal.SIGBREAK). The process exits 0, Uvicorn reports `Application shutdown complete`, Headroom emits its session summary, and the owned port is closed. The force-termination fallback is unused on PASS.

## Reproduction

From `D:\Projeler\Headroom` in PowerShell:

```powershell
uv run --locked --project work\proofs\engine --python work\proofs\engine\toolchain\cpython-3.13.13-windows-x86_64-none\python.exe --cache-dir work\proofs\engine\uv-cache python work\proofs\engine\test_engine_proof.py
uv run --locked --project work\proofs\engine --python work\proofs\engine\toolchain\cpython-3.13.13-windows-x86_64-none\python.exe --cache-dir work\proofs\engine\uv-cache python work\proofs\engine\engine_proof.py
```

The focused unit check reports `Ran 4 tests ... OK`. The runtime runner reports:

```text
PASS pinned engine readiness and owned loopback PID
PASS local telemetry enabled and anonymous beacon disabled
PASS busy port refused; foreign test listener remained alive and bound
PASS healthy-looking wrong listener rejected by PID ownership
PASS controlled shutdown exit=0; lifespan complete; owned port closed; force fallback unused
```

## Fingerprints

- Host: Microsoft Windows 10 Pro 22H2, 10.0.19045, x64. Windows 11 and a clean machine were not tested.
- Toolchain: CPython 3.13.13 x64, project-local standalone base interpreter; `uv 0.11.9`; locked `headroom-ai 0.41.0`, `uvicorn 0.54.0`, and `click 8.5.0` (95 locked packages).
- Pin verification: [PyPI headroom-ai 0.41.0](https://pypi.org/project/headroom-ai/0.41.0/); source archive SHA-256 `b24e6b33becfa767d7b82bce02b9ca06f05ce2a3daf1392f8e591947a9c5bf28`; Windows x64 wheel SHA-256 `18b43230c4dbbe9889fdb0863a7943059f4509fdf49ad20b19be394f2725db18`.
- Isolation: `-I`; the runner uses the project toolchain's base `python.exe` and explicitly adds only `.venv\Lib\site-packages`. This bypasses the Windows venv launcher, whose PID differs from the Python PID that owns the listener. Its environment allowlist supplies temporary HOME/TEMP/APPDATA, workspace, and config directories; `HEADROOM_HOST=127.0.0.1`, a dynamically selected port, `HEADROOM_TELEMETRY=on`, `HEADROOM_BEACON=off`, `HEADROOM_UPDATE_CHECK=off`, and `HEADROOM_LOG_LEVEL=info`.
- Command fingerprint: the two commands above, run from the project root with `uv run --locked`; successful output is summarized here rather than saved as raw logs.

SHA-256 fingerprints for the final proof inputs and the pinned startup/shutdown source:

| File | SHA-256 |
| --- | --- |
| `work/proofs/engine/pyproject.toml` | `7d9802162b931a2c2b8ceb08852f44c587668290d1763eaec49786d089aeeaa9` |
| `work/proofs/engine/uv.lock` | `161d3b1543b1f6620c1ce40e38d9a2fba26d5c64e3e103a41dcd81dff18ced3f` |
| `work/proofs/engine/engine_proof.py` | `52b5a7bc6c33cbe0ecb49c5496e70bb9efbac61ce7cad645503eaf9591df1f6b` |
| `work/proofs/engine/test_engine_proof.py` | `5a1d6ec88cb0e0ac713c79a58fd95cb08b29396cf4c8666ceb9ed78ca8c6622c` |
| project CPython `python.exe` | `282c2635fbd49b4aeefda4a965d84a745508c87e6e5eb95947efd0b5a1c1fb07` |
| installed `headroom/cli/proxy.py` | `19713bfbb09f6506ad51035925eaee20342ab6834294535047d90a3b81d57a74` |
| installed `headroom/proxy/server.py` | `e80fdf59cdede96bfd86541ec22ee4aa98f5d7a3cea8b34e316cd0628c01f8d2` |
| installed `uvicorn/server.py` | `c1be9d133467a42ab88d7168e408ef5a90257661a901899426214a3d1fc2d026` |
| installed `uvicorn/lifespan/on.py` | `8844cf163e921613c33dc75e1b17dae372ca226254ecc2398bb41451ea4170f1` |

The app-level evidence covers startup/readiness, listener ownership, telemetry flags, occupied-port refusal, wrong-PID rejection, and graceful shutdown on this host. The shutdown signal mapping was exercised from the proof runner's console context with `CREATE_NEW_PROCESS_GROUP`; it does not establish behavior from a console-less Tauri GUI process or a clean packaged runtime. H006/H014 should verify that process lifecycle separately. No upstream inference request was sent; this proof does not claim whole-process offline behavior or validate external model-asset downloads.
