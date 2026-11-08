# H036 — uyum kataloğu ve güncelleme davranışı

8 Kasım 2026; **yerel görev kabulü tamamlandı**. Windows 10 22H2 x64, build19045, development0.1.0 ve pinned motor0.41.0. Kullanıcının açık isteğiyle H035'in manuel tepsi kontrolü beklerken yalnız H036 yürütüldü. H035 açık kalır; ürün toplamı34/51. [Plan024](../decisions/024-compatibility-catalog.md), [ana ajan incelemesi](../reviews/H036.md).

Uygulamalar ekranındaki kapalı başlayan Uyumluluk bölümü, bulunan kurulumları ve12 tarihsel test kaydını ayrı gösterir. Sürüm, CLI/Desktop yüzeyi, hesap yöntemi, OS, motor ve kanıt tarihi görünür. Canlı doğrulanmış kapsam ile deneysel yerel test, denenmemiş, bilinmeyen ve desteklenmeyen durumlar ayrıdır; kısmi/engellenmiş alt kabiliyetler ayrıntıda korunur. Geçmiş kayıt mevcut hesabın veya açık sohbetin doğrulandığı anlamına gelmez. Yeni model allowlist'i veya model/efor/ajan değişikliği yoktur.

## Davranış ve kanıt

| Senaryo | Beklenen ve gözlenen | Kanıt | Sınır |
| --- | --- | --- | --- |
| Motor kapalıyken Uyumluluk'u aç | Gerçek IPC12 gömülü kaydı döndürdü; motor kapalı kaldı. | Native `stopped-read.json` | Katalog okuması; canlı sağlayıcı kontrolü değil. |
| Hazır motorda oku, tara, yenile | Motor aynı nesil ve0.41.0 kaldı. Tarama sürerken ve bitince eski kayıtlar gizlendi; Yenile ikinci taramaya bağlı yeni yanıtı gösterdi. | Native `ready-before/ready-read/during-scan/stale-scan/refreshed.json` | Sonuç alındıktan sonraki dış değişiklikler için yeniden tarama/yenileme gerekir. |
| Geç yanıt, değişmiş kurulum/config, bozuk yanıt | Geç yanıt yeni bağlamın sonucu olmadı. Metadata değişimi stale; desteklenen yeni sürüm kanıt yoksa doğrulanmamış. Üç adaptörde kullanıcı config düzenlemesi fingerprint'i değiştirdi; dosya ve auth canary korundu. | Rust discovery/config testleri, strict client ve gerçek-App UI | Fixture değişimleri gerçek hesap/sağlayıcı desteği sayılmaz. |
| Yanlış motor pini ve geri dönüş | Daha eski/yeni/floating pin reddedildi; orijinal tam pin tekrar kabul edildi. Katalog okuması güncelleme veya süreç değişimi yapmadı. | `compatibility_engine_pin_rejects_version_transition_and_accepts_restored_pin`, native aynı-nesil kontrolü | İndirme/kurulu sürüm değiştirme/installer rollback uygulanmadı; H042 açık. |
| Klavye ve normal çıkış | Tab/Space ile ayrıntı kapandı; gerçek Sky çıkış/onayı exit0. State ve gerçek HKCU Run korundu; yalnız owned portlar kapandı, yabancı listener açık kaldı. | Native `keyboard-closed/exit-dialog/result.json` | H035'in tepsi menüsü kabulü değildir. |

Son native koşu: `work/proofs/compatibility/native-runs/c9259b0a-b9d3-4ad4-aea5-9740fc81a28c/`. Gerçek Windows girdisi yalnız Sky ile verildi. CDP gözlemi salt okunur committed public yanıtı strict decoder ve kanonik kaynakla karşılaştırır; IPC çağırmaz, yanıt veya native API değiştirmez. Model/sağlayıcı çağrısı0; auth girişi veya mevcut istemci ayarı yazımı yok. Yardımcı süreç normal tamamlandı, zorla kapanmadı; son pencere listesinde Headroom yoktu ve test motoru bırakılmadı.

## Son adayın yerel kapısı

| Kontrol | Sonuç | Programatik kayıt |
| --- | --- | --- |
| `pnpm.cmd test:app` |173 PASS;5 yeni compatibility testi dahil | `work/proofs/compatibility/post-review-final.json` |
| `pnpm.cmd build` | TypeScript + Vite PASS | Aynı kayıt |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | PASS | `reused-rust.json` → `final-1.json` |
| `cargo test --offline --manifest-path src-tauri/Cargo.toml -- --test-threads=1` |386 PASS,8 mevcut opt-in ignored | Aynı değişmeyen Rust girdileri/araç/OS bağı |
| `cargo build --offline --manifest-path src-tauri/Cargo.toml --features custom-protocol` | Son frontend'i içeren native build PASS | `post-review-final.json` |
| `pnpm.cmd test:a11y` |3 senaryo PASS;84 audit,0 ihlal;48 gezinme örneğinde p9521,8ms, eşik250ms | `work/proofs/accessibility/ui-5c51934c-954b-4f74-8997-ee61ebad2fb9/result.json` |
| Toplu H036 kabulü |28/28 PASS; kaynak/artefakt/kanıt ve araç/OS önce-sonra eşleşti | `work/proofs/compatibility/acceptance.json` |

Son tarama-busy düzeltmesi yalnız frontend ve izole fixture/test dosyalarını değiştirdi. Bu yüzden frontend, build ve UI kapıları yenilendi; etkilenmeyen386 Rust/fmt başarısı aynı girdiler ve araç zinciriyle programatik yeniden bağlandı. Kaynaklar, katalog/manifest, kullanılan EXE/dist/yardımcılar ve referans verilen kanıtların hash'leri otomatik kaydedilir; elle hash taşınmaz. Araçlar: Rust/Cargo1.92.0, Node25.2.1, pnpm11.25.0. TR-koyu/EN-açık son aday görüntüleri ana ajan tarafından incelendi; dört dil/tema birleşimi ve dar viewport karşılıkları otomatik kapsandı. Viewport karşılığı gerçek OS DPI diye sunulmaz.

İlk fixture/UI koşuları, eski iki keşif alanı ve audit öncesi async hazır olma yarışı nedeniyle geçmedi; ürün sözleşmesine uygun fixture ve deterministik bekleme ile düzeltildi. İlk native koşuda tarama sırasında eski satırların görünmesi bulundu ve son adayda giderildi. Bu eski koşular güncel kabul sayılmaz; eşikler düşürülmedi. Test altyapısının normal kapanış/state geri yükleme başarısı bu ürün hatasını gizlemedi.

## Destek ve yayın sınırı

Canlı doğrulandı etiketi yalnız katalogdaki kayıtlı Codex ChatGPT/Windows10 CLI0.166.0 ve Desktop26.1027.6604.0 kapsamları içindir. Gerçek API-key satırı denenmemiştir; Claude/OpenCode sentetik yerel fixture satırları canlı başarıya yükseltilmedi. Bu görev yeni sağlayıcı/model/subagent doğrulaması yapmadı. Windows11/macOS/Linux, temiz kurulum, imzalı yayın, uzun süreli kararlılık ve canlı performans kanıtı değildir.

README, uygulama sürümü, runtime manifest pini ve adaptör kaynak fingerprint'leri tutarlıdır. Kanal development; otomatik güncelleme ve kurulu sürüm geçişi sunulmaz. Gerçek imza/indirme/kurulum/migration/rollback H042'de, installer H037'de, canlı API-key H041'de açıktır. H035'in önceki performans/DPI kayıtları tarihsel adaylarını kanıtlar; H036'nın yeni binary/UI'sı için eski toplu H035 kabulü güncel diye sunulmaz. H035'e dönünce yalnız etkilenen kapsam yeniden değerlendirilir.

Uygulama ve kabul ana ajanda kaldı. Aynı tek Luna salt okunur kanıt raporunu ürettikten sonra kullanım limitine ulaştı; ana ajan kaynakla kontrol edip devraldı. Başka çalışan/model açılmadı. Geliştirme token tasarrufu ve çalışan süre avantajı ölçülmedi.
