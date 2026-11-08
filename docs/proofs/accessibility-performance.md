# H035 — erişilebilirlik ve performans kanıtı

8 Kasım 2026; **kısmi, 33/51; H035 açık**. [Plan 023](../decisions/023-accessibility-performance.md), [ölçüm koşulları](../validation/performance.md), [inceleme](../reviews/H035.md). Ana ajan performans/runtime, gerçek Windows kontrolleri ve kabul sahibi; mevcut tek Luna yalnız salt okunur erişilebilirlik incelemesi yaptı. Geliştirme token tasarrufu ölçülmedi.

Skip bağlantısı, etkinlik filtre hatasının odağı/duyurusu ve açık tema kontrastı düzeltildi. UI pasif runtime sorgusu3s, native actor pasif health sorgusu2s oldu. Açık lifecycle komutları kanaldan hemen uyandırır ve kendi ack'ini gösterir;5s health geçerliliği, her istekte sahiplik kontrolü, auth/model/rota ve stream/CCR sözleşmeleri değişmedi.

Kanıtlar `work/proofs/accessibility/` altında. Son `checkpoint-acceptance.json` güncel kaynak/test/config, yerel kalite kapısı, UI sonucu, görüntüler ve native adayları önce/sonra SHA256 ve aynı araç/OS ortamına bağlar. `core_passed:true`, ana kabul `passed:false`; yalnız eksik tepsi menüsü listelenir. Önceki `core-acceptance.json` kapanış öncesinin tarihsel kaydıdır. Geçen testler tekrar çalıştırılmadı.

| Senaryo | Gözlenen | Kanıt / sınır |
| --- | --- | --- |
| TR/EN × koyu/açık × veri/boş/hata; dört ekran ve diyaloglar |68 axe audit,0 ihlal; klavye/ESC/focus, hata alert/focus PASS | `ui-36e174ba-f4b9-4c8a-a94f-a1463c7c008c/result.json`; gerçek App, izole transport; WCAG sertifikası veya gerçek ekran okuyucu oturumu değildir. |
| Son yerel kalite kapısı |168 frontend/379 Rust PASS,8 opt-in ignored; fmt/TS/Vite/native/a11y PASS | `native-final-3.json` + `reused-frontend.json` + `final-2.json`; yalnız native poll değişiminde ilgili native kapı yenilendi. |
| Motor kapalı/hazır boşta60s | UI7,27%/9,80%, motor0%/0,23%; tüm bellek eşikleri PASS | Native run `156bb28f-b191-440c-b2ce-04699664ea26`; CPU tek çekirdek eşdeğeri,59'ar örnek. Başlangıç/tarama ve model ısınması ayrı60s pencerelerdir. |
| İki ısınma +10 sıralı yerel istek |31.925 byte/128 satır; p95 36,09ms; UI private198,33MiB/motor1156,18MiB | Aynı run `load.json`; sağlayıcı çağrısı0, gerçek sağlayıcı/uzun yük iddiası yok. |
| Native Başlat ve izole UI gezinme | Hazır16,85s; gezinme p95 23,30ms | `startup.json`, UI sonucu; ready sorgulama gecikmesi dahil. |
| Gerçek dar pencere ve klavye |1015×711 CSS piksel, taşma yok; Shift+Tab/Enter ana içeriğe odak taşıdı | `small-720-final*`, `keyboard-skip-focused.json`, `keyboard-main.json`; hedef1024×720'den biraz daha dar native pencere. |
| Gerçek Windows125%/150% |120/144DPI ve DPR1,25/1,5; pause/resume, dört ekran, bağlantı diyaloğunun alt eylemleri/ESC erişilebilir | `os125*`, `os150*`, native görüntüler; Sky gerçek fare/klavye, native IPC değiştirilmedi.150%'de ekran pencere yüksekliğini650CSS piksele sınırladı. |
| Orijinal OS ölçeği | Settings100%, native96DPI/DPR1; Settings kapandı | `os100-restored*`, `display-restored-100.png`; WebView görünüm ölçeği100% kaldı. |
| Son ayrı native normal kapanışı | Gerçek Sky çıkış/onayı, exit0/forcedfalse; original state/protected/Run aynı, owned debug/stable port kapalı, yabancı listener canlı | Run `6d8f33c6-fe4e-4b2e-888b-ca92205c57c4/result.json`; Sky son pencereler listesinde Headroom yok. |

İlk iki CPU başarısız adayı raporda korunur. Üçüncü run'ın ölçüm ve DPI dilimleri geçti; ancak20 dakikalık test sınırı normal çıkış tıklamasından önce doldu. Run `result.json` bu nedenle `forced:true, passed:false`; bu koşu bütün olarak başarılı değildir. Orijinal state geri yüklendi, yabancı listener korundu. Sayısal faz ölçümleri kendi değişmeyen kaynak/binary ve yük fingerprint'leriyle geçerlidir; normal kapanış için ayrı son koşu gerekir.

Son ayrı koşu `6d8f33c6-fe4e-4b2e-888b-ca92205c57c4` aynı kaynak/binary/dist adayındadır. Sky sistem tepsisini hedeflenebilir pencere olarak sunmadığı için kullanıcıya Aç/Duraklat/Devam et denemesi soruldu; yanıt gelmedi. Pasif gözlem yalnız visible/enabled gördü; menü denemesi diye sunulmaz. Sonra ana ajan uygulamayı normal çıkış onayıyla kapattı ve helper orijinal state'i geri yükledi. Test motoru arka planda bırakılmadı. Yalnız tepsi menüsü kabulü açık; bu kanıt gelmeden H035 kapanmaz ve H036 başlamaz.

Windows10 x64 build19045 debug adayının yerel kanıtıdır. Windows11, macOS/Linux, installer, uzun süreli yük, gerçek sağlayıcı ve yardımcı teknolojiyle elle ekran okuyucu deneyimi bu kabulün kanıtlanmış alanları değildir.
