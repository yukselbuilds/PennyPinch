# H010-a — OpenCode V1 local fixture transport

Status: **local proof PASS** on the attached Windows 10 host, 2026-10-19. This is one tools-disabled OpenCode CLI request through the pinned Headroom proxy to a local synthetic upstream. It is not a full H010 compatibility result.

14 Kasım takip: Gerçek CLI araç/CCR testleri, ek yerel MCP köprüsü tanımlanmış izole profilde iki protokolde geçti. Endpoint-only varsayılan profillerde otomatik geri alma geçmedi. Aşağıdaki H010-a tarihsel tools-kapalı kapsamı değişmez; yeni koşullu teknik kanıt ve açık ürün işi belgenin sonundadır.

## Observed behavior

The pinned OpenCode V1 CLI 1.19.3 sent one streamed `POST /v1/chat/completions` through Headroom 0.41.0 to the loopback fixture. The fixture saw model `headroom-opencode-fixture`, `stream=true`, the per-process fake API sentinel, and zero tools. It returned five OpenAI-compatible SSE frames: assistant role, text, finish reason, usage, and `[DONE]`. The CLI emitted `step_start`, a completed `text` part containing the fixture marker, and `step_finish`; the CLI reported 12 input and 4 output tokens and exited 0. The fixture recorded one total request and one chat-completion request.

This does not establish token-by-token latency or that CLI text was visible before the upstream terminal frame. The pinned [V1 `run.ts` output path](https://github.com/anomalyco/opencode/blob/v1.19.3/packages/opencode/src/cli/cmd/run.ts#L710) publishes a JSON `text` event after the text part has ended, so the test asserts the completed text marker and terminal usage only.

The test used an inline, process-only V1 config: only the `headroom-fixture` provider was enabled; external plugins and MCP were empty; built-in tools were disabled and wildcard permission denied; `--pure` was passed. Its API key was a synthetic sentinel. No real model, provider credential, OAuth session, desktop/TUI session, or user configuration participated. No reasoning variant was selected or claimed. The V1 config and custom provider shape follow the official [configuration](https://opencode.ai/docs/config/), [provider](https://opencode.ai/docs/providers/), and [tool](https://opencode.ai/docs/tools/) documentation; V2 behavior is not covered.

Headroom readiness was `0.41.0`, and its PID matched the loopback listener owner. `/health` reported the fixture as the OpenAI upstream, coding savings profile, cache mode, cache and optimize enabled, and effective `HEADROOM_OUTPUT_SHAPER=0`. `/v1/retrieve/stats` reported nested `store.default_ttl_seconds=7200`. The child engine received `HEADROOM_MODEL_ROUTER_ENABLED=0`, `HEADROOM_TELEMETRY=on`, and `HEADROOM_BEACON=off`; these three values are recorded as process environment settings, not as separately introspected health fields. The upstream readiness probe was disabled for this fixture run so the engine made no external readiness request.

The engine's normal control path could not use Ctrl+Break from this proof runner (`controller-console-missing`). The owned engine was terminated through the H006 process handle; exit code was 1, the owned port closed, and the new temporary run root was removed. This is a force-fallback cleanup result, not a graceful-shutdown pass. The CLI itself completed normally with exit 0.

## Reproduction

Run these focused checks from `D:\Projeler\Headroom` in PowerShell:

```powershell
& work\proofs\packaging\artifacts\headroom-engine\runtime\python.exe -I -m unittest discover -s tests\fixtures\opencode -p test_opencode_proof_helpers.py -v
& work\proofs\packaging\artifacts\headroom-engine\runtime\python.exe -I work\proofs\opencode\run_opencode_proof.py
```

The test command reports four tests `OK`. The proof runner verifies the pinned archive and executable hashes before launch, creates a fresh UUID state/workspace/config tree with spaces in the path, captures bounded metadata only, and removes only that run's resolved child directory after owned processes and ports are closed. No raw CLI or engine failure log is persisted. The H010 run root count after the successful run was zero.

The child processes receive a scrubbed environment. OpenCode uses only the isolated HOME, XDG, config, temp, and workspace directories; its PATH is limited to the pinned CLI directory and Windows `System32`. `OPENAI_API_KEY` contains only the synthetic sentinel. Headroom binds `127.0.0.1` on a dynamic port and targets the local fixture. Local telemetry was explicitly configured on and anonymous beacon shipping off.

## Fingerprints

Host: Microsoft Windows 10 Pro 22H2, 10.0.19045, x64. Windows 11, a clean machine, and Tauri process wiring were not tested.

Pinned OpenCode asset: baseline ZIP, 62,204,138 bytes, SHA-256 `0d59113484bfe8d6dc146fc981f5c33faa8318303c183711b7a9387060b289e8`; extracted executable reports version `1.19.3`, SHA-256 `cab0c9a8480f71fbb7cbc88ad91057712ecb500ab41fb9c01dd586cb9d8ea375`. The runner checks both hashes before execution.

Pinned engine: `headroom-ai[proxy]==0.41.0`, CPython 3.13.13 x64. H006 packaged helper and runtime manifest hashes are checked before launch. The command fingerprint is the runner SHA-256; the inline config schema/model/provider and child environment are defined in that same runner. The selected fixture model ID is a test label, not a claim about an actual provider model.

| Input | SHA-256 |
| --- | --- |
| `work/proofs/opencode/run_opencode_proof.py` | `0d6ff45ed4ea359c2ccb98111c7b8759ecad482530c210a6f5169725a2333d27` |
| `tests/fixtures/opencode/openai_fixture.py` | `6057e56ec426abd5b95e51518c346b9e095029eeef2048986ac54e696eb5fcbe` |
| `tests/fixtures/opencode/test_opencode_proof_helpers.py` | `3fb4a9fbbad66b4e699ecb0a5745066f3d76435ba6f704d107c566c69db4d293` |
| packaged `headroom_runtime.py` | `bf887b6f783f4385debc84836353340f39e3773433782305376a3247be0f4ac1` |
| packaged `runtime-manifest.json` | `8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908` |
| engine lock (`uv.lock`) | `161d3b1543b1f6620c1ce40e38d9a2fba26d5c64e3e103a41dcd81dff18ced3f` |
| pinned `proxy/server.py` | `e80fdf59cdede96bfd86541ec22ee4aa98f5d7a3cea8b34e316cd0628c01f8d2` |
| pinned `proxy/handlers/openai.py` | `05f624cd0c704e9b8ba4b332284cd7fe7e81708f7e237da123fc216aabc8b4af` |
| pinned `agent_savings.py` | `3d2c7c7035ff0c4765c04b873d6a22a7f75c79627ba0bf76b47718fd10a46502` |
| pinned `cache/compression_store.py` | `1f269d508abe7590f0634654832bd6fd45786dc8f3e569d983966648c87b1a73` |
| pinned `proxy/runtime_env.py` | `38454fa8331c81158fa7c8e34ccadc0ea0499980ab0d37e6eab44d4d98e95392` |

## Limits

This slice covers the V1 CLI's ordinary tools-off API transport with a synthetic OpenAI-compatible backend. It does not cover real credentials or inference, ChatGPT auth, tool execution, agents/MCP, TUI/desktop, V2 config, first-token streaming, graceful engine shutdown from the GUI, Windows 11, or clean-machine packaging. H010 remains open for those client paths and product integration.

## H010-b — Araç ve CCR yolları

14 Kasım 2026. Windows 10 build 19045 x64, aynı pinned OpenCode 1.19.3 binary ve Headroom 0.41.0 ile iki protokol ayrı sınandı. **Kabul koşulludur:** Pozitif koşularda ayrı test profiline dar yerel retrieval MCP köprüsü açıkça eklendi. Mevcut Headroom Bağlan eylemi yalnız endpoint ayarını yönetiyor; bu köprüyü ürün olarak kurmuş veya varsayılan araç/CCR kabulünü geçmiş sayılmaz. Ana H010/42–54 açık kalır.

İlk endpoint-only Chat `9519a08f...` ve Responses `f46496df...` koşularında gerçek veri aracı tamamlandı, özgün 14.999 bayt motora ulaştı ve sıkıştırma marker'ı oluştu; otomatik retrieval aracı bildirilmediği için iki koşu da başarısız kaldı. Pinned `proxy/handlers/openai.py::_should_inject_openai_chat_ccr_tool`, streaming Chat'te continuation olmadığından otomatik araç eklemeyi bilerek reddeder. Responses ise zaten bildirilmiş headroom_retrieve aracı varsa buffered server-side continuation kullanır. Bu farklı gereklilikler genel OpenCode uyumluluğuna yükseltilmez.

Sonraki test konfigürasyonu, tek sabit log/tanık aracı ile yalnız hash alıp sahip engine'in loopback `/v1/retrieve` adresine erişen ikinci stdio MCP sunucusunu tanımlar. URL/path/shell veya auth kabul etmez. [Sabit OpenCode local MCP şeması](https://github.com/anomalyco/opencode/blob/e3c4eb242585dcd77be1a867391b7c25fd47ce52/packages/core/src/v1/config/mcp.ts) ve [araç adlandırma kaynağı](https://github.com/anomalyco/opencode/blob/e3c4eb242585dcd77be1a867391b7c25fd47ce52/packages/opencode/src/mcp/catalog.ts) test yapılandırmasının temelidir; kaynaklar `work/research/h010-mcp/manifest.json` içinde hash'leriyle sabittir. Gerçek çalışma kanıtı bunlardan ayrı aşağıdaki iki koşudur.

| Koşul | Chat Completions | Responses |
| --- | --- | --- |
| Yapılandırılan selector / wire model | lab/slot / headroom-opencode-tool-fixture | openai/slot / gpt-4.1 |
| Provider SDK | @ai-sdk/openai-compatible | @ai-sdk/openai |
| Retrieval bildirimi | Test profilinde açıkça tanımlı MCP köprüsü | Aynı açık test MCP bildirimi |
| Geri alma işlemi | OpenCode gerçek MCP tools/call yapar | Motor server-side işler; retrieval MCP tools/call yapılmaz |
| Native istek / upstream mesajı | 4 / 4 | 3 / 4 |
| Native araç çağrıları | Veri, retrieval, tanık; hepsi completed | Veri ve tanık; ikisi completed |
| Sonuç | Final/CLI exit0/stderr0 | Final/CLI exit0/stderr0 |

İki kolda native istemci streaming ister; Responses kolundaki dört upstream mesajı motor tarafından `stream:false` ile buffer edilir. Chat upstream'i streaming kalır. Fragment'lı tool arguments, call_id ve nihai text gerçek SDK/CLI'den geçti. Native girişte 14.999 bayt özgün sonuç korunur; upstream'te 103 bayt olur. CCR JSON hash/original_content, girişte görülen wire string ve sentetik logla SHA/bayt düzeyinde aynıdır. Model selector/wire ayrımı korunur; reasoning alanı seçilmediği için herhangi bir reasoning özelliği desteği iddia edilmez. Model isimleri sentetik sağlayıcı test girdileridir; gerçek çıkarım veya ürün model allowlist'i değildir.

Nihai kayıtlar Responses `work/proofs/opencode-tools/runs/5f4cdcee-f73f-4533-9ddf-9ea19f94960a/result.json` ve Chat `.../732cb0fe-23a1-4d03-909b-da5e9c69cba3/result.json`. Beş odaklı test; yanlış call_id/marker/hash/içerik/model/reasoning ve Responses biçim dönüşümünü denetler. Üç ek sınır kontrolü geçersiz hash/araç/ek URL alanını ağa çıkmadan reddeder ve pinned Chat streaming policy'sinin değişmediğini doğrular. Kaynak/CLI/motor/tokenizer ve araç fingerprint'leri önce/sonra aynı. Her kolda motor graceful exit0, sahip port kapalı ve force fallback yoktur. [Programatik kabul](opencode-tools.json), [inceleme](../reviews/H010-b.md).

H009-b'nin sabit MCP veri sunucusu/BoundedOutput ve H041'in controller/helper altyapısı değiştirilmeden yeniden kullanıldı. HF/transformers offline, cache/coding, CCR TTL7200, yerel ölçüm açık ve beacon/router/output-shaper/memory/learn kapalıdır. Sentetik testte protected-tools boş listesi kullanılır; bu dilim gerçek Read kaynağı/sayısal korumasını yeniden sınamaz. Proxy/offline ayarları kayıttır, packet capture yapılmadı. Kullanıcının auth/config'i, aktif sohbetleri ve model/efor/ajan sayısı değiştirilmedi; Actions/kurulum/yayın veya ücretli çağrı yoktur.

Ürün kaynakları ve eski kapıdaki 325 girdi/araçlar aynı olduğundan 441 Rust, 221 Node, fmt ve frontend/embedded build kanıtları ilgili kapsamda yeniden kullanıldı. Yalnız yeni benchmark/fixture doğrulandı. Tek blok byte azalması toplam token, cache indirimi, maliyet, TTFT veya model kalitesi kazanımı değildir; CCR özgün içeriği tekrar taşır. Her protokolde bir olumlu koşu genel kararlılık veya tüm sürüm/model/alt ajan desteği sayılmaz.

### Tekrarlama ve açık ürün işi

Pinned payload/cache ve proje içi 1.19.3 executable gerekir. Repo kökünde; yalnız ilgili fingerprint değiştiğinde başarılı kontroller yenilenir:

```powershell
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B -m unittest discover -s tests/fixtures/opencode -p test_tool_fixture.py
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B bench/opencode/roundtrip.py --run --surface responses --with-retrieval-bridge
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B bench/opencode/roundtrip.py --run --surface chat --with-retrieval-bridge
```

Bayrak açıkça test-only köprü bildirimi ekler; kullanıcı ayarına kurulum yapmaz. Bridge'siz koşu bilinçli olarak başarısız başlangıç kanıtıdır; rutin başarılı smoke diye tekrar çalıştırılmaz.

**H010-b3 Windows 10 yerel ürün kabulü tamamlandı:** Ürünün Önizle → Uygula akışı pinned retrieval köprüsünü native plan/receipt üzerinden atomik kurar; kullanıcı değişikliklerini koruyan Undo, duraklatma/yeniden açılış ve release kaldırma girişi doğrulandı. İki protokol gerçek native UI → kaydedilmiş kullanıcı JSONC'si → izole OpenCode CLI ile geçti; test bridge bayrağı veya inline endpoint override kullanılmadı. Ayrıntı ve sınırlar [ürün retrieval kanıtında](opencode-retrieval.md), [programatik sonuçta](opencode-retrieval.json) ve [incelemede](../reviews/H010-b3.md). Yukarıdaki koşullu b2 kayıtları tarihsel kapsamını korur. Canlı sağlayıcı/auth, gerçek alt ajan, TUI/V2/Desktop, temiz Windows/Windows 11 ve diğer OS açık olduğundan ana H010 tamamlandı veya bütün OpenCode yolları destekleniyor denmez.
