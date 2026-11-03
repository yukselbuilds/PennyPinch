# 010 — İçeriksiz sayaç normalizasyonu

3 Kasım 2026. H023 uygulayıcısı ana ajan; token ölçümünün anlamı ve köken güveni uygulama kararlarıdır. Temel: H012-a, `docs/contracts.md` ve hash'leri eşleşen pinned 0.41.0 kaynak incelemesi. H012'nin diğer ürün kabiliyetleri bu kararla tamamlanmaz.

## Sözleşme

- Normalleştirici saf ve sınırlıdır; dosya/HTTP/model isteği veya kalıcı state yönetmez. H024 depolama ve dönemler, H025 ekran bağlantısı, H026 model/ajan atamasıdır. Bu görev mevcut uygulamanın/diğer sohbetlerin süreç veya rotalarını değiştirmez.
- Önce/sonra sayısı ayrı `Reading` içinde nullable token, sayaç türü, tokenizer kimliği ve kapsanan bileşeni taşır. Kimlikli aynı temel ve aynı bileşen olmadan fark/yüzde türetilmez. Kanıtlı çiftte `S=B−A`; `B>0` için `100×S/B`, aksi halde yüzde null. Gerçek sıfır, eksik sayı ve negatif fark ayrı kalır.
- Başlıca girdi metriğine yalnız Headroom'un mesaj veya bütün girdi çiftleri adaydır. Tool schema, novel, RTK, cache ve output ayrı kalır; sağlayıcı usage alanları girdi çiftine eklenmez. Bütün girdiyi ölçmeyen mesaj çifti bütün girdi diye etiketlenmez. Normalleştirici farklı temelleri bir toplamda birleştirmez; ağırlıklı dönem toplamı H024'ündür.
- Proxy köken anahtarı runtime nesli ve istek kimliğinden oluşturulur. Kimlikler düz metin tutulmaz, SHA-256 anahtara çevrilir. `origin_id` aynı isteğin ölçümlerini bağlar; `event_id` optimizer/bileşen slotunu da taşır, açık sabit etiketler kullanır. Mesaj/bütün girdi slotu ortaktır; örtüşen çiftler birlikte eklenmek yerine çatışır. MCP ancak güvenilir toplayıcının açık proxy köken eşlemesiyle aynı event'e bağlanabilir; aynı model veya benzer ID eşleme sayılmaz. Tam aynı tekrar bir event, çelişen tekrar bilinmeyen/conflict olur; belirsiz temeli/zamanı korunmaz. Başka runtime neslindeki aynı ID başka event'tir. Yeniden polling/kalıcı upsert H024'te bu anahtarları kullanır.
- UTC zaman doğrulanır. Yanlış kimlik/sayı/zaman sabit hata üretir; payload hata metnine kopyalanmaz. Token sayıları JSON/JS güvenli tamsayı aralığında, batch en çok10.000 kayıt, ham snapshot en çok2MiB'dir. Eksik zamanlı event dönem metriğine aday olmaz.

## 0.41.0 adaptörü

Motorun `recent_requests` alanı varsa onu, yoksa `request_logs` alanını projekte eder; ikisini birlikte saymaz. Yalnız istek ID/zaman ve açık sayısal alanlar alınır. İçerik, raw ID, auth, modelden ajan çıkarımı, URL/path, serbest tags, dönüşüm detayı ve lifetime toplamı taşınmaz. Snapshot toplamları event toplamına eklenmez.

Bu eski satırlar tokenizer kimliği veya güvenilir bileşen/provenance bilgisi taşımaz. Dolayısıyla `before/after` ham gözlem olarak korunur ama temel/bileşen bilinmiyor kalır; `has_exact_tokens`, motorun saved/% alanları veya tags bu güveni yükseltmez. H021'in `2306/12, saved0/%0` ve `12/12` satırlarından masaüstü tasarrufu türetilmez. Kanıtlı yeni bir toplayıcı daha sonra aynı saf API'ye açık temelli ölçüm sağlayabilir; bu görev motora yeniden token sayımı eklemez.

Recent pencere, geçmişin tamlığı veya bütün trafik sayılmaz; aggregate-only ve kimliksiz satırlar görünür metadata'dır. Kaynak reset/restart yeni nesil ve ayrı sayı olarak kalır; snapshot farkı ek kazanç değildir. Buffer boşluğu/eksik kapsamın depoda dönemlere taşınması H024'tedir. Eski lifetime51 istek/4471 saved bugünkü veriymiş gibi içeri alınmaz.

## Kabul

1. Açık aynı temelli120.000→75.000 ölçüm45.000/%37,5 verir; sıfır, sıfır taban, eksik ve artış korunur.
2. Farklı/bilinmeyen temel, H021 gerçek satırlarının sayı biçimi ve içerik canary fixture'ı uydurma yüzde veya içerik sızdırmaz.
3. Kökeni kanıtlı proxy/MCP tekrarı, çelişki ve iki runtime nesli doğru ayrılır; katman/aggregate sayaçları ana girdiyi şişirmez.

Odaklı Rust testleri → ana ajan incelemesi → değişen kaynakta son yerel kapı. Değişmeyen frontend kanıtı ancak kaynak/config/araç fingerprint'i eşleşirse yeniden kullanılır. Fixture sonucu canlı sağlayıcı/model/subagent veya performans kanıtı değildir.
