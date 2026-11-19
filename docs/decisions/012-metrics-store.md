# 012 — Yerel metrik deposu ve dönem sorgusu

7 Kasım H031 eki: varsayılan30 korunarak7/30/90 kullanıcı tercihi eklendi. SQLite schema2, ölçümler korunarak schema1'den taşınır; tek cutoff eski tarihli replay'in süre uzatılınca dönmesini önler. Aşağıdaki ilk H024 kabulü tarihseldir; güncel karar [019](019-operating-preferences.md).

4 Kasım 2026. H024 uygulayıcısı ana ajan; kalıcı kayıt ve ölçüm semantiğinin sahibi ana ajandır. Tek Luna xhigh çalışanı yalnız sabitlenmiş ağırlıklı toplam sözleşmesinin birim testlerini yazar.

## Kapsam ve sınırlar

- H023 schema1 normalize event'leri yerelde saklanır; salt okunur native sorgu H025 ekranı için dönem toplamlarını döndürür. Canlı collector/polling, gerçek model isteği, ekran, model/ajan ataması ve hesap/sohbet listesi bu göreve dahil değildir. Kaynak metadata'sındaki temel belirsizliği yükseltilmez.
- SQLite transaction/upsert kullanılır. Tüm geçmişi her polling'de yeniden yazan JSON deposu yerine event_id anahtarı ve zaman indeksi seçilir. `rusqlite=0.41.1`, bundled SQLite ve mevcut kilitteki `chrono=0.4.45` doğrudan sabitlenir; sistem SQLite kurulumu gerekmez. [rusqlite](https://docs.rs/rusqlite/0.41.1/rusqlite/) ve [paket metadata'sı](https://crates.io/crates/rusqlite/0.41.1) 4 Kasım'da kontrol edildi; MSRV1.89 mevcut Rust1.92 ile uyumludur. Cargo.lock indirilen paket checksum'larını tutar.
- Kayıt sadece H023'ün sınırlı numeric/köken metadata'sını içerir. Prompt, yanıt, auth, dosya yolu, URL ve serbest motor tags alanı alınmaz. UI'ya event yazma/import komutu verilmez; kalıcı yazım crate içindeki normalizasyon/toplayıcı sınırındadır.
- Uygulamaya ait metrik dizini kullanılır; constructor ve boş sorgu dosya yaratmaz. Var olan reparse/link/çoklu hardlink dosyaları, yabancı/veri tabanı şeması ve bozuk kayıtlar sabit hata verir; sessiz sıfırlama/üzerine yazma yoktur. Dizin pin'leri mevcut no-delete korumasını kullanır. SQLite transaction yarım yazımı geri alır; başka uygulamanın dosyası sahiplenilmez.

## Kimlik, dönem ve saklama

- `event_id` upsert anahtarı; aynı `origin_id` isteğin katmanlarını bağlar. H023'ün tekrar/çatışma birleştirmesi depo ile ortaklaştırılır. Kaynak kaynaklı aynı ölçüm bir kez kalır; çelişki bilinmeyen olur ve sonraki iyi replay ile tekrar ölçülmüşe dönmez.
- Oturum, bu uygulama sürecinin **izleme oturumudur**; istemcinin sohbeti veya hesap oturumu değildir. İlk kaydın izleme oturumu replay/reopen ile değiştirilmez. Motor nesli event'te kalır; farklı nesilde aynı istek yeni event'tir. Uygulama yeniden açılınca yeni izleme oturumu, korunmuş bugün/7 gün geçmişi vardır.
- Bugün, işletim sistemi yerel takvimindeki bugünün başlangıcından şimdiye kadardır. Son 7 gün, bugünü ve önceki altı yerel takvim gününü kapsar. Kayıtlar UTC saklanır. Dönem sınırı ve günlük grafik sorguda aynı yerel gün başlangıcını kullanır. İki karşılığı olan gece yarısında erken an seçilir. Gece yarısı veya bütün gün atlandıysa ilk geçerli yerel an kullanılır. Saat dilimi sınırı çözülemezse sorgu hata verir. Gelecek zamanlı kayıtlar dönem toplamına eklenmez.
- Varsayılan saklama **30 gün**; UTC yaşına göre eski metadata işlem içinde temizlenir. Zamanı olmayan kayıt için ilk görülme zamanı kullanılır. Saklama süresi sorgu yanıtında görünür; ayar ekranı H025/sonraki ayarlar işidir. Otomatik disk kurtarma veya güvenli adli silme iddiası yoktur.
- Depo sınırları açıktır: en çok100.000 event,128MiB SQLite dosyası ve H023 batch sınırı. Kapasite/izin/bozuk şema hatasında işlem geri alınır; kısmi başarı veya eksik veri tam trafikmiş gibi dönmez. Yük/uzun süre performans kabulü H035/H041'dedir.

## Toplam ve kapsam

- Yalnız main_metric_eligible Headroom girdi çiftleri; aynı sayaç türü, tokenizer kimliği ve bileşen içinde toplanır. Birden çok temel/bileşen ayrı grup olarak kalır; birleşik ana sayı bilinmiyor olur. Provider usage, RTK/cache/output ana girdiye eklenmez.
- Grup ayrıntısı en çok64 satırdır; kalan sayı `omitted_basis_groups` alanında görünür. Farklı temeller, taşma kontrolü yapmak için bile tek sayıda toplanmaz; mixed_bases önceliği korunur. Grup içi JS sınırı taşması ayrı counter_overflow'dur.
- Ağırlıklı oran `100 × (ΣB − ΣA) / ΣB`; istek yüzdeleri ortalanmaz. `B=0`, boş veya karşılaştırılamayan veride yüzde null; gerçek sıfır ve negatif değer korunur. JS güvenli tamsayı sınırını aşan toplam bilinmeyen/overflow olur.
- Saklanan/karşılaştırılabilir/dışlanan/zamanı yerleştirilemeyen event adetleri ve rolling/aggregate/kimliksiz kaynak uyarıları ayrı döner. Ölçülebilen alt toplam tüm trafik değildir; trafik tamlığı kanıtı yokken `complete_traffic=false` kalır. Raw lifetime/snapshot aggregate tasarrufu ek event yapılmaz.

## Kabul ve doğrulama

1. Yeni kayıt → aynı proxy/MCP replay → gerçek dosyadan yeniden açma: tek event ve korunmuş dönem toplamı; yeni izleme oturumuna eski event taşınmaz.
2. Yerel gece yarısı/7 takvim günü sınırı ve kısmi veri → `%14` ağırlıklı örnek, boş/sıfır/negatif/karışık temel/taşma ayrımı; kayıtsız veya belirsiz zaman tam kapsama sayılmaz.
3. Retention, çelişen replay, yabancı şema, DB kilidi/yarım işlem ve güvensiz dosya → sabit hata veya doğru kapsam; mevcut veri bozulmaz.

Önce odaklı Rust RED/GREEN; ana ajan fark/bağlı akış incelemesi; son adayda tek yerel Rust fmt/test/embedded build kapısı. Değişmeyen frontend test/build kanıtı yalnız ilgili kaynak/araç fingerprint'i eşleşirse tekrar kullanılır. Yeni native sorgu gerçek uygulama IPC yolunda model çağrısı olmadan kontrol edilir. Fixture metrikleri gerçek kullanıcı tasarrufu sayılmaz.
