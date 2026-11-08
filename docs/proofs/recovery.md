# H034 — hata ve toparlama kabulü

8 Kasım 2026; **tamamlandı, 33/51**. [Plan 022](../decisions/022-recovery.md), [inceleme](../reviews/H034.md). Ana ajan backend/entegrasyon/sahiplik ve kabul sahibi; mevcut tek Luna strict istemci/yardım arayüzü dilimini tamamladı. H035 başlamadı.

Genel bakış ve Uygulamalar'ın sonunda “Bağlantıyı denetle” motoru, kalıcı girişi, üç uygulamanın kayıtlı ayar kapsamını ve son yedi günlük yerel trafik işaretini ayırır. Teşhis bir kullanıcı eyleminde tek okumadır; pending sırasında tek istek korunur. Config/auth yazımı, model/istemci çalıştırma, otomatik bypass/onarım yoktur. Sonraki adımlar mevcut lifecycle veya açık bağlantı önizlemesine götürür.

Kaydedilmiş hata varsa kaynak ancak gözlenebildiği ölçüde açıklanır; bu sürümde hata katmanı unknown kalır. İstemcide 401/403 veya giriş hatası varsa o istemcide oturum açma yönlendirmesi vardır; auth sırrı okunmaz. Hiç trafik görülmemesi kaçak veya sıfır tasarruf sayılmaz; hata bildirilmemesi başarılı istek etiketi almaz. Yerel ayar eşleşmesi sağlayıcı, model, ana/alt ajan veya sohbet doğrulaması değildir.

## Kanıt

Kayıtlar `work/proofs/recovery/` altında. `acceptance.json`, 18 kontrolle 187 kaynak/config girdisini, son binary/dist ve UI bundle'ını aynı araç/OS ortamına bağlar. Sonuçla önce/sonra SHA256 aynı adımda tutulur; kabul edilen girdilerde drift yoktur. Başarılı kalite kapısı tekrar çalıştırılmadı.

| Senaryo | Gözlenen | Kanıt ve sınır |
| --- | --- | --- |
| Strict IPC, korelasyon, alan/sayım/tarih/durum tutarlılığı, raw error bastırma, browser unavailable | 7 yeni client testi PASS; unavailable'da invoke yok, tek istek serbest bırakılır | `worker-final-20261108-358a1eb17f71448b869994f8b0c65245.json`; root gerçek DTO ve bağlı akışı inceledi. Son tam Node kapısı güncel root adayını içerir. |
| Üç adaptörde sahip olunan config, farklı endpoint/bozuk config, auth canary, exit busy, Claude farklı proje kapsamı | Ayarlar ve ownership fingerprint'i aynı; çakışma/kapsam belirsizliğinde eşleşme bildirilmez | Gerçek Windows dosya fixture'ları; `focused-4.json` temel 7 Rust testi, son kapıda scan yarışına karşı ek test dahil 8 yeni Rust testi. |
| Kurulum metadata değişimi/kayıp dosya/yeniden tarama; rapor sırasında yeni scan | Eski current/compatibility kullanılmaz; farklı scan'de ilk sayım korunup unknown olur | Kaynak `discovery_tests.rs`, `recovery_tests.rs`; dosya çalıştırılmaz. Metadata eşitliği hash veya protokol kanıtı değildir. |
| Eksik runtime/failed/timeout public durumları | Gerçek manager raporu doğru kategori verir; generation/engine başlatılmaz, yabancı listener canlı | `manager_tests.rs` enjeksiyonu; gerçek motor sürecini öldürme veya canlı sağlayıcı arızası değildir. |
| Son yerel kalite kapısı | 168 frontend/379 Rust PASS, 8 opt-in ignored; fmt/TS/Vite/custom-protocol build PASS | `final-1.json`, aynı aday önce/sonra fingerprint'i ve araç/OS. İlk fixture compile hatası düzeltildi; başarısız koşu PASS sayılmadı. |
| TR/EN × koyu/açık × 1280/1024/819/683/507; klavye/focus, hata/pending, tarih, gezinme/unmount | 20 birleşim ve 8 davranış grubu PASS; overflow/page error yok | `ui-b73bf4d8-088b-48e1-9489-5a2669607e84/result.json`; gerçek App, açıkça izole örnek transport. Root TR-dark/EN-light render'larını inceledi. |
| UI yeniden tarama ve motor/config bağlamı değişimi | Eski rapor gizlenir, stale açıklaması ve açık tekrar denetleme vardır | Son UI koşusu; ilk denemede eksik scan-generation bağlantısı bulundu ve gerçek `Applications → App → ProblemNotice` akışında düzeltildi. |
| Gerçek native teşhis tıklaması | Stopped motor/ready giriş; start sırasında eski rapor ve disabled; sonra ready/ready | Run `b4cbd558-306c-497e-8387-87c2f22465b6`: `stopped-check.json`, `engine-changed.json`, `ready-check.json`. Gerçek Sky mouse/keyboard; passive observer sonuçları değiştirmedi, native invoke immutable kaldı. |
| Gerçek native sonraki adım | Uygulamalar'a gitme ve bağlam değişiminde önceki raporun geçersizliği | `applications-context.json`; art arda native rescan yapılmadı, o davranış son gerçek-App UI testinde doğrulandı. |
| Native frontend/backend sözleşmesi | Üç committed gerçek yanıt strict UI decoder'ından geçti | Aynı run `decoder-acceptance.json`; yeni veri çözümü, IPC/sağlayıcı çağrısı yok. |
| Kapanış ve sahiplik | Normal exit0, debug/stable own portlar kapalı, yabancı listener canlı; orijinal runtime/metrics/preferences baytları geri yüklendi | Aynı run `result.json`, `baseline.json`; protected state ve gerçek HKCU Run aynı. Son çıkış tıklaması ayrıca gözlenmedi; kapanış helper process/port/exit kaydıyla doğrulandı. |

Native koşu boş izole ölçüm deposunu kullandı; **sağlayıcı çağrısı 0**. Gerçek kullanıcı ayarı veya auth değiştirilmedi. “Son trafik” kayıtlı Headroom katmanındaki en yeni zamanlı istekten gelir; sınırlı görünümde kesin yokluk çıkarılamazsa unavailable kalır. Rapor içeriğinde path, raw log/error, auth, model/provider etiketi, owner/generation/sohbet kimliği yoktur.

Ortam Windows10 x64 build19045, Rust/Cargo1.92.0, Node25.2.1, pnpm11.25.0, Python3.14.0. Native dil TR; EN izole UI/client ile doğrulandı. Windows11 H047/karar006 ücretsiz istisnasıyla not_tested; diğer OS, installer, uzun süreli yük, gerçek sağlayıcı auth hatası ve gerçek motor kill testi yapılmadı. H035 erişilebilirlik/performance ve H041 yayın kapıları kendi kapsamlarında açıktır. Geliştirme token tasarrufu ölçülmedi.

H034 tamamlandı; sıradaki H035 başlamadı ve ana görev sonunda duruldu.
