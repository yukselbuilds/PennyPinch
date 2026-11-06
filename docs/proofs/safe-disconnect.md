# H029 — güvenli bağlantı kaldırma kabulü

6 Kasım2026. Yerel Windows10 x64 kabulü. [Plan017](../decisions/017-safe-disconnect.md), [inceleme](../reviews/H029.md).

## Gerçek davranış

Kaldırma, seçili istemci/kapsamın Headroom tarafından eklenen yönlendirmesini onaydan sonra geri alır. Mevcut `undo_connection` IPC → `PlanManager` → `FileTransaction`/ownership yolu kullanılır. Model/auth/reasoning ve kullanıcı ekleri korunur; sonradan değiştirilmiş endpoint silinmez. Yeni backend undo protokolü veya ikinci config yazıcısı eklenmedi. `integrations/disconnect.rs` yalnız Windows test modülüdür.

Kaldırmak motoru durdurmaz. Açık istemci güncellenmiş ayarı almak için yeniden açılabilir; açık sohbetin otomatik doğrudan bağlantıya taşındığı vaat edilmez. Motor Stop ayrı eylemdir: stable ingress yeni route'u kapatır, mevcut bağlantıyı kendi neslinde bırakır ve owned engine/relay temizliğini bekler.

## Kanıt matrisi

| Senaryo | Gözlenen | Kanıt / sınır |
| --- | --- | --- |
| Üç adaptörde sonradan kullanıcı düzenlemesi, manager yeniden açılışı, endpoint çatışması | Sahipli alan geri alındı; kullanıcı değişikliği ve başka dosya korundu; değiştirilmiş endpoint/config+active ownership byte'ları yazılmadı | İki yeni `disconnect_all_adapters_*` testi; son351 Rust suite'i gerçek izole dosya sistemini kullanır |
| Gerçek motor + gated SSE sırasında kaldırma | İlk delta istemciye ulaştıktan sonra Codex leaf undo; aynı generation/endpoint açık, diğer iki bağlı istemci dosyası aynı, tek POST terminal frame ile tamamlandı | `live-17.json`; gerçek pinned0.41.0/controller/helper/manager, loopback sağlayıcı fixture'ı |
| Aktif SSE → Stop → normal gate | Yeni stable-ingress isteği yanıt byte'ı almadan kapandı; eski SSE kapı açılmadan kesilmedi; terminal frame geldi; graceful stop ve owned port temizliği geçti | Aynı opt-in koşu. Normal gate2s içinde açılır; bu private engine portunun2s'de kapanacağı SLA değildir |
| Aktif SSE → Stop, gate kapalı | Terminal başarı yok; `cleanup_failed` ile eksik kapanış görünür; POST tekrar gönderilmedi; active TCP sayısı0; owned backend portu kapalı; ayrı listener canlı; manager shutdown temiz | Aynı koşu, mevcut10s uvicorn request-drain/20s process/30s controller/90s actor bütçeleri değiştirilmedi; stop için önceden belirlenen60s üst sınırı |
| Gerçek native kaldırma | Ayrı auth'suz Codex test profiline native preview/apply; gerçek yeni onay UI'sından ESC/iptal ve onay/undo; owner/revision released; motor generation aynı; ayrı explicit Stop ack | Native run `54557cfb-0979-4bac-bb62-44e0b6a33f34`; global kullanıcı config'i ve app ayarları önce/sonra aynı, yalnız teste ait metadata CAS ile eski byte'larına döndü |
| Bekleyen/hatalı UI, erişilebilirlik | İkinci undo ve kapanış engeli; pending iyimser başarı değil; conflict/commit_unknown sonrası yeniden ownership okuma; odak geri dönüşü ve Tab döngüsü; TR/EN/açık/koyu/819–683 genişlik ve gerçek WebView125/150/100 ölçek | Aynı native run'daki izole bileşen fixture'ı; gerçek native IPC'ye erişmeyen bundle guard'ı vardır; mock sağlayıcı/hesap kanıtı değildir |
| Tokenizer başlangıcı | Altı public vocab SHA256 ile doğrulandı; yedi tiktoken kodlaması socket bağlantıları yasakken gerçekten yüklendi | [Hazırlık](../development/tokenizers.md); `work/proofs/tokenizers/prepare-95e27441-9279-49bc-bb90-c3d5e8c58008.json` |

Native pencere exit0 verdi; debug listener kapandı; artifact ve korunan kullanıcı/app ayarı fingerprint'leri koşu içinde değişmedi. Native onay görünümü ve açık/koyu izole render ana ajan tarafından incelendi. Son native onay capture'ı koyu/boş render ürettiğinden tek başına görsel kabul sayılmaz; aynı görünümün önceki gerçek native `9c8497df-f736-4a42-97ce-95b379aecf01` capture'ı ve son native run'ın izole render'ları kullanıldı. Fonksiyonel native ack/odak kontrolleri son artifact üzerinde geçti.

## Bulunan iki motor sınırı

İlk soğuk token sayımı, eksik vocab yüklemelerinde iki10s bekleme yapabildi. İzole tanıda20,080s görüldü; cache hazırlandıktan sonraki aynı taşıma tanısında ilk istek0,381s, sonraki üç istek0,032–0,046s idi. Bunlar tekil tanı gözlemleridir, benchmark veya gecikme garantisi değildir. Çözüm sağlık timeout'unu artırmadı: helper her engine açılışından önce cache verisini doğrular ve yalnız engine sürecinin `TIKTOKEN_CACHE_DIR` değerini bağlar. Git'e veri/binary girmez; installer içine alınması H038/H041'de ayrıca doğrulanır.

Pinned uvicorn,10s sonunda kalan yanıtı iptal edip process exit0 üretebiliyor. Adapter artık `timeout graceful shutdown exceeded` sabit işaretini içeriksiz flag olarak tanır; bu durumu graceful diye iletmez. Stop DTO/şeması değişmedi. Üç yeni Python shutdown/cache testi ve mevcut controller protokol testleri `units-2.json` ile geçti.

Erken fixture koşularındaki canonical Windows yol ve HEAD prewarm varsayımları düzeltildi. Kabul edilen TCP soketine açıkça blocking mod verilmemesi erken kapanma/502 ve startup beklemeleri üretebiliyordu; son fixture bu modu kurar ve yarım/boş prewarm girişinde döngüye girmez. Eşzamanlı kontrol yükü ilk şüpheydi; tek süreçte de502 görüldüğü için neden olarak kabul edilmedi. Son17 koşusu bu düzeltilmiş fixture'ı kullanır. Geçici actor/HTTP teşhis kodu ürün adayından kaldırıldı.

## Son kalite ve yeniden kullanım

`build-2.json` TypeScript/Vite ve embedded native build; `final-1.json`138 frontend testi; `native-final-2.json` fmt ve351 Rust testi/7 opt-in ignored; `live-17.json` ayrı gerçek SSE opt-in PASS. Bunlar `work/proofs/disconnect/` içindedir.

Son incelemede yalnız `cfg(all(test, windows))` altındaki `integrations/disconnect.rs` fixture'ı değişti. Güncel native suite bu dosyayı derleyip çalıştırır. Node/build/Python sonuçları, değişmeyen bütün ilgili ürün/TS/runtime/script girdileri ve aynı araç/ortamla yeniden kullanılır; tam156 girdili haritaların tek farkı programatik kontrol edilir. Artifact, asset ve doküman bağlaması `acceptance.json` içindedir; başarılı engine/UI testleri nedensiz tekrarlanmaz.

## Kabul sınırı

Gerçek sağlayıcı/model çağrısı ve yeni hesap girişi yapılmadı. SSE testi gerçek motoru, izole yerel sağlayıcıya bağlar; canlı Codex/Claude/OpenCode hesabı, bütün modeller veya alt ajan desteği iddiası değildir. Windows11/macOS/Linux, temiz makine/installer, uzun yük/kararlılık ve API-key H041 kapısı açık kalır. Geliştirme token tasarrufu ölçülmedi; yüzde üretilmez. H029 kapanınca H030'a geçilmez.
