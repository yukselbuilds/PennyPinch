# H023 — İçeriksiz sayaç normalizasyonu

3 Kasım 2026. **Kabul edildi; ürün sayımı20/51.** [Karar010](../decisions/010-metrics-normalization.md), [sözleşme](../contracts.md), [inceleme](../reviews/H023.md). Uygulamayı yeni AGENTS kapsamında ana ajan yaptı: token ölçüm semantiği ve köken kararları root'a aittir. Luna'ya iş verilmedi; aynı alanda ikinci yazan yoktu.

## Kabul edilen davranış

| Senaryo | Sonuç | Kanıt |
| --- | --- | --- |
| Açık aynı temelli120.000→75.000 |45.000 token, %37,5; yalnız aritmetik fixture, canlı tasarruf değil | `metrics_comparable_reduction_zero_baseline_and_increase` |
| Gerçek sıfır / sıfır taban / eksik / artış | %0, null yüzde, null miktar ve negatif fark ayrı korunur | Aynı test; boş batch testi |
| Farklı veya bilinmeyen temel/bileşen | Ana miktar/yüzde türetilmez; eski gözlem sayıları kaybolmaz | Basis/component ve legacy regression testleri |
| H021 gerçek satırlarının sayı biçimi |2306/12/saved0 ve12/12/saved0 satırları bilinmeyen temelde; sahte yüzde yok | Git fixture + `review-boundaries.json`; sayı dizileri H021 `traffic.json` ile programatik eşleştirildi |
| İçeriksiz projeksiyon | Request/response içeriği, auth, raw ID, serbest tags, sahte basis/köken, lifetime/saved/% çıkışa taşınmaz | Canary fixture, raw-origin ve limit testleri |
| Kanıtlı proxy/MCP kökeni ve tekrar | Aynı ölçüm bir event; eşlenmeyen MCP ana toplam adayı değil | `metrics_proxy_mcp_replay_deduplicates_only_proven_origin` |
| Çelişen tekrar | Miktar/yüzde ve belirsiz zaman/temel unknown olur; çatışma görünür | İlk red regresyonu → düzeltme → PASS |
| Runtime restart / UTC eşdeğeri | Aynı ID farklı nesilde ayrı; aynı UTC anının farklı offset yazımı birleşir | Restart ve equivalent UTC testleri |
| Katman ve provider sayımları | RTK/schema/novel/cache/output ana girdiye eklenmez; provider usage ayrı | Katman testleri ve novel input kaynak incelemesi; aynı kökende üç ölçümden yalnız bir main-eligible |
| Aggregate / kısmi pencere / kimliksiz satır | Aggregate event yapılmaz; eksik kimlik sayılır, snapshot tam trafik sayılmaz | Partial-window testi; `complete_traffic=false` |
| Sınır / hatalı metadata |2MiB,10.000 gözlem, JSON-güvenli tamsayı, UTC/UUID doğrulaması; sabit hata | Limit/type/identity testleri |

Rust veriyolu `bounded JSON → allowlist metadata → typed Observation → normalize → schema1 event/batch` olarak doğrulandı. `recent_requests` tercih edilir; yoksa `request_logs`. Aynı iki liste birden sayılmaz. Kaynakta tokenizer/bileşen provenance'ı bulunmayan0.41.0 gözlemlerinden karşılaştırılabilirlik çıkarılmaz; kanıtlı yeni toplayıcı bu API'ye açık temel sağlayabilir. Generic ölçüm API'si ham engine/renderer claims kabul eden bir Tauri komutu değildir.

## Doğrulama ve fingerprint

`work/proofs/metrics-normalization/` altında:

- `entry.json`: kanonik kök, uygulayıcı kararı, başlangıç kaynak/config hash'leri; kaynak notundaki altı pinned engine dosyası eşleşti.
- `red.json`: ilk8 test uygulanmamış davranışta beklenen şekilde başarısız. `focused-first.json`:8 PASS.
- `review-red.json`: üç çelişkili kaydın zaman/temel hatası yeniden üretildi. `focused-review.json`: düzeltme ve genişleyen12 test PASS; kaynak önce/sonra aynı.
- `review-boundaries.json`: mevcut H022 kaynaklarından yalnız `main.rs` modül satırı ve Cargo'nun mevcut time parsing özelliği değişti; Cargo.lock aynı. Diğer H022 kaynakları korundu.
- `frontend-reuse.json`: mevcut32 frontend kaynak/config ve3 dist, Node25.2.1/pnpm11.25.0/Windows10 fingerprint'leri H022 kanıtıyla aynı. **97 frontend PASS ve build kanıtı yeniden kullanıldı; pnpm test/build tekrarlanmadı.**
- `final-quality.json`: final adayda bir kez Cargo fmt, tüm Rust testleri **285 PASS/3 ignored**, offline custom-protocol build PASS. Rust/Cargo1.92.0; kaynak, araç ve ortam önce/sonra eşleşti.81 kaynak/fixture/config ve binary hash'i kaydedildi. Önceden var olan6 build warning'i,0 hata. QA sonrası hash kontrolünde aynı aday korundu.
- `acceptance.json`: kapsam, kanıt hash'leri ve sınırlamalar. CodeGraph proje launcher sync başarılı, zaten güncel; graf sonucu runtime/test kanıtı sayılmadı.

Başarılı testler compact, yeni gün veya belge kapanışı nedeniyle yinelenmedi. Yalnız yeni test/kod değişikliği ilgili odaklı kanıtı yeniledi. Ham başarısızlık dökümü, HTTP içerikleri veya auth bilgisi saklanmadı.

## Sınır ve sonraki bağ

Bu saf hesaplama/adaptör kabulüdür. H023 uygulamada polling, kalıcı kayıt veya gerçek dashboard açmadı; UI değişmediği için masaüstü akışı tekrar edilmedi. H024 kayıt/upsert, dönem ve gerçek gap takibini; H025 ekranı bağlayacak. Canlı karşılaştırılabilir token toplayıcısı veya motorun0.41.0 satırlarının temeli kanıtlanmış sayılmaz. Yeni toplayıcı eklenirse sayım yöntemi/tokenizer ve bileşen provenance'ı ayrıca gösterilmeli; eski sayılar otomatik yükseltilmez.

Model/auth/rota/kullanıcı ayarı veya canlı süreç değiştirilmedi. Model/istemci/alt ajan desteği genişletilmedi. Provider/model çağrısı veya ücretli kaynak kullanılmadı. Windows10 embedded derlemesi diğer OS/installer veya uzun süreli performans kanıtı değildir. Geliştirme token tasarrufu ölçülmedi. H008 Codex sürüm engeli, H012'nin dış kabiliyetleri ve H019 amber sınırı açık kalır.
