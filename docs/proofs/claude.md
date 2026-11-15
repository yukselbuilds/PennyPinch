# Claude Code local transport proof (H009-a)

15 Kasım 2026 güncel ana kabulü: [H009 tamamlandı, yerel Windows 10 teknik kapsamı](claude-acceptance.md). Aşağıdaki H009-a ve H009-b sonuçları kendi tarihsel profilleriyle korunur. Canlı hesap/sağlayıcı, alt ajan ve eklenti desteği eklenmedi.

**Result:** PASS for the local, tools-disabled API-fixture stream path on the Windows 10 22H2 x64 host (10.0.19045). This is a synthetic transport proof; it does not establish live Anthropic/model behavior or complete H009.

14 Kasım takip: Gerçek CLI ile açık tek MCP aracı ve CCR devam döngüsü ayrıca kabul edildi; aşağıdaki H009-a tarihsel tools-kapalı kapsamı değişmez. Yeni H009-b kapsamı belgenin sonundadır; canlı Anthropic/OAuth ve ana H009 açık kalır.

## What was exercised

Claude Code CLI 2.1.158 was started with a disposable profile and a synthetic API-key sentinel. Its `ANTHROPIC_BASE_URL` targeted the pinned Headroom 0.41.0 runtime on loopback; the engine forwarded to a loopback-only Anthropic SSE fixture. The packaged H006 runtime helper (`runtime/headroom_runtime.py`) started and owned the engine listener. No user credentials, settings, sessions, client configuration, or persistent state were copied or changed.

The fixture observed exactly one `/v1/messages` request with the selected fixture label `claude-opus-4-6`, `effort=high`, streaming enabled, zero tools, the sentinel credential, and an Anthropic version header. The model label is a test input and does not describe the user's model. The request's `thinking` object was present; this proof does not claim that its value was preserved.

The fixture sent a partial text delta containing the marker, then held the terminal SSE events until the CLI emitted that marker in a `stream_event`. Only then did it send `message_stop`. The CLI reported a successful final result, exited 0, and exposed terminal usage of 12 input and 4 output tokens. The usage parser prefers the direct `usage` totals when the duplicate `modelUsage` breakdown is also present; the focused tests cover that case, breakdown-only results, missing usage, and the partial-stream gate.

Effective engine settings were cache mode, coding profile, cache/optimization enabled, CCR TTL 7200 seconds, output shaper explicitly off, model router off, local telemetry on, and anonymous beacon off. The engine PID and loopback listener PID both were 10444 during this run. Cleanup stopped the CLI and fixture, removed the isolated run directory, and confirmed the owned engine port closed. Graceful shutdown was not established: the consoleless controller reported `controller-console-missing`, used the owned-process force fallback, and observed engine exit code 1. This does not establish Tauri GUI lifecycle behavior; that remains for H014.

## Reproduction

Focused helper tests:

```powershell
& 'work/proofs/packaging/artifacts/headroom-engine/runtime/python.exe' -I -m unittest discover -s 'tests/fixtures/claude' -p 'test_claude_proof_helpers.py' -v
```

One final end-to-end run:

```powershell
$claude = (Get-Command claude -ErrorAction Stop).Source
& 'work/proofs/packaging/artifacts/headroom-engine/runtime/python.exe' -I 'work/proofs/claude/run_claude_proof.py' --claude-exe $claude
```

Both commands passed for the fingerprinted candidate below. No raw CLI or failure logs are persisted.

## Fingerprints

| Item | SHA-256 / value |
|---|---|
| E2E command fingerprint | `8ea21e21cc615a667775c90e1e7e1eeb3d20d28458e07138fb2bfee0a7f8a4a4` |
| Isolated config fingerprint | `450f492af397f3cee74e98cd8b9d5baa54263e53183352807ef39cc9a01d3b52` |
| `work/proofs/claude/run_claude_proof.py` | `6034f8b65d162bd1824358e724f5c5e7c41699605d2ff4a0672902741146a120` |
| `tests/fixtures/claude/anthropic_fixture.py` | `e69c49f33437f679b3f271102a95db01354d9f075a4a7f7bf0572d6ebc2112a5` |
| `tests/fixtures/claude/test_claude_proof_helpers.py` | `64efada3c14ca2403206c3d57c5e890a47280ea8e848f8c5d000936a3b8a9f40` |
| Packaged Headroom helper | `bf887b6f783f4385debc84836353340f39e3773433782305376a3247be0f4ac1` |
| Packaged runtime manifest | `8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908` |
| Engine source `pyproject.toml` | `7d9802162b931a2c2b8ceb08852f44c587668290d1763eaec49786d089aeeaa9` |
| Pinned engine lock | `161d3b1543b1f6620c1ce40e38d9a2fba26d5c64e3e103a41dcd81dff18ced3f` |
| CPython 3.13.13 x64 executable | `282c2635fbd49b4aeefda4a965d84a745508c87e6e5eb95947efd0b5a1c1fb07` |
| Claude Code CLI 2.1.158 executable | `10fa305545b20baf6e074a086762bd252b89dfe7035848a5d41385503b1a6c74` |
| Pinned `headroom/proxy/server.py` | `e80fdf59cdede96bfd86541ec22ee4aa98f5d7a3cea8b34e316cd0628c01f8d2` |
| Pinned `headroom/proxy/handlers/anthropic.py` | `a9e4ef859e2d7a5990db72117159751fb1b16a072bb047c503ceddf4d5ffd467` |
| Pinned `headroom/agent_savings.py` | `3d2c7c7035ff0c4765c04b873d6a22a7f75c79627ba0bf76b47718fd10a46502` |
| Pinned `headroom/cache/compression_store.py` | `1f269d508abe7590f0634654832bd6fd45786dc8f3e569d983966648c87b1a73` |

The runtime manifest identifies `headroom-ai[proxy]==0.41.0`, CPython 3.13.13 x64, 102 locked packages, and the package/source fingerprints above. The runtime tree is 22,225 files / 523,394,231 uncompressed bytes; that is the existing packaged runtime size, not a separate Claude bundle-size measurement.

## Boundaries

This proves only the local synthetic Anthropic API-fixture path on the recorded Windows 10 host. It does not prove a live Anthropic request, inference from the selected fixture model label, preservation of the `thinking` value, user OAuth/auth compatibility, Claude Desktop, enabled tools, hooks, plugins, MCP, subagents, a clean machine, Windows 11, or Tauri's consoleless lifecycle. Tools were deliberately disabled; no paid provider call was made. The fixture and temporary state were local, while any unrelated engine asset egress is outside this proof.

Claude's gateway/base URL configuration is described in [Anthropic's LLM gateway documentation](https://docs.anthropic.com/en/docs/claude-code/llm-gateway). Nonessential traffic suppression is described in [Claude Code data usage documentation](https://code.claude.com/docs/en/data-usage).

## H009-b — Gerçek CLI araç ve CCR döngüsü

14 Kasım 2026. Windows 10 build 19045 x64'te Claude Code **2.1.158**, aynı ölçülmüş executable hash'i ve pinned Headroom **0.41.0** ile yerel dilim kabul edildi. Gerçek model/hesap kullanılmadı; upstream, protokolü uygulayan sentetik loopback fixture'dır. Model etiketi `claude-opus-4-6` ve `high` test girdisidir; ürün başka modele/efora geçmez veya destek kapsamını bu etikete bağlamaz.

Gerçek CLI, yerleşik araçlar kapalı ve yalnız `mcp__headroom_fixture__inspect_logs` izinli olacak şekilde izole profilde başlatıldı. Stdio MCP sunucusunun tek aracı sabit sentetik log veya kısa tanık döndürür. CLI çıktısının motora girmeden önceki ve upstream'e ulaştıktan sonraki temsilleri ayrı gözlenir; raw gövdeler rapora yazılmaz.

| Aşama | Beklenen ve gözlenen |
| --- | --- |
| İlk client araç çağrısı | Fragment'lı SSE tool_use gerçek CLI'de aynı kimlik/argümanla MCP tools/call oldu. |
| Özgün sonuç | CLI → test ingress → motor yolunda 120 satır / 14.999 bayt özgün log birebir aynıydı. |
| Sıkıştırma | Aynı tool_result kimliğiyle upstream'e 103 bayt ve 24 haneli CCR marker ulaştı; retrieval araç tanımı vardı. |
| Geri alma | Upstream'in tek headroom_retrieve çağrısını motor işledi. JSON hash/original_content zarfındaki özgün içerik SHA-256 ve bayt düzeyinde eşleşti. |
| Devam | CCR'den sonra ikinci gerçek MCP çağrısı/tanığı ve beklenen final tamamlandı; CCR ayrı client aracı olarak sızmadı. |
| Kapanış | CLI exit0/stderr0; motor graceful exit0, sahip port kapalı ve force fallback yok. |

Toplam **3 native Messages isteği / 4 upstream Messages mesajı** vardır. Sırasıyla upstream stream değerleri `true, false, false, true`: CCR için motorun buffered ve server-side continuation yolu kullanıldı; native istemci üç istekte de streaming istedi. Model/efor/thinking ve sentetik auth her upstream aşamada korundu. Bu deney ilk token gecikmesini veya gerçek model yanıt kalitesini ölçmez. Tek bloktaki byte azalması, bütün istek/token/fatura kazancı değildir; retrieval özgün içeriği tekrar taşır.

**İstemci tarafında önemli sınır:** İlk 75KB log koşularında Claude CLI sonucu kendi izole profilinde tool-results dosyasına taşıdı. Sağlayıcıya giden kısa referans özgün log değildi; bu koşular başarısız olarak korundu. Pozitif fixture 120 satırla istemcinin dosyaya taşıma sınırının altında kaldı; CLI cap'ine override verilmedi. Bu nedenle native dosyaya taşınmış büyük çıktıya Headroom token tasarrufu atanmaz. Daha sonra bir koşu fixture'ın CCR JSON zarfını yanlış varsayması nedeniyle başarısızdı; pinned `ccr/response_handler.py` sözleşmesine göre düzeltildi.

Nihai native kayıt `work/proofs/claude-tools/runs/0c5e4fe1-4cad-4d58-b0b5-24d878e9bed6/result.json`; eski başarısızlar ve inceleme öncesi PASS ayrı korunur. Kaynak, CLI, Python, motor `.py` ve tokenizer cache dosyaları koşunun önce/sonrasında eşittir. Dört yeni odaklı normal/hata/sınır kontrolü geçti. Üretimde kullanılan kaynak/test/config ve Node/pnpm/Rust araçları değişmediği için mevcut 441 Rust, 221 Node, fmt ve frontend/embedded build kanıtı ilgili fingerprint'lerle yeniden kullanıldı; aynı testler sebepsiz tekrarlanmadı. [Programatik kabul](claude-tools.json), [inceleme](../reviews/H009-b.md).

Motor coding/cache, CCR TTL7200, yerel ölçüm açık, beacon kapalı; router/output shaper/memory/learn kapalı profilindeydi. H041 fixture overlay'i yalnız yerel ML cache kullanımı için HF/transformers offline ekler; bu sentetik testte protected-tool listesi boştur. Yerleşik Read/Bash/Edit kapalıdır; bu dilim gerçek kaynak/sayısal Read korumasını yeniden doğrulamaz. Loopback observer bir test aracıdır; gerçek Tauri UI/Rust ingress veya canlı auth kabulü değildir. Proxy/offline ayarları kaydedilmiştir, packet capture yapılmamıştır.

### Tekrarlama

Mevcut pinned motor/Python/cache ve hash'i eşleşen CLI gerekir. Başarılı sonucu yalnız ilgili girdi değiştiğinde yenile. Komutlar repo kökünde, kullanıcı profiline müdahale etmeden ayrı UUID test ortamı üretir:

```powershell
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B -m unittest discover -s tests/fixtures/claude -p test_tool_fixture.py
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B bench/claude/roundtrip.py --run --claude-exe %USERPROFILE%/.local/bin/claude.exe
```

Son komut başka binary hash'ini çalıştırmaz. Native CLI caps/ayarları, kullanıcı auth'u ve başka projelerin oturumları değiştirilmez. Canlı Anthropic/OAuth, genel araç/hook/plugin, alt ajan, Desktop ve başka OS kanıtı açık kalır; ana H009 ve toplam 42/54 bu yerel kabulle kapanmaz. Katalogdaki eski tools-kapalı profil bu farklı MCP profilinin kabulüymüş gibi yükseltilmedi.
