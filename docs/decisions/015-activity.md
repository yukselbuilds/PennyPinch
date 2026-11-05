# 015 — İçeriksiz etkinlik ve ayrıntı

5 Kasım 2026. H027 tamamlandı;25/51. Ana ajan kaynak/metadata, kalıcı uyum, sorgu/parser ve kabul sahibidir. Mevcut tek Luna sabit props'lu sunumu yazdı; durdurulduğu doğrulanıp devralındı. H028/H033 ve hesap/sohbet görevleri başlamaz. [Kabul](../proofs/activity.md).

## Kaynak ve semantik

Pinned0.41.0 RequestLog total_latency_ms (isteğin wall-clock süresi), optimization_latency_ms, cache_hit, transforms_applied ve nullable error içerir. Recent_requests25 satırında zaman/transform vardır; error/cache yalnız son10 request_logs satırında bulunur. Birincil recent listesi event kaynağı olarak korunur; ikincil metadata yalnız aynı request_id + timestamp ve çelişmeyen model eşleşmesinde tamamlanır. İki liste iki kez sayılmaz. RequestLog ilk-token süresi yayımlamaz; raw tags veya isimsiz TTFT alanından doldurulmaz.

Error metni deserialize sırasında yok sayılır; sadece alanın bilinmiyor/null/dolu durumu sabit enum olur. Null/boş hata kaydı başarı veya tamamlanmış stream kanıtı değildir. cache_hit yalnız kaynak bildirimi, sağlayıcı cache-read indirimi değildir. Transform adları serbest metin olduğundan saklanmaz; yalnız bounded adet korunur. Katman kaydı ve raporlanan dönüşüm, bütün uygulama konfigürasyonuna/RTK kurulumuna kanıt değildir. Ham hata/komut/header/body/kişisel yol/konuşma veya raw request ID listede, ayrıntıda ve serialize edilebilir veri projeksiyonunda bulunmaz.

## Uygulama ve sınırlar

- Observation/event'e varsayılan unknown request metadata eklenir; eski schema1 payload okunur. Süreler finite0..86.400.000ms, transform adedi0..128. Native typed sınır doğrulanır; raw geçersiz süreler bilinmiyor kalır. Metadata enrichment/absorbing conflict token ve attribution hesabından ayrıdır. SQL şeması/pin/retention/transaction sınırları değişmez.
- H024 dönem seçimi tek ortak yordam olur; Overview ve Activity aynı session/yerel gün/7 gün +30 gün retention semantiğini kullanır. Zamanı olmayan kayıtlar döneme olmuş sayılmaz; belirsiz kapsam sayısı görünürdür. Aynı origin'in katmanları tek istek kaydıdır; korelasyonsuz kaynaklar fiziksel çağrı adedi sayılmaz.
- Salt okunur list/detail native IPC; yalnız canonical UUID request, typed dönem/filter ve opaque origin kabul edilir. Filtreler istemci/unknown, model exact/unknown, hata bildirimi ve ölçüm durumudur. Filtre bütün seçilen kayıtlara uygulanır, sonra en yeni100 eşleşme döner; matched/limit/has_more açıkça gösterilir. Sonsuz liste/pagination eklenmez. Ayrıntıda en çok32 ölçüm ve omitted sayısı; bütün ölçümler ana toplama dahildir. Rastgele path/SQL/export yazımı yoktur.
- Liste/ayrıntı DTO'su yalnız allowlist metadata ve aynı ağırlıklı MetricSummary'dir. Tokenizer adı yerine opaque temel kimliği gösterilebilir; raw tokenizer metni dışarı taşınmaz. Model/istemci/ajan H026'nın kanıt sınırlarını korur. Farklı tokenizer/katman yüzdeleri toplanmaz; unknown yüzde0 olmaz.
- Activity kısa Graphite liste + filtre + klavyeyle açılan modal ayrıntıdır. Native istek korelasyonu ve son-query/son-selection denetimi; geç cevap yeni filtreye veya başka isteğe düşmez. Aynı-query hata son görüntüyü stale tutabilir. UI yerel sorguyu5 saniyede yeniler; motor/model başlatmaz. Dosya dışa aktarma H033'tür; bu görev onun için güvenli serialize edilebilir veri sınırını ve canary kontrolünü kurar.

## Kabul

1. Recent/log eşleme, süre0/eksik/negatif/büyük, bilinmeyen TTFT/cache, sabit hata enum'u, metadata replay/conflict, eski payload/reopen ve canary temizliği; modelden istemci/ajan çıkarımı yok.
2. Aynı dönem/filtre,100 sınırından önce doğru filtreleme, stabil son-zaman sırası/unknown zaman, opaque yanlış ID/not_found,32 ayrıntı sınırı; ölçüm ve JSON projeksiyonunda sır/içerik/ham log yok.
3. Gerçek owned collector→SQLite→native list/detail/read-only UI; dolu satırlar ayrı izole fixture kanıtı. Filtre/geç yanıt/klavye modal/locale/dar görünüm ve native zoom. Odaklı RED/GREEN, root tek birleşik review, son adayda tam yerel kapı ve güncel native kabul. H027 kapanınca durulur; H028 başlamaz.
