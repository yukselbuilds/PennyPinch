# Local tokenizer dictionaries

During H029 the real engine waited for missing tiktoken dictionaries on the first token count. The pinned Headroom loader could wait 10 seconds per encoding. One cold diagnostic request took 20.080 seconds. The existing 5-second health observation could go stale during that wait and cause the engine to lose its active status. This observation covers the recorded cold request.

The fix bundles the engine's data dependencies. The six public URL and SHA-256 pairs in `runtime/desktop_controller.py` come from `tiktoken_ext/openai_public.py` in the pinned tiktoken package. Cache filenames follow tiktoken's SHA1(URL) rule and SHA-256 verifies the contents. The data stays under `work/proofs/packaging/artifacts/headroom-engine/runtime/tiktoken-cache`, outside Git and the user profile's cache.

Run the preparation command from the repository root when building or packaging the project.

```text
python scripts/prepare-tokenizers.py
```

The first preparation downloads the pinned public data over HTTPS. It reuses valid files and reports corrupt existing files. Each file is limited to 10 MiB and published atomically after verification. Then the packaged Python loads seven built-in encodings and tokenizes a sample with socket connections blocked. The six source files total 8,465,407 bytes of tokenizer data.

Use the check command to verify the prepared cache offline.

```text
python scripts/prepare-tokenizers.py --check
```

Before each new engine process, the helper verifies every file and sets `TIKTOKEN_CACHE_DIR` for that process only. Missing or corrupt data prevents the ready state. The change preserves the user's model, reasoning and authentication settings, the coding/cache profile and the health and stop time budgets. Acceptance covers the seven built-in tiktoken encodings.

Initial preparation and offline smoke evidence is in `work/proofs/tokenizers/prepare-95e27441-9279-49bc-bb90-c3d5e8c58008.json`, with before-and-after input hashes and the six artifact checksums. H038 and H041 track inclusion of this cache directory and the existing third-party licenses in the installer and runtime. The product was a local development candidate at this checkpoint. End users receive the prepared data with the package.
