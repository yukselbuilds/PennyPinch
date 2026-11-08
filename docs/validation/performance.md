# H035 — yerel erişilebilirlik ve performans ölçümü

8 Kasım 2026; **H035 tamamlandı**. [Plan023](../decisions/023-accessibility-performance.md), [güncel kabul](../proofs/accessibility-performance.md). Bu kayıt Windows10 geliştirme binary'si içindir; yayın/installer, uzun süreli yük veya canlı sağlayıcı performansı değildir.

## Sabit koşullar

Intel Core i7-10875H, 8 çekirdek/16 mantıksal işlemci, yaklaşık 16GB RAM; Windows10 x64 build19045, 1920×1080 ekran. Referans donanım ve orijinal OS ölçeği100% `work/proofs/accessibility/reference.json` içinde kaydedildi. CPU yüzdesi bir mantıksal çekirdeğe göre hesaplanır; 16'ya bölünerek küçük gösterilmez. UI grubu Headroom + kendi WebView süreçleri; motor grubu kendi job'ındaki diğer helper/Python/konsol hostlarıdır. Motor kapalıyken debug konsol hostunun küçük bir bellek tabanı vardır.

Ölçüm, yalnız elde tutulan test job'ındaki süreçlerin GetProcessTimes/GetProcessMemoryInfo değerlerini 1s aralıkla okur. Başka uygulama/oturum, komut satırı, auth veya içerik kaydedilmez. Private bytes ve working set ayrı tutulur. Yerel fixture POST yanıtı gerçek sağlayıcı performansı veya maliyeti sayılmaz.

Eşikler: hazır/boşta60s ve motor kapalı/boşta60s için grup başına ortalama CPU≤%10 tek çekirdek; UI private≤512MiB, motor private≤1536MiB. İki ısınma + 10 sıralı/saniyede en çok bir 128 satırlık sentetik araç JSON isteğinde ısınmış yerel p95≤5s, aynı bellek sınırları. Native hazır sinyali≤100s; izole App gezinme olayından ikinci frame'e p95≤250ms. Eşikler sonuçtan sonra yükseltilmedi.

## Güncel H036 sonrası aday

Koşu `93ce1a01-e141-42b3-b8fe-4afeea51fd0a`: tamamlanmış frontend ardından60s motor-kapalı UI7,54%/motor0%; ayrı60s model warmup UI10,78%/motor15,09%; sonrasında60s ready UI9,85%/motor0,26%. Her faz59 örnek. Warmup CPU'su boşta sınırına dahil edilmedi; eşikler korunur.2+10 aynı yerel istekte p9535,88ms, peak private UI185,30MiB/motor1157,14MiB; load CPU13,28%/4,89% için boşta eşiği uygulanmaz. Proxy12 istek; exact cache nedeniyle yerel fixture upstream1, dış sağlayıcı0. Sky Başlat→ready16,72s; kaynak/binary hash'leri aynı adımda kaydedildi.

Değişen Uygulamalar125%/150% gerçek Windows ölçeğinde120/144DPI ve DPR1,25/1,5 ile aç/kapat/yenile kontrolünü geçti.100%/96DPI/DPR1 geri yüklendi. Hem bu run hem kullanıcı tepsi run'ı normal exit0/forcedfalse, owned port/state/Run korumasıyla bitti. Güncel kaynak kapısı173/386/84, eşleşen girdilerle yeniden kullanıldı. Diğer ekranların değişmeyen ilgili kaynakları ve shell/focus bölümü programatik eşleştirildi; tarihsel dar-pencere/klavye/DPI kapsamı yalnız bu alanlarda yeniden kullanıldı.46 teknik kontrol ve son belge kabulü kanıtı bağlar. UI CPU sınırına yakınlık ve kısa debug koşusu nedeniyle uzun yük/release kararlılığı iddia edilmez.

## Tarihsel bulgular ve geçerlilik

| Koşu | Gözlenen | Karar |
| --- | --- | --- |
| İlk aday `c7de7336-3531-4dcb-9a46-3f66849288eb` | Motor hazırdan sonraki ilk60s: UI22,41%, motor14,72%; load10 istek (artı2 warmup), en yavaş41,55ms | İlk CPU penceresi FAIL, başarı sayılmaz. Arka plan model ısınması ayrıca saptandı. |
| İkinci aday `b2e52781-fd3c-41e2-be28-a6ead122eeb5` ilk motor-kapalı60s | UI13,36%; ilk10 örnek41,10%, sonraki10 örneklik pencereler yaklaşık6–8% | Açılış/tarama işiyle çakıştı; başarısız kayıt korunur. |
| Aynı ikinci aday, gerçek frontend yüklemesi bittiği doğrulanan motor-kapalı60s | UI5,50%, motor0%; ardından ayrı model warmup60s UI12,86%/motor14,61% | Boşta off PASS; ısınma ayrı raporlanır, CPU boşta sınırına dahil edilmez. |
| İkinci aday, ısınma sonrası hazır/boşta60s | UI13,51%, motor0,47% | UI CPU hedefi FAIL; salt başlangıç maliyeti diye açıklanmadı. |
| Son düzeltme | UI pasif poll1s→3s; native pasif health poll1s→2s | Açık komutlar kendi ack'iyle anında güncellenir. Per-request ownership ve5s sağlık geçerliliği aynı. |
| Son aday `156bb28f-b191-440c-b2ce-04699664ea26`, açılış/tarama60s | UI12,37%, motor0%; model warmup60s UI10,58%/motor15,34% | Başlangıç maliyeti; boşta CPU başarı penceresi değildir. |
| Son aday, gerçek frontend tamamlandıktan sonra motor-kapalı60s | UI7,27%, motor0%;59 örnek | Sabit boşta CPU ve bellek eşikleri PASS. |
| Son aday, model ısınması sonrası hazır/boşta60s | UI9,80%, motor0,23%;59 örnek | Sabit boşta CPU ve bellek eşikleri PASS; UI sonucu sınıra yakındır. |
| Son aday, iki ısınma +10 sıralı yerel istek | 31.925 byte/128 satır; p95 ve en yavaş36,09ms; UI private198,33MiB, motor1156,18MiB | Yerel yük gecikmesi ve bellek PASS; yük CPU'su UI13,16%/motor2,61%, boşta sınırı bu pencereye uygulanmaz. |
| Son aday, gerçek Sky Başlat → loopback ready | 16,85s; UI gezinme p95 23,30ms | Başlatma100s ve gezinme250ms eşikleri PASS. Ready gözlemi sorgulama gecikmesini içerir. |

Başlangıç için iki ayrı sabit60s pencere (arayüz/tarama ve model ısınması) saklanır. Boşta ölçümleri gerçek committed frontend hazır koşulundan ve model ısınma penceresinden sonra başlar. İlk başarısız pencereler silinmedi veya PASS'a çevrilmedi. İlk/son koşul farklılıklarından kaynaklanabilecek etkiler nedeniyle bu kayıttan bir performans kazanım yüzdesi çıkarılmaz.

## Erişilebilirlik

Proje içi `pnpm.cmd test:a11y` gerçek App'i izole transport ile çalıştırır. axe-core4.14.0 yalnız geliştirme bağımlılığıdır; native API guard test bundle'a gerçek Tauri modülü girmesini reddeder. Son frontend adayı `ui-36e174ba-f4b9-4c8a-a94f-a1463c7c008c`: 68 audit, 0 ihlal; TR/EN/açık-koyu/veri-boş-hata, dört ekran ve diyaloglar. Etkinlik hata focus/alert, Tab/Shift+Tab/Enter/ESC ve focus dönüşü doğrulandı. Gezinme p95 23,30ms; 1024×720'nin100/125/150 ölçek karşılığı viewport'larında eylemler yatay taşmadı. Bunlar gerçek OS DPI yerine sunulmaz; otomatik sonuç WCAG sertifikası veya elle ekran okuyucu deneyimi değildir.

Gerçek Sky fare/klavye ile native pencere 1015×711 CSS piksele daraltıldı; bu, 1024×720 hedefinden biraz daha kısıtlıdır. Skip bağlantısı görünür odak aldı ve Enter ana içeriğe taşıdı. Gerçek Windows Settings üzerinden125%/150% uygulandı: GetDpiForWindow120/144 ve WebView DPR1,25/1,5 birlikte kaydedildi; uygulama görünüm ölçeği100% kaldı.125%'te duraklatma,150%'de devam etme, dört ekran, bağlantı diyaloğunun alt eylemleri ve ESC ile kapanması gözlendi.150%'de ekran alanı pencere yüksekliğini650 CSS piksele indirdi; yatay taşma yoktu. Native gözlem TR/koyu kapsamındadır; diğer dil/tema birleşimleri otomatik UI kapısındadır.

Tarihsel Windows Settings100%, native96DPI/DPR1 olarak geri alındı ve Settings kapatıldı. Kanıt `os125*`, `os150*`, `os100-restored*`, `keyboard-*` JSON'ları ve native görüntülerdir. O ölçüm koşusu20dk sınırında zorla kapandı; bütün run FAIL kalır. Ayrı o-aday kapanışı normal exit0 verdi; bu eski kapanış yeni binary'nin sonucu sayılmadı. Güncel kullanıcı tepsi onayı ve normal kapanış üstteki yeni koşularla tamamlandı; test motoru arka planda bırakılmadı.
