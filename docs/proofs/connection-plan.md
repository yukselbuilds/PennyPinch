# H017 — Bağlantı planı ve önizleme

21 Ekim 2026. **H017 Windows10 yerel önizleme kabulü tamamlandı.** Tasarım: [004](../decisions/004-connection-plan.md), [son inceleme](../reviews/H017.md). Gerçek config yazımı/bağlama H018–H021 kapılarıdır. A–C1 kayıtları dilim tarihindeki sonuçları korur; son aday ve kabul aşağıdadır.

## A — Saf planlayıcı kabulü

Codex TOML, Claude JSON, OpenCode V1 JSONC fixture'ları yalnız endpoint alanını önerir. Model/provider/auth/reasoning/ajan sayısı korunur; etkin çalışma ayarları `unknown`, yeni süreç/oturum gereği `restart_required=true`. Plan beş dakika süreli UUID, config SHA-256 ve backend-supplied kalıcı endpoint kimliği/portuyla ilişkilidir. Auth dosyası/config dosyası/sohbet DB'si bu saf katmanda okunmaz veya yazılmaz.

256KiB belge sınırı; JSONC parser Scanner'ıyla AST öncesi 64 iç içe yapı ve token sınırı; parser sonrası node/duplicate-key kontrolü var. TOML parser'ın kendi 80 recursion guard'ı ve sonraki ağaç kontrolü kullanılıyor. Özel gateway, profil, bulut ve birden fazla/override provider rotası belirsiz destek olarak engellenir; özgün sır/URL/path/hook komutu çıktıya taşınmaz.

**13 odaklı Rust testi PASS**, rustfmt/check PASS. Rust/Cargo1.92.0, Windows x64 MSVC, proje içi cache/target. Saf çekirdeğin henüz production komutuna bağlanmaması nedeniyle check'te41 dead_code uyarısı var; son adayın durumu değildir. Test/komut/source/config fingerprint'leri [A ayrıntısında](../../work/proofs/connection-plan/planner-a.md). Ana ajan testleri ve parser/plan kodunu inceledi; erken incelemedeki derinlik/route/generation/restart sorunları bu adayda giderildi. Kaynaklar değişmediği sürece aynı A komutu tekrarlanmaz.

## B1 — Opaque seçim ve ingress erişimi kabulü

`select_installation(id, expected_scan_generation)` yalnız keşif yöneticisindeki summary'yi döndürür; UUID/pozitif JS-safe nesil/varlık/güncel tarama kontrol edilir. Dosya yolu renderer'a çıkmaz. `ready_stable_endpoint()` taze, hatasız loopback endpointini döndürür. Motor durmuşken route-none; Ready iken health ve route/runtime nesil uyumu gerekir. Runtime yaşam döngüsü değiştirilmedi.

Discovery1 ve runtime4 odaklı test RED→GREEN, fmt/check PASS.48 geçici dead_code uyarısı sonraki komut bağını bekliyor. Ana ajan accessor ve testleri inceledi; gerçek client config/engine/process çağrısı yapılmadı. B1 kaynak SHA'ları: discovery `d361b8aca433327dd7cd3e600e3db659a1436a6e3e98c8fa6c8938a8584ef4dc`, discovery_tests `4d6f54ecc0be9196375477a883095e31192c4acde68e6fa195335e54c00e7ce0`, manager `7a87cdf639510a2e1cab2d3e55038b66e1985d18ac51f82628b1465043297f99`, manager_tests `7b5bb168099c8d91e991aa4edcc42bff80403150c394faf882b781dae0f73e09`.

## B2 — Bounded config servisi kabulü

Servis seçilen client türünün user config konumunu yalnız backend env/default köklerinden türetir. Geçersiz override'da fallback yok; JSON+JSONC birlikteyse conflict. Missing/present ayrıdır; missing planı dosya/dizin oluşturmaz. İki okuma arasında içerik, kaynak dosya slotu/volume/file identity, keşif ve endpoint yeniden karşılaştırılır. Sırsız planlar UUID ve beş dakikalık bounded lease ile bellekte kalır; apply komutu yok.

**15 I/O ve16 plan testi/fmt/check PASS.** Gerçek Windows writer-open ve parent rename reddi; privilege gerektirmeyen gerçek junction'ın reddi ve hedefin korunması doğrulandı. İlk READ_ATTRIBUTES-only handle rename'i engellememişti; Microsoft [CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew) paylaşım sınırıyla karşılaştırıldı, final LIST_DIRECTORY|READ_ATTRIBUTES|SYNCHRONIZE/no-share-delete guard'ı regression'da geçti. İlk symlink privilege denemesi başarısızdı, sessiz skip başarı sayılmadı. Yalnız proje fixture'ları kullanıldı. [Komut/source/toolchain fingerprint'leri](../../work/proofs/connection-plan/planner-b2.md).

Ana ajan guard, source/identity binding, ortak hata zarfı ve cache'i inceledi. C1'de değişken içerik/context hash framing'i ayrıca güçlendiriliyor; B2 proof'u kendi hash'leri içindir. Önceki A kaynağı B2'de presence/version/source alanlarıyla değiştiği için16 test yeniden koşuldu. Son kalite kapısı henüz çalışmadı.

## C1 — Typed komut ve bridge kabulü

`main.rs` tek mevcut RuntimeManager/DiscoveryManager'ı aynı PlanManager'a bağlar; ikinci runtime yaratmaz. `preview_connection_plan` blocking dosya okumasını ayrı çalıştırır, raw join/OS hatası taşımaz. Strict TS decoder yalnız dar argümanları gönderir, istemci/request/ID/nesil ve bütün alan tutarlılığını kontrol eder. Mixed-client, out-of-Date expiry ve missing-config çelişkileri inceleme regresyonlarıyla reddedilir. AtomicBool+RAII yalnız bir preview okumasına izin verir; ikinci çağrı correlated busy alır, conflict sonrası guard bırakılır.

**6 TS bridge testi/typecheck; yeni concurrency1 ve gerçek missing-config1 Rust regresyonu/fmt PASS.** İlk C1'de wire-name1, fingerprint4 ve same-byte file/slot2 odaklı test de geçti. Fingerprint artık length-framed content digest'i kullanır. [C1](../../work/proofs/connection-plan/typed-bridge-c1.md), [inceleme düzeltmeleri](../../work/proofs/connection-plan/typed-bridge-c1-review-fixes.md), [format fingerprint](../../work/proofs/connection-plan/format-pass.json). Sonuncusu aynı Rust kaynaklarıyla son kapıda tekrar koşulmadan kullanılacak. Native komut başarısı henüz iddia edilmez.

## C2 — Graphite önizleme ve inceleme

Applications → belirli kurulum → kullanıcının Preview eylemi → strict bridge → PlanManager → bounded config reader → planner → redakte önizleme yolu tamamlandı. Çoklu kurulumda seçim açıkça yapılır; ekran açılması veya normal runtime poll'u config okumasını başlatmaz. Kapsam yalnız `user_config`; diğer katmanlar ve açık oturumdaki etkin değerler bilinmiyor gösterilir. Bir endpoint alanı önerilir; model/efor/auth/provider/ajan sayısı korunur. Hook varlığı sabit etiketle gösterilir; içeriği çalıştırılmaz/gösterilmez. Apply devre dışı ve yalnız önizleme açıklaması görünür.

İnceleme sonrası endpoint alan adları kesinleştirildi, satırların okunurluğu düzeltildi, ürün metninden görev kimliği çıkarıldı ve geç gelen eski yanıtlar bastırıldı. Built-artifact kontrolünde Tab'ın BODY'ye kaçabildiği bulundu; görünür/etkin kontrolleri saran capture listener düzeltildi. Son TSX SHA `d3f711b7518de45ebd48736a3b86956ab03ddb394ecfad3a2998381fb3450265`. [İlk UI dilimi](connection-plan-ui.md), [ilk düzeltmeler](connection-plan-ui-review-fixes.md), [odak düzeltmesi](connection-plan-ui-focus-fix.md).

## Son yerel kalite kapısı

Ortam: Windows10 Pro22H2 x64/build19045; Rust/Cargo1.92.0, rustfmt1.8.0, Node25.2.1, pnpm11.25.0; Edge155.0.4283.62/Playwright1.62.1. Cargo cache/target proje altında. [Gate scripti](../../work/proofs/connection-plan/final_gate.py), [son47 kaynak/config ve artifact fingerprint'leri](../../work/proofs/connection-plan/final-quality.json).

| Kontrol | Sonuç |
| --- | --- |
| `node --experimental-strip-types --test tests/app/*.test.mjs` (raporda5 dosya açıkça listeli) | 38 PASS,0 FAIL;0 uyarı |
| `pnpm build` (typecheck dahil) | PASS,1,999s;0 uyarı |
| `cargo test --manifest-path src-tauri/Cargo.toml --offline -- --test-threads=1` | 107 PASS,0 FAIL,2 opt-in ignored;32,950s;0 uyarı |
| `cargo build --manifest-path src-tauri/Cargo.toml --offline --features custom-protocol` | PASS,34,803s;1 kullanılmayan test-adapter fonksiyonu uyarısı |
| Format | [Önceki PASS](../../work/proofs/connection-plan/format-pass.json), aynı Rust/config/toolchain hash'leri doğrulanarak tekrar çalıştırılmadan kullanıldı |

Build'in `preview_config` uyarısı testlerin kullandığı saf adapter'ın production'da kullanılmamasıdır; production yolu `preview_config_with_presence` kullanır. Metadata sayacı gerçek uyarı+Cargo özetini2 satır sayar; iki farklı uyarı veya sıfır uyarılı build iddia edilmez. Bloklayıcı değildir.

[Frontend metadata](../../work/proofs/connection-plan/frontend-quality.json): ilk frontend gate sonrası yalnız TSX odak düzeltmesi yapılınca ilgili `.ts`/`.mjs`/dependency/toolchain girdileri değişmeyen38 test reuse edildi ve build yenilendi. Final birleşik kapıda kaynak/artifact/toolchain eşleşmesiyle frontend ve format PASS'leri tekrar koşulmadı. Kredi/compact duraklaması geçerli kanıtı geçersizleştirmedi.

## Statik artifact UI kabulü

`node work/proofs/connection-plan/validate_ui.cjs`: **32 temel+8 dialog düzeni,5 akış grubu PASS;0 browser error,0 external page request**. TR/EN, koyu/açık,1280×800 ve760×620; explicit seçim/no-auto-preview/poll-stable, Tab/ShiftTab/Escape odağı, refresh, üç kesin alan adı, blocked/missing/expired/conflict/invalid yanıtlar ve kapanan dialogdan geç yanıtın yeni istemciyi ezmemesi kontrol edildi. Vite/server açılmadı; built dist `route.fulfill` ve açıkça sentetik memory bridge kullanıldı. **Native IPC kanıtı değildir.** [Metadata](../../work/proofs/connection-plan/ui-review/summary.json), [koyu EN görseli](../../work/proofs/connection-plan/ui-review/plan-en-dark.png), [dar açık TR görseli](../../work/proofs/connection-plan/ui-review/plan-tr-light-760.png). Ana ajan iki görseli inceledi.

## Gerçek native kabulü

Embedded EXE **15.298.560B**, SHA `4ee7b53789d8829a66a7fd8e8974d369fd4017e8d754770a8e3436172f818bf1`. [Launcher](../../work/proofs/connection-plan/native_fixture.py) yalnız çocuk Headroom sürecine disposable CODEX_HOME/CLAUDE_CONFIG_DIR/XDG_CONFIG_HOME/PATH verdi; gerçek config, auth veya sohbet DB'si kullanılmadı. Unicode/boşluklu köklerdeki üç dosya gerçek backend tarafından okundu. `.cmd` adayları0 bayttır, çalıştırılmadı. Keşifteki kurulu Claude adayı da çalıştırılmadı. Motor başlatılmadı/sağlayıcı isteği gönderilmedi.

OCU normal MCP fiziksel indexed mouse ile üç kartın Connect/Preview eylemleri kullanıldı. Codex TOML `openai_base_url` ve OpenCode JSONC `provider.openai.options.baseURL` için `http://127.0.0.1:51778/v1`; Claude JSON `env.ANTHROPIC_BASE_URL` için aynı kalıcı adres `/v1` olmadan görüldü. Claude çoklu kurulum başlangıçta seçilmemişti; PATH adayı açıkça seçildikten sonra ayrı Preview tıklandı. Config bildirimleri ve korunma durumları doğru; runtime etkin ayarı bilinmiyor, unknown-version blocker ve disabled Apply görünür. Codex Refresh yeni süre üretti. Claude alt satırları ScrollIntoView ile görünür incelendi. OpenCode'da fiziksel Tab/ShiftTab son↔ilk kontrol sarması ve Escape sonrası kart odağı gözlendi. Header Close da çalıştı.

Native dropdown option fiziksel click'i OCU overlay hit-target guard'ı reddetti; hiçbir tıklama gönderilmedi. Aynı açık/focuslu combobox'ta End/Return ile seçim yapıldı. Generic scroll body'yi hareket ettirmeyince elemanın sunduğu ScrollIntoView kullanıldı. Alt+F4 sonrası tool readback `appNotFound` verdi; retained launcher bağımsız olarak **normal exit0** doğruladı. Bunlar ürün başarısı diye gizlenen otomasyon hataları değildir. [Gözlem metadatası](../../work/proofs/connection-plan/native-proof.json), [gerçek hash/exit raporu](../../work/proofs/connection-plan/native-run.json).

Üç config'in baytları ve6 dosyalı fixture ağacı değişmedi/ek dosya oluşmadı; tercihler başlangıç/son SHA `04ba8cfd197a03cf18ac2e1870b890ba4c90d060e0ba7d766e495ca61b43e7ac`. Kapanış sonrası app PID10152 yok,51778 listener0; sürekli izleme veya açık test penceresi kalmadı.

## Kabul ve kalan sınırlar

H017 önizleme kabulü karşılandı. Bilinmeyen sürümler engelli taslak üretir; bu native test kurulu Codex/Claude/OpenCode'un canlı auth/model/transport/alt ajan desteğini kanıtlamaz. Katalog yükseltilmedi. Windows11/temiz installer, atomik apply/undo (H018), gerçek istemci trafiği (H019–H021), pause/CCR ve gerçek tasarruf ölçümü ayrı açık kapılardır. TASKS/checkpoint güncellendi; **H018 başlamadan duruldu**.

Son devir tutarlılığı: [validator](../../work/proofs/connection-plan/validate_handoff.py) yalnız45 tekil ana görev/12 tamam işaretini, yerel belge linklerini, kaynak/artifact/native fixture/tercih hash'lerini ve kapanış raporlarını kontrol eder. Çıktı `work/proofs/connection-plan/handoff-quality.json`; test/build/UI komutlarını tekrar çalıştırmaz.
