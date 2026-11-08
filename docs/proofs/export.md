# H033 — istatistik ve tanı dışa aktarma kabulü

8 Kasım 2026; **tamamlandı, 32/51**. [Plan 021](../decisions/021-export.md), [inceleme](../reviews/H033.md). Ana ajan ölçüm anı, kayıt/dosya güvenliği, entegrasyon ve kabul sahibi; mevcut tek Luna frontend/client/test dilimini tamamladı. H034 başlamadı.

Genel bakıştaki Dışa aktar menüsünden seçili dönem özeti JSON/CSV veya ayıklanmış tanı JSON alınır. Rapor, tıklamada görünen native ölçüm anını kullanır; kaydetme penceresinde beklerken gelen yeni ölçüm dosyanın toplamlarını değiştirmez. RAM cache'i8 aggregate kayıtla sınırlıdır. Özet istek/model listesi veya ücret hesabı değildir; bilinmeyen değer null/boş, gerçek0 ve signed artış ayrı kalır. Tanı, ölçüm okunamasa da kamuya açık durum/kategorileri kaydedebilir.

Yalnız kullanıcının Save As ile seçtiği yeni dosyaya yazılır. Mevcut hedef, güvensiz/linkli dizin, yanlış uzantı, meşguliyet veya expired capture başarı sayılmaz. Dosya tamamlanıp sync edildikten sonra aynı açık handle üzerinden replace etmeyen rename yapılır; parent pin ve yarış koruması korunur. [Microsoft API sözleşmesi](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_rename_info), pinned SDK kaynağı ve gerçek FS testleriyle kontrol edildi. Güç kesintisi dayanıklılığı veya yeni OS desteği iddiası yoktur.

## Son adayın kanıtı

Kayıtlar `work/proofs/export/` altında. `acceptance.json`,16 kontrolle180 güncel kaynak/config girdisini, binary/dist/UI bundle'ını ve araç/OS ortamını bağlar. Sonuç ve önce/sonra fingerprint'ler aynı adımlarda programatik tutulur. Ham auth, prompt, HTTP gövdesi veya hata dökümü kalıcılaştırılmadı.

| Senaryo | Gözlenen | Kanıt / sınır |
| --- | --- | --- |
| Strict client zarfı, korelasyon, byte sınırı, single-flight, cancel/error |8 yeni client testi PASS | `worker-f8c322f6-68da-47e2-9b2d-01a6f1e668e5.json`; root wire düzeltmesini inceledi, son tam Node kapısı aynı testi içerir. |
| Capture sabitleme/8 kayıt/expired, JSON ve TR/EN CSV0-negatif-null, canary/formül, cancel/busy/exit rollback |5 report +3 gerçek FS testi PASS | `focused-3.json`; yeni/racing hedef, Unicode filename, uzantı ve kapasite; eski dosya değişmedi. Gerçek disk doldurulmadı. |
| Son yerel kalite kapısı |161 frontend/371 Rust PASS,8 opt-in ignored; fmt PASS | `final-1.json`; `build-1.json` TS/Vite/custom-protocol PASS'i aynı source/config/araç fingerprint'iyle bağlı. Aynı başarılı build gereksiz tekrarlanmadı. |
| TR/EN/açık-koyu ×1280/1024/819/683/507, ESC/focus/pending/error/period/unmount |20 birleşim,6 davranış grubu PASS; taşma/page error yok | `ui-c2cbe56d-dfbc-445f-8ef4-531837db18de/result.json`; gerçek App + açıkça izole örnek transport. TR-dark ve EN-light görüntüleri ana ajan tarafından incelendi. |
| Native Save As ile JSON/CSV/tanı + Cancel | Gerçek mouse/keyboard ile3 dosya; tıklamadaki committed UI capture ile eşleşme | Run `b90930b8-1015-4bab-b182-143d2738f541`, `files-acceptance.json`:18 kontrol. Salt okunur/passive observer sonuç değiştirmedi; native invoke immutable kaldı. |
| Dönem/source/coverage/yüzde/miktar, beklerken yeni snapshot | JSON/CSV aynı tıklama anını korudu; dosya zamanı daha yeni UI zamanına kaymadı | Aynı run. Yerel SQLite fixture100→50/%50; bu örnek veri canlı sağlayıcı veya geliştirme tasarrufu değildir. |
| CSV kodlama/locale ve tanı allowlist | UTF-8 BOM/CRLF/TR delimiter ve sayılar; credential/content/path/kimlik alanı yok | Aynı18 kontrol + renderer/client testleri. Native dialog TR; EN UI/client/renderer kapsamındadır. |
| Cancel | Üç mevcut raporun fingerprint'i aynı, dördüncü dosya yok | `cancel-before.json`, `cancelled-final.json`, `files-acceptance.json`. |
| Kapanış/state | UI çıkış onayı sonrası process yok, owned debug/stable port kapalı; runtime/metrics/preferences baseline baytları ve protected state/Run aynı | `baseline.json`, `cleanup-recovery.json`. Yardımcı betik SQLite handle kilidi nedeniyle klasör restore'unda hata aldı; ayrı güvenli recovery tamamlandı. Native application exit code kaydı yoktur;0 diye yazılmadı. |

İlk `0c93ba02-1869-4b00-90b5-9969bb50e96f` koşusu kredi/oturum kesintisinde kaldı; PASS sayılmadı. `interruption-recovery.json` orijinalleri geri yükledi, o anda bulunan test state'i ayrı klasörlerde korudu. Son run'ın recovery kaydı aynı orijinal state zincirini doğrular. Helper'ın sorunu Python SQLite context manager'ının bağlantıyı kapatmamasıydı: eski artefact değişmeden `seed_closed.py` ve `native-safe.py` üretildi; `seed-smoke-c0df9be9-738e-4410-9710-b7c0026432c6/result.json` bağlantı kapandıktan sonra dizin taşımanın çalıştığını doğruladı. Bu QA düzeltmesi ürün testinin yerine sayılmadı.

Ortam Windows10 x64 build19045; Rust/Cargo1.92.0, Node25.2.1, pnpm11.25.0, Python3.14.0. Windows11 H047/karar006 ücretsiz istisnasıyla not_tested; diğer OS/installer/yayın/canlı sağlayıcı/uzun yük/gerçek disk dolması doğrulanmadı. İstemci/auth/model/başlangıç ayarları değiştirilmedi. Geliştirme token tasarrufu ölçülmedi.

H033 kapatıldı; sıradaki H034 hata ve toparlama akışları başlamadı. Ana görev sonunda duruldu.
