# H032 — tema ve iki dil kabulü

7 Kasım2026; **tamamlandı,31/51**. [Plan020](../decisions/020-theme-language.md), [inceleme](../reviews/H032.md). Ana ajan uygulama, kalıcı kayıt eşzamanlılığı ve kabul sahibi; mevcut tek Luna yalnız dar salt okunur lokalizasyon incelemesi yaptı. H033 başlamadı.

Mevcut Graphite açık/koyu token'ları ve TR/EN bileşen sözlükleri korundu; yeni i18n katmanı veya bağımlılık eklenmedi. Motor hatası metni, gösterilmeyen bir ham koda atıf yapmak yerine görünen çevrilmiş iletiye yönlendirir. Tema/dil kaydı mevcut atomik revision/fingerprint deposunu kullanır; root `lang`, `data-theme` ve CSS `color-scheme` aynı seçimi yansıtır.

Native açılışta kayıtlı dark tercihi okunamayıp system/light görünmesi incelendi. Ekran doğru biçimde busy hatası gösteriyor, kayıtlı dosyayı değiştirmiyor ve Yeniden oku sunuyordu. Kök neden aynı PreferenceStore klonlarının paralel okumasında sıfır beklemeli Windows named mutex'te çakışmasıydı. Paylaşılan std Mutex, aynı store'un okumalarını ve yazmalarını sıraya alır; mevcut cross-process kilit, CAS, atomik replace, doğrulama ve dış çatışma davranışı korunur. Bu düzeltme ana ajan tarafından yapıldı.

## Kanıt

Kayıtlar `work/proofs/appearance/` altında. `acceptance.json`26 kontrolle171 güncel kaynak/config girdisini, son binary/dist'i, UI bundle'ını ve araç/OS ortamını bağlar. Her koşuda sonuç ve önce/sonra fingerprint'ler programatik kaydedildi. Ham auth, prompt, HTTP gövdesi veya hata dökümü tutulmadı.

| Senaryo | Gözlenen | Kanıt / sınır |
| --- | --- | --- |
| TR/EN × açık/koyu × veri/boş/hata × dört ana ekran |48 durum, her biri1280/1024/819/683/507 genişlikte PASS; yatay taşma yok | `ui-a613a3ad-734e-4ccd-a14c-274bebf0cf1d/result.json`; gerçek App, izole typed transport ve açıklanmış örnek ölçümler.16 görüntü üretildi, ana ajan dört farklı ekran/tema/dil örneğini görsel olarak inceledi. |
| Render edilen sayı/yüzde/tarih/duration;0/artış/unknown, ayrıntı/ESC | TR/EN Intl çıktısıyla eşleşti, bilinmiyor sıfırlaşmadı | Aynı UI kaydı; Europe/Istanbul timezone, süre ayrıntısında12,5/12.5. Kaydedilen etkinlik listesi tam sayıya yuvarlar; ölçüm semantiği değişmedi. |
| System media açık→koyu→açık, manuel seçim, dil değişimi, başarısız save | System izledi, manuel dark sabit kaldı, root-lang güncellendi, hata önceki kayda döndü | Aynı UI kaydı; medya taklidi, Windows görünüm ayarını değiştirmez. |
| Aynı store'un4 okuyucu ×16 paralel okuması | Önce FAIL/busy, sonra tüm okumalar en/dark/revision1; dosya baytları aynı | `read-race-red.json`, `preferences-focused.json`:12 store testi PASS; stale/concurrent save ve CAS kontrolleri dâhil. |
| Son adayın tam yerel kapısı |153 frontend/363 Rust PASS,8 opt-in ignored; fmt/TS/Vite/custom-protocol build PASS | `final-2.json`; native'de bulunan somut yarış ilk adayı geçersiz kıldı. Düzeltme sonrası son adayın tam kapısı bir kez. Değişmeyen frontend UI PASS tekrar çalıştırılmadı. |
| Gerçek native dört tema/dil birleşimi ve iki dilde dört ekran | PASS; seçili değer, kaydedilmiş tercih, root-lang/theme ve renk paleti aynı | `native-runs/41c2d1c7-4a15-4ae4-8afd-64716e997d80/phase-*.json`; Windows input yalnız Sky mouse/keyboard, typed IPC yalnız salt okunur gözlem. |
| Manuel EN/light save→exit→reopen ve TR/system save→exit→reopen | İki yeniden açılışta kayıtlı seçenekler, “Preferences loaded.” / “Tercihler yüklendi.” ve doğru görünüm | Aynı native run; üç oturumun tümü exit0, debug ve stable port kapalı, zorla kapanış yok. Başlangıçtaki TR/dark da kullanıcı retry'si olmadan yüklendi. |
| State ve sahiplik | Runtime/metrics/preferences baytları geri yüklendi; protected state ve gerçek HKCU Run aynı, yabancı listener korundu | Aynı run `result.json`; model çağrısı0. İlk tanı run'ı `3c632979-7ff4-441e-bc95-7f5a60303afe` yarış görüldükten sonra yalnız owned job kapatılarak sonlandırıldı; state geri yüklendi, PASS sayılmadı. |

Yerel ortam Windows10 x64 build19045; Rust/Cargo1.92.0, Node25.2.1, pnpm11.25.0, Python3.14.0. Windows11 önceki H047/karar006 ücretsiz kullanım istisnasıyla not_tested. Native mevcut sistem tercihini salt okunur doğruladı; gerçek OS tema geçişi veya OS DPI testi yapılmadı. Diğer OS, installer/yayın, uzun yük ve canlı sağlayıcı kanıtı değildir. Global istemci/auth/model ayarı değiştirilmedi, geliştirme token tasarrufu ölçülmedi.

H032 kapatıldı; sıradaki H033 istatistik ve tanı dışa aktarma başlamadı. Ana görev sonunda duruldu.
