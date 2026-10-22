# Tasarruf ölçümü — pinned kaynak incelemesi

**Tarih:** 19 Ekim 2026. **Kapsam:** paketlenmiş `headroom-ai==0.41.0` kaynak okuması; runtime/ürün testi değildir. H012 ve H023–H025 için tasarım girdisidir. Bu belge bu görevleri tamamlamaz.

## Ürün kararı

Genel bakışın ana metriği, aynı tokenizer ve aynı kapsamla ölçülen girdi çifti üzerinden hesaplanan **girdi token azalması**dır: `S=B-A`, `oran=100×ΣS/ΣB`. Miktar, yüzde, dönem ve ölçüm kapsamı birlikte değişir. İstek yüzdeleri ortalanmaz; eksik veri sıfıra çevrilmez. Pozitif tabanlı geçerli sıfır `%0`, `A>B` token artışı, taban yoksa `—` gösterilir.

Bu metrik isteklerin iletilen girdisini ölçer. Aynı konuşmanın geçmişi sonraki isteklerde yeniden taşınabilir; bu yüzden toplam çıkarılan token miktarı, benzersiz içerik veya fatura indirimi olarak adlandırılmaz. Cache ve yeni içerik oranları ayrı temellerdir. Bunların pay/paydasını birbirine karıştırmak yasaktır.

`agent_savings.py` içindeki coding profilinin `target_savings=0.50` değeri kaynakta nominal/display-only, `target_ratio` ise `None` olarak tanımlı. Bu sabit yüzde gerçek ölçüm, kalite garantisi veya bütün isteklerde beklenen tasarruf değildir; boş paneli bu değerle doldurmayız.

## Motordaki kaynaklar

Kaynak kökü: `work/proofs/packaging/artifacts/headroom-engine/runtime/Lib/site-packages/headroom/`.

| Kaynak / alan | Kaynaktaki anlam | Masaüstü için sonuç |
| --- | --- | --- |
| `proxy/outcome.py`, `RequestLog.input_tokens_original/optimized` | İstek düzeyindeki yerel öncesi/sonrası; toplu novel düzeltmesinden ayrı tutulur. | Aynı cetvelle karşılaştırılabilir çift için aday. Tamlık, yaşam döngüsü ve negatif değer davranışı fixture ile doğrulanmalı. |
| `/stats.tokens.input` | Sağlayıcı input sayımı varsa onu, yoksa yerel tahmini kullanır. Provenance sayaçları ayrıca vardır. | Yerel `saved` eklenerek gerçek yerel `B` türetilmez; tokenizer temelleri farklı olabilir. |
| `/stats.tokens.active_savings_percent` | Kaynaktaki ifade `all_layers_saved / (attempted_input_tokens + tool_search_saved_total)`; sıkıştırılmaya çalışılan içerikle sınırlıdır. | Ana kartta tüm istek tasarrufuymuş gibi gösterilmez. İleri ayrıntı olarak kendi adı ve paydası gerekir. |
| `/stats.tokens.proxy_savings_percent` | `proxy_compression_saved / proxy_total_before_compression`. | Ürün metriğinin yerine doğrudan geçirilmez; toplu payın novel davranışı ve input provenance'ı ayrıca kontrol edilir. |
| `/stats.tokens.savings_percent`, `all_layers_savings_percent` | `(proxy_compression_saved + tool_search_saved_total) / total_before_compression`. | Tool schema katmanı dahildir; katmanları tekrar eklemek çift sayımdır. |
| `/stats.tokens.new_input_savings_percent` | Aynı uygun istek grubunda `new_input_saved / (new_input + new_input_saved)`; provider uncached/cache-write kırılımı gerekir. | Yeni içerik için ayrı oran. Eksik kırılımda motorun verdiği `0`, masaüstünde kanıt olmadan gerçek `%0` sayılmaz. |
| `proxy/savings_tracker.py`, `display_session` | Kalıcı tracker'ın kendi inactivity süresiyle tanımlanan oturumu. | Masaüstü açılışı, istemci sohbeti ve gün filtresiyle eşitlenmez. Kendi başlangıç/bitiş kaynağı taşınır. |
| `savings_ledger.py` | Kaynak bazlı kalıcı event'ler, ayrı maliyet/cache temelleri; `saved=max(before-after,0)`. | Negatif fark yalnız bu `saved` alanından geri kazanılamaz. Hiç tasarruf etmeyen ve `new_input` taşımayan event atlanabilir; tüm trafik paydası bu ledger'dan varsayılamaz. |

`proxy/conversation_savings.py` ve `proxy/outcome.py`, tam geçmişi yeniden gönderen Responses isteklerinin birikimli kazancını, güvenilir konuşma kimliği varsa running totals için novel kazanca dönüştürür. İstek logları ile running totals bu nedenle her zaman aynı toplama sahip değildir. `previous_response_id` veya sunucu tarafı `conversation` kullanan artımlı istekler ayrıca ele alınır.

Konuşma eşlemesi için açık kimlik gerekir. Model, sistem talimatı veya `prompt_cache_key` tek başına sohbet/ajan kimliği sayılmaz. Kimlik yoksa bilinmiyor korunur. Bu kaynak davranışını bütün gerçek istemcilerin güvenilir kimlik gönderdiği şeklinde yorumlamayız.

## H012/H023 uygulama gereklilikleri

1. Ölçüm şeması kaynak/provenance, tokenizer temeli, runtime nesli, istek kimliği, zaman, kapsam ve ölçüm durumunu taşır. Provider usage, wire çifti, novel kazanç, tool-schema kazancı ve maliyet ayrı alan/temellerdir.
2. H023 öncesinde içeriksiz event adaptörü doğrulanır. `RequestLog` içinde `request_messages`, `compressed_messages` ve serbest tags bulunabildiğinden raw log/snapshot JSON'u diske veya UI'ya kopyalanmaz. Alanlar allowlist ile seçilir; auth, içerik ve kişisel yollar alınmaz.
3. Kaynak polling ise aynı event bir kez alınır; ring buffer boşluğu, restart/sayaç sıfırlaması ve kayıp aralığı görünür olur. Snapshot farkları ile event toplamları birbirine eklenmez. Tamlık bilinmiyorsa ölçülebilen alt küme açıkça belirtilir.
4. Proxy, MCP ve alt ajan aynı olayı raporlayabilir. Kaynakları körlemesine birleştirmek yerine sahiplik ve event kökeniyle dedup yapılır; güvenilir eşleme yoksa ayrı kapsam sunulur.
5. Her dönemde sıfır tasarruflu geçerli istekler de paydaya girer. Yalnız kazançlı olayların oranı bütün trafiğin oranı olarak sunulmaz. Yuvarlama toplamdan sonra yapılır.
6. Sabit model/reasoning korunur. Cache isabeti, gerçek net maliyet ve abonelik kotası hakkında yerel token azalmasından sonuç çıkarılmaz. Retrieval/yeniden deneme ek girdileri kapsamda ayrı gözlemlenir; çıkarılan içerik net kazanç diye sunulmaz.
7. `RequestLog` çiftinin hangi bileşenleri saydığı doğrulanır: mesaj gövdesi, sistem metni ve tool schema kapsamları varsayılmaz. `outcome.py`, tool-schema kazancının tags üzerinden ayrı taşındığını ve `tok_before/after` çiftini değiştirmediğini söylüyor. Ayrı katmanın payını eklemek ancak örtüşmeyen, karşılaştırılabilir öncesi/sonrası temeli de kurulursa mümkündür; mesaj oranına yalnız ek kazanç yığılmaz. Daha dar ölçüm bütün girdiymiş gibi etiketlenmez. Kapalı output shaper için output tasarrufu ana girdi metriğine eklenmez.

Odaklı fixture kapsamı: farklı tokenizer sayımları; kazançlı ardından kazançsız istek; negatif fark; `B=0`; eksik usage; aynı event replay; runtime restart; buffer boşluğu; açık kimlikli iki konuşma ve kimliksiz trafik; Responses tam geçmiş/artımlı ayrımı; proxy/MCP tekrar bildirimi; katman çift sayımı; TR/EN ve seçilen dönem tutarlılığı.

## İnceleme fingerprint'leri

SHA-256 değerleri bu kaynak okumasının sürümünü sabitler; davranış testi PASS'i değildir. Motor güncellenirse yalnız etkilenen anlamlar yeniden incelenir.

| Göreli kaynak | SHA-256 |
| --- | --- |
| `proxy/server.py` | `e80fdf59cdede96bfd86541ec22ee4aa98f5d7a3cea8b34e316cd0628c01f8d2` |
| `proxy/conversation_savings.py` | `31ab5aa5c0a0c71f5b3941b36d28162505cfd1f9f84153cb5859edad39c396f5` |
| `proxy/outcome.py` | `5d7ad234e0caab9f7adc305de6e1fe5c89ba5e99ebad2d991c75d910ef8b43b6` |
| `proxy/prometheus_metrics.py` | `8a25b94a0a4c3fe0657810d16326ff74d32538928e07ae82bb4c0290cecfc4b8` |
| `proxy/savings_tracker.py` | `ae6a75c99b98388e44670ee2b57bfab25fc6bada1c963b83baa94e940feb6daa` |
| `savings_ledger.py` | `361b011c256839ed4ddbbabae0f141c3aa735ca82688724775424b1f3be5e899` |
