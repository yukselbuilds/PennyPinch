# H018 — Config işlemi kanıtı

22 Ekim2026. **H018 ana görev yerel kabulü tamamlandı.** [Karar005](../decisions/005-config-transaction.md), [kabul matrisi](../validation/config-transaction-matrix.md). A/B/C aşamalarının aşağıdaki açık notları tarihsel ilerleme kaydıdır; güncel sonuç D bölümündedir. Kullanıcı config/auth/DB değiştirilmedi. Yalnız yeni proje UUID fixture'ları kullanıldı; Windows sistem/registry/ACL ayarı değiştirilmedi, elevation istenmedi.

## D — Son aday ve native kabul

Root incelemesi ve aynı Luna çalışanının C3 etiket düzeltmesinden sonra son yerel kapı çalıştırıldı. [Frontend](../../work/proofs/config-transaction/frontend-quality.json):49 Node PASS, typecheck+Vite build PASS; sunucu başlatılmadı. [Tam gate](../../work/proofs/config-transaction/final-quality.json):160 Rust PASS/0 FAIL/3 ignored,19,188s; embedded custom-protocol build PASS,15,021s.3 ignored: ayrı opt-in pinned runtime ve real discovery testi; external-filesystem child helper ise parent testlerce çalıştırılır. Derlemede iki gerçek dead_code uyarısı (`preview_config`, `config_was_missing`) ve bir özet satırı var; wrapper bunu3 warning satırı sayar. Format/başarılı frontend kontrolleri eşleşen source/config/toolchain/artifact hash'leriyle tekrar edilmeden kullanıldı.57 kaynak/config fingerprint'i kayıtlı.

[Statik UI](../../work/proofs/config-transaction/ui-review/summary.json):32 temel+8 dialog yerleşimi,7 akış PASS; TR/EN, dark/light,1280×800 ve760×620,0 JS hata/harici istek. Edge155.0.4283.62/Playwright1.62.1; root dark ownership ve TR light dar ekranı görsel inceledi. Apply/Undo pending lock, no-op/blocked/expired, snapshot error, invalid/ambiguous/rejected sonucu yeniden okuma, kurulum bulunmadan Undo ve focus dönüşü kapsamı. Bu test sentetik IPC kullanır; native kanıt aşağıdadır.

[Native son kabul](../../work/proofs/config-transaction/native-proof.json), [12 bağımsız dosya/kayıt kontrolü](../../work/proofs/config-transaction/native-checks.json), [A normal kapanış](../../work/proofs/config-transaction/native-run-a.json), [B yeniden açılış/kapanış](../../work/proofs/config-transaction/native-run-b.json). İncelenen EXE15.866.368B/SHA `8cf08831ac4c32b53952c07bd76b0ebe6f042a381e175ae5b73465359ebdb624`. OCU gerçek mouse/keyboard ile UI→Tauri→aynı manager→TxF→gerçek disposable dosya yolu kullanıldı:

- Codex TOML Apply; sıradan editör atomic-save'iyle yeni `userNote`; Undo yalnız endpoint'i kaldırdı, yeni alan kaldı.
- Claude JSON Apply; normal Alt+F4/exit0; yeni süreçte Preview gerekmeden sahiplik bulundu ve Undo çalıştı. Başlangıç dosyası byte-exact geri kaldı.
- OpenCode JSONC Apply; endpoint kullanıcı değişikliği sonrası Undo conflict gösterdi, config+record hash'leri değişmedi. Fixture endpoint'i eski yönetilen değere getirip unrelated atomic-save yapınca Reload→Undo çalıştı; yorum ve yeni alan kaldı.

Native fixture UUID'si ve bütün doğrulanmış yollar [manifestte](../../work/proofs/config-transaction/native-fixture.json) kayıtlıdır. Çocuk sürece özel CODEX_HOME/CLAUDE_CONFIG_DIR/XDG_CONFIG_HOME ve diğer izolasyon kökleri kullanıldı; gerçek client/hook çalıştırılmadı, motor/provider/model isteği gönderilmedi. Sentetik npm sürüm metadata'sı canlı istemci uyumluluğu sayılmaz.

İki normal exit0; tercihler değişmedi. Son aşamada üç ownership kaydı `released`, fixture'larda endpoint yok; uygulama süreci0, stable51778 dinleyici0. Test dosyaları ve üç sırsız released tombstone tutuldu; sürekli izleme veya açık test penceresi yok. Kapanış sonrası OCU `appNotFound` readback'i bağımsız exit0 ile doğrulandı. Daha önce otomatik review'ın reddettiği yedi cleanup hedefi frozen kalır; alternatif temizlik denenmedi.

Yerel kapsam Windows10 Pro22H2 x64/build19045, normal yetki, aynı yerel sabit NTFS volume. TxF bakım/gelecek desteği riski ve fail-closed davranış karar005'te kayıtlı. Windows11, EFS üzerinde gerçek fixture, cross-volume, güç kesintisi, canlı auth/model/ana-alt ajan ve installer/yayın sonucu yok. Runtime start/stop/pinned motorun H040 tarihsel kanıtı yeni config göreviyle yeniden koşulmuş sayılmaz. H019 sırada, başlamadı. Token tasarrufu ölçülmedi; yüzde verilmedi.

## A1 — Klasik yayın API'si feasibility

Aynı Luna çalışanının fixture deneyi: target shareREAD-only ile normal writer open ve ReplaceFile Win32=32; MoveFileEx replace=5; FileRenameInfoEx/POSIX=32 ile reddedildi. Target shareREAD|DELETE + zayıf parent pin altında POSIX absolute rename çalıştı, ancak dış ReplaceFile da çalışabildi. Güçlü parent (LIST_DIRECTORY|ADD_FILE|READ_ATTRIBUTES|SYNCHRONIZE, shareREAD-only) altında normal writer/replace engellendi; bizim relative RootDirectory/FileRenameInfoEx çağrımız87, absolute çağrı32 aldı. Güvenli atomik karşılaştırmalı yayın sonucu çıkarılmadı.

[İlk matris](../../work/proofs/config-transaction/a4c651f3-0407-475f-b71c-3d6bafd51782/proof.json), [son parent matrisi](../../work/proofs/config-transaction/e8a6f67c-8269-4339-9e02-94e20d1f1deb/proof-parent-only.json), [son probe kaynağı](../../work/proofs/config-transaction/e8a6f67c-8269-4339-9e02-94e20d1f1deb/probe_isolation.py), SHA `ddebc2c24d76a1c9c7ad19e406b2aa0a646fc1a904547b008655d45cb00ebf43`. Ara isolation denemesindeki fixture PermissionError başarı sayılmadı; yeni dar matrisle ayrıldı. Ham hata çıktısı kalıcı tutulmadı.

## A1 — KTM/TxF native feasibility kabulü

Ana ajan QA, Python3.14.0 / Windows10 Pro22H2 x64 build19045 / yerel NTFS. İki farklı komut farklı durumları test etti; başarılı base probe yeni matriste tekrar çalıştırılmadı, source/proof hash'iyle kullanıldı.

| Komut / kanıt | Ölçülen sonuç |
| --- | --- |
| `C:\Python314\python.exe work/proofs/config-transaction/txf_probe.py` — [metadata](../../work/proofs/config-transaction/txf-runs/f0d6431c-a2bc-49c2-bb1a-93548fa5f2fc/proof.json) | PASS. Dış okuyucu eski config'i gördü, ownership yoktu. Writer/MoveFileEx/ReplaceFile/DeleteFile hem file handle açıkken hem kapatılıp commit öncesi32/6800 ile reddedildi. Commit iki dosyayı yayımladı; explicit rollback ve açık handle'larla child exit73 eski config'i korudu. |
| `C:\Python314\python.exe work/proofs/config-transaction/txf_crash_matrix.py` — [metadata](../../work/proofs/config-transaction/txf-runs/96150dc7-6c9f-4a9b-b7e3-0083a6bef9d6/proof.json) | PASS. Process elevation=false. Ayrı racer child, tüm transacted file handle'ları kapanmışken de yazamadı/replace/delete yapamadı. Commit öncesi child exit73 config+ownership ikisini eski; commit sonrası exit74 ikisini yeni bıraktı. Önceden açık writer admission6800 ile reddedildi. Eksik dosya adı transaction'da rezerveydi; outside CREATE_NEW6800, rollback sonrası dosya yok. |
| `C:\Python314\python.exe work/proofs/config-transaction/txf_absence_guard.py` — [metadata](../../work/proofs/config-transaction/txf-runs/47e7b5c5-8a14-4bbf-972a-f13ad832a6c0/proof.json) | PASS. Alternatif OpenCode slotu için TxF CREATE_NEW+READ/WRITE/DELETE+DELETE_ON_CLOSE; handle kapandıktan commit öncesi ayrı child CREATE_NEW6800 ile reddedildi. Dosya dışarıdan görünmedi, commit sonunda da yoktu; sonrasında isim yeni dosyaya açılabildi. |

[Base probe](../../work/proofs/config-transaction/txf_probe.py), [ek matris](../../work/proofs/config-transaction/txf_crash_matrix.py) ve [absence guard](../../work/proofs/config-transaction/txf_absence_guard.py) yalnız kendi `txf-runs/<UUID>` köklerini kullanır. Fixture child'ları kendi süreçlerini `os._exit` ile sonlandırdı; kullanıcı/istemci süreçleri sonlandırılmadı. Metadata kaynak/Python/OS/hash/sonuç ve yalnız sentetik dosya hash'leri içerir. Gerçek auth veya raw failure log yok.

**Bu kabul native API feasibility içindir.** Rust production primitive, ownership şeması, PlanManager/typed UI ve son yerel kalite kapısı açık. NTFS desteği olmayan/EFS/başka OS ortamı veya saldırgan kernel yazarı için sonuç uydurulmaz. Power-off testi yapılmadı. Microsoft'un TxF önerilmeme/gelecekte kaldırılabilme riski karar005'te açıkça kayıtlıdır.

## B1 — Saf patch/undo

Ana ajan kaynak ve9 odaklı testi inceledi; **alt dilim kabul edildi**. Codex TOML, Claude JSON ve OpenCode V1 JSONC'de yalnız missing endpoint eklenir; same endpoint byte-exact no-op/sahiplik yok, custom/blocked/mismatch plan reddedilir. Missing Codex/Claude sentinelleri desteklenir; OpenCode missing rota engeli korunur. Hâlâ yalnız bellekte çalışan, test build'ine dahil edilmiş çekirdektir; production disk apply/undo komutu açılmaz.

İnceleme düzeltmeleri: JSONC comma ile property arasını topluca silmek yerine token aralıkları ayrı kaldırıldı; kullanıcı yorumları ve single-property trailing-comma korunur. Eklenen JSON ayırıcıya whitespace katılmadığından yorum koruması ile özgün byte-exact roundtrip birlikte sağlanır. Sonradan veri/yorum eklenen oluşturulmuş container korunur; endpoint kullanıcı tarafından değiştirilmişse conflict. TOML root key/value span'leri parser'dan alınır.

9/9 focused `integrations::transaction_tests`, fmt/check PASS; yeni dependency yok. Check'te tarihsel `plan::preview_config` test adapter'ı için1 dead_code uyarısı var. [Komut/source/config fingerprint'leri](../../work/proofs/config-transaction/patcher-b1.md). Ana ajan aynı başarılı kontrolleri tekrar çalıştırmadı. Bu sonuç dosya yayını/çökme/istemci benimseme kanıtı değildir.

## A2 — Rust filesystem transaction primitive

Aynı Luna çalışanının Rust primitive'i ana incelemeden geçti; **alt dilim kabul edildi**.11 focused PASS/1 ignored child helper; helper dış writer/move/replace/delete/create denemelerinde parent testten gerçekten çalıştırıldı. fmt/check PASS; son aday kaynak/manifest fingerprint'leri [writer-a2.md](../../work/proofs/config-transaction/writer-a2.md) içindedir. Ana ajan aynı başarılı komutları tekrar çalıştırmadı.

İnceleme düzeltmeleri: absent-slot reservations ile file-entry indeksleri ayrıldı; token transaction kimliğine bağlandı. Seal sonrası hatalı mutasyon commit'i zehirler ve rollback olur; commit pin/deadline kontrollerini yeniden yapar.32/33/6800 sabit Conflict olarak döner. File handles commit öncesi kapanır; NTFS transaction intent lock'u korunur. En çok4 path/256KiB,10s timeout, aynı volume ve `TRANSACTION_DO_NOT_PROMOTE`; EFS/reparse/hardlink/readonly ve desteklenmeyen dosya sistemleri kapalı. Daha zayıf rename fallback yok.

Bu Rust dilimi ayrı child crash kabulü iddia etmez; native crash kanıtı A1'de ayrıdır. B2 tanısı sırasında bu kaynağın SHA'sı değişti; eski A2 PASS'i yeni hash'e aynen reuse edilmedi. Geçici instrumentation kaldırıldı; güncel primitive'in A2 testleri son H018 gate'inde yeniden kapsanacak.

**Yeni frozen kalıntılar:** A2'nin önceki başarısız testlerinin yedi UUID kökü için guarded cleanup `blocked by policy` ile reddedildi. `work/proofs/config-transaction/runs/` altında `06efb0f5-20be-45f8-9716-75dedf0fb6d1` (config.json/replacement.json), `892ef8e9-f5e5-46d0-9a8f-c61bf191ce9d` (linked.json/source.json), `abef4a2a-169d-4bcb-8a4e-1df19b8e11e7` (original.toml), `be7ba9a2-29df-4355-8d02-1c607da12d7b` (boş), `c1084172-e7eb-4f4f-904e-323690989733` (config.toml), `589a9f5d-a46f-437e-8ac0-a2cceab44861` ve `e6aa4a1f-7d6e-4246-830a-81d26aa01236` (preflight.json). Bu hedefler/altları veya kapsayan parent temizliği başka araçla tekrarlanmaz. Son başarılı testler yalnız kendi yeni UUID'lerini temizledi.

## B2 — Kalıcı sahiplik alt kabulü

Ana kaynak incelemesi ve16 focused ownership testi kabul. Sabit3 client slotu/en çok16KiB; schema/owner tag/UUID/source/presence/mask/revision/hash doğrulaması, aktif başka root/owner'a geçişin reddi. Raw body/auth/URL/hook/path tutulmaz; dosya gövdesi yerine gerekli hash ve validated patch receipt saklanır. Same caller transaction'a read/stage API'si var; bağımsız commit/backup yok. Constructor ve missing read pasif, aynı endpoint no-op'ta prepare çağırmamak service sorumluluğudur.

Lazy mkdir öncesi mevcut ancestor zincirleri held pin'lerle tutulur ve sonrası doğrulanır; root rename Win32=32 ile reddedildi. Önceden mevcut gerçek NTFS junction ve hardlinked slot fail-closed. Nested yolun `/`→canonical `\` mismatch'i fixed-stage/Win32=0 ile tanılandı; normal components'ı ayrı push ederek düzeltildi, nested create+rollback regression korundu. OpenCodeMissing testi yalnız codec kanıtıdır, planner bu rotayı açmaz.

Scoped rustfmt ve offline check PASS; **tam-crate format henüz PASS değil** (`transaction.rs`/`state/mod.rs` dahil değişen dosyaların format borcu C1'de kapatılacak). [Son komut/kaynak fingerprint'leri](../../work/proofs/config-transaction/ownership-b2.md). Güncel `transaction_fs.rs` SHA `8b5f8f567855ddbfebe0d7baa0bb18f2748d11203f785a4b025d2ca25cc041e5`, eski A2 hash'iyle eşleşmiyor. Yeni/ilişkili receipt testleri ve A2 regression'ları final gate'de kapsanacak; root başarılı B2 suite'i tekrar etmedi. C1 mevcut manager apply/undo/snapshot etkin, C2 typed IPC/C3 Graphite/D native ve son kalite kabulü açık. **H018 ana görevi tamamlanmadı.**

## C1 — Gerçek yönetici işlemleri alt kabulü

21 Ekim son adayı,22 Ekim devamında9 source/dependency SHA ve Rust/Cargo1.92.0/rustfmt1.8.0/Windows10 build19045 eşleşmesiyle korundu; kredi/tarih değişimi nedeniyle test tekrarı yapılmadı. **14 connection-action+16 ownership focused PASS**;3 config formatı, missing Codex, blocked/expired/replay/stale plan, same-bytes/new-ID, kök/tarama/endpoint drift, OpenCode alternate-slot appearance, unowned no-op, unrelated atomic-save undo, managed-field conflict ve iki yönde precommit rollback kabul. [Komutlar/fingerprint'ler](../../work/proofs/config-transaction/actions-c1.md).

Root kaynak incelemesi sonrası fixed error code→retryable/message key tekilleşti; record source güncel config/plan ile karşılaştırıldı. Live-plan helper iki expiry saatini ve no-op dönüşü dahil güncelliği doğrular; sürüm allowlist'i planner helper'ından kullanılır. Yanıt summary/fingerprint'i commit öncesi hazırlanır. Config ve record aynı TxF commit'inde, undo eski after-ID veya tüm dosya hash'ini zorunlu tutmaz. Snapshot içeriksiz kayıt metadata'sıdır; trafik doğrulanmış sayılmaz.

`cargo fmt --check` ve offline check komutları PASS; **format kapsamı notu:** `include!` ile açılan action dosyasını cargo fmt'nin taraması C2'de normal `#[path] mod` bildirimi ve tüm-Rust hash listesiyle kesinleştirilecek. Güncel primitive'e canonical-target getter eklendi; eski A2 source PASS'i yeni hash'e aynen reuse edilmez, final gate tüm regression'ları çalıştıracak. C2 kredi sınırı nedeniyle başlamadan kesilmişti;22 Ekim aynı Luna çalışanıyla typed Tauri/TS bağı etkin. C3 Graphite ve D son quality/native kabulü açık; **H018 ana görev kutusu hâlâ açık, H019 başlamadı.**

## C2 — Dar masaüstü/TS bağı alt kabulü

22 Ekim. Root main/locator/TS decoder ve testlerini inceledi; **11 focused Node, typecheck, fmt ve offline check PASS**. Aynı `Arc<PlanManager>` üstünde `apply_connection_plan`, `undo_connection`, `get_connection_ownership`; filesystem işi spawn_blocking, join hatası sabit config_unavailable. Setup locator pasif, debug proje içi root/release app-local-data root; store kurulamaması preview/app açılışını durdurmaz ve state dizini yaratmaz.

TS yalnız C1 dar request alanlarını yollar; schema/request/client/operation, exact key sets/UUID/hash/revision/port/source/ownership-state korelasyonu ve exact fixed error tuple'larını doğrulayarak yeni DTO oluşturur. Browser invoke, raw exception/secret aktarımı, otomatik mutation retry yok. [Komut/source fingerprint'leri](../../work/proofs/config-transaction/typed-actions-c2.md). Gerçek client config veya trafik denenmedi.

Action modülü `include!` yerine normal `#[path] mod` ile açıldı;30 Rust source+build.rs/Cargo.toml [format-pass.json](../../work/proofs/config-transaction/format-pass.json) içinde. C3 Rust'a dokunmazsa bu PASS son gate'de tekrar çalıştırılmadan kullanılabilir. Offline check'te2 bilinen unused-helper uyarısı var (preview_config/config_was_missing); warning suppression eklenmedi. C3 Graphite etkin; final frontend/full Rust/embedded build/native UI kabulü henüz yok.
