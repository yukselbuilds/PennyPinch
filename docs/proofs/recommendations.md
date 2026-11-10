# H045 — Yerel öneriler kabulü

10 Kasım 2026. Windows 10 22H2 x64 yerel kabulü geçti. Ayarlar’daki **Yerel öneriler** seçeneği kapalı başlar. Açıldığında Genel bakış, yeterli kanıt olan önerileri gösterir. Yeni ölçüm veya kurulum kontrolü koşulu ortadan kaldırınca satır kaybolur. [Ölçüm ve tercih sözleşmesi](../decisions/029-local-recommendations.md) · [Kanıt kaydı](recommendations.json)

| Kontrol | Sonuç |
| --- | --- |
| Sinyal kuralları | Boş/kısmi/karışık ölçümler, sıfır veya negatif fayda, eski zaman, yinelenen origin, çakışma, farklı motor/model/rota ve eşik sınırı test edildi. Test fixture’ları yeni sağlıklı örneklerle önerinin kalktığını gösterdi. |
| Tercih ve IPC | Eksik alan kapalı başlar. Eski biçimde kayıt mevcut tercihi korur. Yanlış tip ve revision/fingerprint çatışması önceki kaydı korur. Kapalı istemci öneri sorgusu istemez. Eski ve geçersiz öneri cevabı görünmez. |
| Gerçek Windows uygulaması | QA geçmişindeki 1.000 → 1.250 token örneğinde açma ve kapatma, aynı negatif ölçüm üzerinde doğrulandı. Uygulama normal kapandı. Yeniden açılışta açık tercih yüklendi. İkinci geçerli ölçümle net 400 token azalma oluştu ve girdi artışı önerisi kayboldu. Kapatma tercihi kaydedildi ve uygulama yine kod 0 ile çıktı. |
| Arayüz | İzole Tauri transport ile gerçek App’de dört öneri, tek Etkinlik ve tek Uygulamalar eylemi, güncel ölçümle kalkma ve klavye doğrulandı. TR/EN, açık/koyu ve dar görünümde 88 erişilebilirlik denetiminde ihlal yok. Viewport kontrolü gerçek OS DPI kabulü değildir. |
| Son yerel kapı | 181 frontend ve 417 Rust testi, TypeScript/Vite, fmt ve embedded custom-protocol derlemesi geçti. 15 opt-in Rust testi ignored. 239 kalite ve 248 native girdi kendi koşularında aynı kaldı. |

| Before | After | Why |
| --- | --- | --- |
| Her ölçüm satırında ayrı Etkinlik düğmesi. | Bölüm altında tek Etkinlik ve gerektiğinde tek Uygulamalar düğmesi. | Özellikle dar ekranda tekrar ve klavye durakları azaldı. |
| Satır anahtarı liste sırasını içeriyordu. | Sinyal türü ve gerektiğinde istemciye bağlı sabit anahtar. | Bir öneri kaybolunca diğer satırların kimliği korunur. |
| Cache notunda kaynak ayrımı belirsizdi. | Sayımın sağlayıcı ve yerel yanıt cache isabetlerini birlikte kapsadığı yazıyor. | Ekrandaki oran motorun raporladığı alanı tarif eder. |

İlk native denemede sürücü açma adımında durdu. QA tercih dosyasında `true` kaydı doğrulandı. Beklemeli klavye akışıyla aynı QA binary’si tekrar kullanıldı. İlk kapanışın cleanup adımında dosya paylaşım kilidi görüldü. Daha sonra klasörü silme komutu otomatik denetimde `blocked by policy` nedeniyle reddedildi. Ayrı QA profili kapalı tercih ve iki sentetik ölçümle yerinde kaldı. Son iki koşu normal çıktı ve QA debug portları kapandı.

Gerçek uygulama testi ayrı QA kimliği ve yerel geçmiş veritabanı kullandı. Motor başlatılmadı. Cache ve sıkıştırma kuralları deterministik metadata fixture’larıyla sınandı. Model sağlayıcısına çağrı yapılmadı. Bu kabul Windows 11, üretim dağıtımı, canlı sağlayıcı etkisi veya yeni ayar açıkken eski ikiliye geri dönüşü kapsamaz.
