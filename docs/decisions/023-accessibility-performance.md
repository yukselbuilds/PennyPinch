# H035 — erişilebilirlik ve performans

8 Kasım 2026 başladı; 33/51 tamam, H035 açık. Ana ajan kabul, gerçek Windows/DPI/tepsi ve süreç sahiplikli performans ölçümünden sorumludur. Aynı tek Luna yalnız salt okunur erişilebilirlik kaynak incelemesi yapar; ürün değişikliğini ana ajan uygular. Ponytail full, Emil ve DESIGN.md korunur. Bir düzeltme dilimi beş dosyayı aşarsa davranışlara göre sırayla bölünür.

## Amaç ve korunacak sözleşmeler

Dört ana ekran ve bağlantı/kaldırma/istek ayrıntısı/çıkış diyaloglarında klavye erişimi, erişilebilir adlar, okunabilir kontrast ve dar/yüksek ölçek kontrolü. Ölçüm, model/auth, kullanıcı config'i, runtime rotası ve sahiplikli teardown değişmez. Mock veri yalnız açıkça izole UI testidir; canlı hesap veya tasarruf başarısı sayılmaz. Mevcut gerçek motor ve uygulama üzerinden ayrı CPU/bellek ölçülür. Yeni backend, auto tuning veya sürekli monitor eklenmez.

Test için axe-core sabit sürümlü geliştirme bağımlılığıdır; uygulamaya paketlenmez. Ana dört ekranda TR/EN/açık-koyu/boş-veri-hata denetimi, açık modallar ve export menüsü ayrıca kontrol edilir. Hedef WCAG2 AA ilgili otomatik kurallarında critical/serious ihlal olmaması; elle ad/focus/ESC ve önemli metin kontrastı incelemesiyle birlikte değerlendirilir. Otomatik kontrol WCAG sertifikası veya gerçek ekran okuyucu deneyimi sayılmaz.

## Önceden belirlenmiş ölçüm koşulları

Yerel Windows10 geliştirme binary'si, tek uygulama penceresi, ağ sağlayıcısı yerine yalnız loopback fixture. Referans CPU/RAM/OS/araç sürümleri fingerprint kaydına girer. UI grubu uygulama + kendi WebView süreçleri; motor grubu yalnız sahip olunan helper/Python/engine süreçleridir. Harici uygulamalar veya başka Codex oturumları örneklenmez. CPU yüzdesi duvar süresine göre bir mantıksal çekirdek eşdeğeridir; 16 çekirdeğe bölünüp küçük gösterilmez. Bellek private bytes ve working set ayrı tutulur.

Kısa koşuların eşikleri sonuçtan önce sabittir:

- Motor kapalı 60s ve hazır/boşta 60s: her grubun ortalama CPU'su bir çekirdeğin %10'unu aşmaz; UI private bytes en fazla 512MiB, motor en fazla 1536MiB.
- Isınma için iki istek, sonra sırayla saniyede en fazla bir adet olmak üzere 10 yerel chat-completions isteği. Her istek aynı sabit 128 satırlık sentetik araç JSON'unu taşır; içerik hash/boyut/satır sayısı kaydedilir. Isınmış loopback uçtan uca p95 en fazla 5s; UI private bytes en fazla 512MiB, motor en fazla 1536MiB. Bu sağlayıcı gecikmesi veya tüm iş yüklerinin performansı değildir.
- Native başlatma mevcut START_REPLY_TIMEOUT 100s sınırında kalır. Kısa UI gezinme tepkisi izole gerçek-App koşusunda p95 en fazla 250ms; ağ veya başlatma süresiyle karıştırılmaz.

İlk ölçümde hazır sinyalinden sonraki ilk 60s yanlışlıkla boşta pencere sayıldı. Sabitlenmiş 0.41.0 `proxy/server.py:1974` ve `:2204` model ısınmasının startup dönüşünden sonra daemon thread'de sürdüğünü gösterir; ölçümde de bir CPU sıçraması kaydedildi. Bu ilk başarısız sonuç korunur. Son adayda başlangıçtan sonraki sabit 60s ayrıca “ısınma” olarak raporlanır; ardından 60s boşta ölçülür. CPU/bellek/gecikme eşikleri aynı kalır, ısınma CPU'su boşta başarıya katılmaz. Bu ayrım yavaş/yoğun başlangıç maliyetini gizlemez; performans raporu iki evreyi birlikte verir.

İkinci koşunun motor-kapalı ilk dakikası da WebView açılışı/ilk taramayla çakıştı: ilk 10 örneğin UI ortalaması %41, sonraki pencereler %6–8 idi. Bu başarısız sonuç da korunur. Boşta ön koşulu artık gerçek committed Overview props'unda durmuş motor, hazır ölçüm ve tamamlanmış üç bağlantı okumasıyla doğrulanır (`frontend-settled.json`); bunun ardından yeni 60s pencere ölçülür. İlk açılış/tarama maliyeti ayrıca raporlanır; eski başarısız pencere PASS yapılmaz.

Başarısız ölçüm için eşik gevşetilmez; koşul/kök neden ve küçük düzeltme kaydedilir. Uzun süreli kararlılık, üretim release binary performansı ve gerçek sağlayıcı maliyeti bu koşuyla kanıtlanmaz.

## Kabul senaryoları

1. Dört ekran ve modallar → Tab/Shift+Tab/Enter/Space/ESC ile önemli eylemler erişilebilir, adlar ve focus dönüşü doğru; her iki tema/dilde kritik otomatik erişilebilirlik hatası yok. 1024×720 ve 125/150 ölçek görünümünde yatay taşma veya erişilemeyen kritik eylem yok. Gerçek OS DPI ile uygulama WebView zoom'u ayrı kaydedilir.
2. Native pencere/tepsi → gerçek Windows DPI bilgisi kaydedilir, erişim/geri açma ve uygun eylem/focus kontrol edilir. Test OS ekran ölçeği değiştirirse başlangıç değeri geri yüklenmeden kapanmaz. Kullanıcıya özgü kalıcı ayarlar/state korunur. Dış ortam sağlanamayan kabul açık/not_tested kalır; WebView zoom'u OS DPI kanıtına çevrilmez.
3. Yukarıdaki sabit veri/yük/süre/eşiklerle uygulama ve motor ayrı ölçülür; normal graceful kapanış, kendi port/süreçlerinin yokluğu, diğer listener ve orijinal state'in korunması. Son kaynak/config/test/araç/OS/binary fingerprint'i sonuçla aynı adımda. Son ürün adayında WORKFLOW yerel kapısı bir kez; değişmeyen başarılı bloklar yeniden çalıştırılmaz.

Kanıt `work/proofs/accessibility/`, tekrar çalıştırılabilir UI kapısı `tests/e2e/accessibility.spec.ts`, ölçüm koşulları/sonuçlar `docs/validation/performance.md`, kabul `docs/proofs/accessibility-performance.md`, inceleme `docs/reviews/H035.md`. H035 kabul edilince TASKS/checkpoint güncellenir, kullanıcıya bildirilir ve durulur; H036 başlamaz.

8 Kasım checkpoint: erişilebilirlik/yerel kalite kapısı, sabit performans fazları, gerçek Windows125/150%DPI ve100%'e dönüş geçti. Son ayrı native normal exit0/state/port koruması geçti; ölçüm koşusunun20dk sınırında zorla bitişi ayrıca FAIL olarak korunur. `checkpoint-acceptance.json` bütün geçerli girdileri bağlar; yalnız tepsi menüsü denemesi eksiktir. Sky sistem tepsisini hedeflenebilir pencere olarak sunmadı, kullanıcı yanıtı gelmedi; bu kabul gevşetilmez. H035 açık/33/51; H036 başlamaz.

## H036 sonrası H035 kapanış adımı —8 Kasım

Kullanıcının açık sıra istisnasıyla H036 tamamlandı; güncel toplam34/51. Kullanıcı PC başına döndü ve güncel adayda X→tepsiden aç→Duraklat→Devam et akışının tümünün çalıştığını bildirdi. Pasif committed kayıt aynı motor neslinde enabled/paused geçişlerini revision0→4 ile doğruladı; ayrı koşu normal exit0/state/Run/port korumasıyla bitti. Kanıt `work/proofs/compatibility/native-runs/2dd91219-d59e-47be-8ccb-c2b6d6a57fcd/`.

Ana ajan kalan kabulü sonuçlandırır; yeni çalışan veya ürün kodu değişikliği yok. H036 EXE/dist ve Uygulamalar ekranını değiştirdiği için eski performans adayını güncel saymak yerine aynı mevcut `native.py`, `settled.mjs`, `measure-settled.py` koşulları/eşikleriyle güncel adayın yerel fazları bir kez yenilenir. Kaynak/binary değişimi tekrarın somut gerekçesidir. Gerçek Windows125/150%DPI'de değişen Uygulamalar'ın kapalı/açık uyum ayrıntısı ve kritik eylemlere erişim kontrol edilir; başlangıç ölçeği geri konur. Değişmeyen diğer ekranların kayıtlı DPI kanıtı kendi kapsamıyla korunur. Güncel173 frontend/386 Rust/84 audit kapısı H036'nın aynı kaynak/araç/ortam fingerprint'leriyle yeniden kullanılır; ürün testleri tekrarlanmaz. Kısa performans koşusu uzun yük/sağlayıcı veya yayın garantisi değildir. Ana H035 kabulünden sonra durulur; H037 başlamaz.

Sonuç: güncel `93ce1a01-e141-42b3-b8fe-4afeea51fd0a` koşusu normal exit0 ile tamamlandı; off/ready UI7,54%/9,85%, motor0%/0,26%, aynı yükte p9535,88ms ve bellek eşikleri PASS. Gerçek125%/150%DPI Uygulamalar kontrolü ve100%'e geri yükleme geçti.46 teknik kontrol; son belge kaydı README durum düzeltmesini ürün değişikliğinden ayırır. H035 tamamlandı,35/51; H037 başlamadı. Cross-platform native Actions işleri H048/H049'da, mevcut ücretsiz kota/hard-stop doğrulamasıyla elle planlı; bu görevde CI çalıştırılmadı.
