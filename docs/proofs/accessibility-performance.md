# H035 — erişilebilirlik ve performans kanıtı

8 Kasım 2026; **tamamlandı, 35/51**. [Plan023](../decisions/023-accessibility-performance.md), [ölçüm koşulları](../validation/performance.md), [inceleme](../reviews/H035.md). Ana ajan performans/runtime, gerçek Windows kontrolleri ve kabul sahibi; mevcut tek Luna yalnız salt okunur erişilebilirlik incelemesi yaptı. Geliştirme token tasarrufu ölçülmedi.

## Güncel adayın kapanışı

H036 uygulama kodunu ve binary'yi değiştirdiği için güncel adayda performans ve değişen Uygulamalar ekranının OS DPI kabulü yenilendi. `work/proofs/accessibility/h036-final-acceptance.json`46/46 kontrolü, `closure-acceptance.json` son belge değişimlerini bağlar. Güncel173 frontend/386 Rust/8 ignored ve84 audit/0 ihlal, aynı kaynak/araç/OS ile H036'dan yeniden kullanılır; ürün testleri tekrarlanmadı. Önceki toplu H035 PASS kaydı yeni binary'ye taşınmadı. Etkilenmeyen UI kaynakları ve App shell/focus bölümü programatik aynı bulunarak eski dar-pencere/klavye/diğer-ekran DPI kapsamı yeniden bağlandı.

| Güncel kontrol | Gözlenen | Kanıt |
| --- | --- | --- |
| Kullanıcı tepsi denemesi | X→tepsiden aç→Duraklat→Devam et tümü çalışıyor onayı; aynı nesil/revision0→4 içinde iki enabled/paused/enabled çevrimi | `work/proofs/compatibility/native-runs/2dd91219-d59e-47be-8ccb-c2b6d6a57fcd/manual-tray-user-report.json`, `tray-observations.json` |
| Motor kapalı/hazır boşta60s | UI7,54%/9,85%; motor0%/0,26%;59'ar örnek, sabit bellek sınırları PASS | Run `93ce1a01-e141-42b3-b8fe-4afeea51fd0a`, `idle_off_settled/idle_ready.json` |
| Isınma ve yerel yük | Ayrı60s warmup UI10,78%/motor15,09%;2+10 yerel istekte p9535,88ms, UI185,30MiB/motor1157,14MiB private | Aynı run `warmup/load.json`; aynı31.925byte/128satır sentetik girdi. Proxy12 isteği aldı; exact cache nedeniyle fixture upstream çağrısı1, dış sağlayıcı0. |
| Başlatma/gezinme | Gerçek Sky Başlat→ready16,72s; güncel UI p9521,8ms | `startup.json`, H036 `ui-5c51934c-954b-4f74-8997-ee61ebad2fb9/result.json` |
| Değişen Uygulamalar125%/150% |120/144DPI, DPR1,25/1,5; uyum aç/kapat/yenile12kayıt; kritik eylemlere yatay erişim, taşma yok | Aynı run `catalog125*`, `catalog150*`; gerçek Sky fare/klavye, API/IPC enjeksiyonu yok |
| Geri yükleme/kapanış |100%/96DPI/DPR1 geri döndü, Settings kapandı; her iki güncel native koşu normal exit0/forcedfalse | İki run `result.json`; state/protected/gerçek HKCU Run aynı, owned portlar kapalı, yabancı listener canlı, son Sky listesinde Headroom/Settings yok |

UI CPU hazır sonucu %10 sınırına yakındır; uzun süreli/release/sağlayıcı performansı veya kazanım yüzdesi çıkarılmaz. Son kapanışta yalnız belgeler değişti; README'nin H035 durumu düzeltildi, ürün girdileri aynı kaldı. Kaynak/kanıt/artefakt/araç/OS önce-sonra fingerprint'leri programatik kaydedildi. Tepsi görsel davranışı kullanıcı onayıdır; pasif DOM visibility tek başına pencerenin gizlendiği kanıtı sayılmadı. H035 tamamlandıktan sonra duruldu; H037 başlamadı.

## İlk adayın tarihsel kaydı

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

Tarihsel ayrı koşu `6d8f33c6-fe4e-4b2e-888b-ca92205c57c4` o kaynak/binary/dist adayındadır. O anda tepsi onayı gelmemişti; pasif visible/enabled gözlemi menü denemesi sayılmadı. Ana ajan normal kapattı ve state geri yüklendi. Bu eski pending kaydı korunur; güncel kullanıcı onayı, performans/DPI ve normal kapanış üstteki yeni kabulde tamamlandı.

Windows10 x64 build19045 debug adayının yerel kanıtıdır. Windows11, macOS/Linux, installer, uzun süreli yük, gerçek sağlayıcı ve yardımcı teknolojiyle elle ekran okuyucu deneyimi bu kabulün kanıtlanmış alanları değildir.
