# H027 — içeriksiz etkinlik kabulü

5 Kasım 2026. Karar [015](../decisions/015-activity.md); ana ajan incelemesi [H027](../reviews/H027.md). H033 dosya/CSV dışa aktarma bu görevin dışındadır; serialize edilebilir liste/ayrıntı projeksiyonu içerik canary'leriyle sınandı.

| Senaryo | Beklenen ve gözlenen | Kanıt |
| --- | --- | --- |
| Kaynak metadata | İki liste bir kere sayılır; eşleşmeyen kimlik/zaman/model tamamlanmaz.0 ve bounded süre korunur, eksik/negatif/aşırı süre ve ilk-token unknown kalır; hata/transform/içerik atılır. RED2 davranış hatası, ardından GREEN. | `work/proofs/activity/metadata-red.json`, `metadata-green-2.json`; son aday Rust testleri |
| Kalıcılık ve sorgu | Replay/enrichment/absorbing metadata conflict tokenları değiştirmez. Eski schema1 payload/reopen okunur; session/yerel dönem/retention Overview ile aynıdır. Filtre100 sınırından önce, bilinmeyen zaman sonda. Ayrıntı32 + omitted; toplam bütün event'lerdendir. | `metrics-green-2.json`; son aday `activity_tests` |
| Hassas alan ve client | Model/personal path, unsafe ID, yanlış korelasyon/dönem, bozuk sayaç/yüzde reddedilir. Export-safe DTO bilinmeyen secret/body/agent/tokenizer alanlarını atar. Geç liste/ayrıntı yanıtı yeni kapsama düşmez. | `node-focused-1.json`; son aday133 frontend PASS |
| Gerçek owned HTTP zinciri | Metadata fixture'ı loopback peer/nesil sınırından geçip SQLite → manager list/detail'e ulaşır; replay bir event, ham içerik/secret yok. Legacy token temeli unknown kalır. | Son aday `owned_http_metadata_reaches_sqlite_without_payload_or_replay_inflation` |
| Yerel ürün kapısı | `pnpm test:app`, `pnpm build`, `cargo fmt --check`, offline Rust test ve custom-protocol build;133 frontend/344 Rust PASS,5 mevcut opt-in ignored. | `work/proofs/activity/final-1.json` |
| Gerçek masaüstü | Native strict client üç dönemi ortak Overview sayılarıyla doğruladı; not-found/invalid-input sabit hata ve own pinned collector recording geçti. Gerçek kaynak0 istek, model çağrısı0. | `ui-runs/0c6068a4-27ad-479d-85bf-c1202b7f912c/result.json` |
| Dolu UI fixture | Gerçek Activity hook/client + izole core mock: filtre/exact validation,0/unknown/negatif, geç ayrıntı, stale/recovery,metadata conflict,100 satır sınırı, empty/error/unavailable,Enter/Space/Escape/focus. TR/EN,dark/light ve1280/1024/819/683 görünüm taşmadı; native125/150/100 zoom geçti. | Aynı native run, `synthetic-*.png`; gerçek boş ekran `native-activity.png` |

`work/proofs/activity/acceptance.json` son kapının142 kaynak/config ve araç/ortam fingerprint'lerini, güncel EXE/dist/fixture/runner + pinned models/server/outcome kaynaklarını programatik karşılaştırır. Before/after ve güncel aday eşleşmeden PASS üretmez. Pencere exit0 ile kapandı; sahipli debug portu kapandı, mevcut Codex config ve uygulama preferences/onboarding/ownership değişmedi. Ham hata/provider/HTTP dökümü tutulmadı; başarısız testte sabit sınıf/işaretler ve yalnız teste ait ekran görüntüsü kaydedildi.

Bu Windows10 22H2/build19045 yerel geliştirme kabulüdür. Windows11/macOS/Linux/installer/uzun yük veya canlı sağlayıcı tasarrufu kanıtı değildir. TTFT, destekleyen kaynak olmadan unknown; hata alanının null olması başarı, cache flag'i indirim değildir. Gerçek kullanıcı/geliştirme tasarrufu yüzdesi ölçülmedi. H028/H033 başlamadı; H041 canlı API-key kabulü açık kalır.
