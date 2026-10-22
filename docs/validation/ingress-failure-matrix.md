# Kararlı giriş ve arıza davranışı

21 Ekim 2026. H040 Windows10 yerel kabul matrisi; standalone relay, gerçek pinned motor ve native UI kanıtları ayrı tutulur. [Mimari karar](../decisions/003-connection-ingress.md), [yerel kanıt](../proofs/connection-ingress.md).

## Rota ve auth sınırı

Rust giriş opak TCP taşır; HTTP isteğini, auth yöntemini, modeli veya reasoning alanını seçmez. Yalnız doğrulanmış ve uygulamanın sahip olduğu Headroom backend'ine bağlantı açar. Mevcut istemci bağlantı planı veya katalog kaydı bu görevle otomatik destek durumuna yükselmez.

| Yol | H040 kanıtı | Motor arızasında doğrudan upstream |
| --- | --- | --- |
| Yerel Responses / sentetik API-key | A2 byte dizisi, half-close, SSE/no replay; gerçek sağlayıcı yok | Kapalı |
| Yerel Anthropic Messages / sentetik API-key | A2 byte dizisi ve half-close; gerçek sağlayıcı yok | Kapalı |
| ChatGPT gateway / OAuth | Bu giriş üzerinden test edilmedi | Kapalı; API-key yoluna çevrilmez |
| Claude OAuth / yönetilen auth | Bu giriş üzerinden test edilmedi | Kapalı |
| WebSocket / HTTP2 / HTTPS-to-local | Bu giriş üzerinden test edilmedi | Kapalı |
| CCR geçmişi / eski nesilden retrieval | Restart sonrası süreklilik kanıtlanmadı | Kapalı; eski içeriğin yeni motorda var olduğu varsayılmaz |
| Ayrı sağlayıcı, süreç veya bulut yolu kullanan model/alt ajan | İlgili istemci rotası için ayrıca kanıt gerekir | Kapalı |

Burada “kapalı”, istemcinin konfigürasyonuna yeni bir fallback eklenmediğini ve Rust'ın ikinci upstream POST üretmediğini ifade eder. İstemcinin kendi retry davranışı ayrı doğrulamadır. Opak relay hatada HTTP yanıtı sentezlemez; bağlantıyı kapatır. Yerel testlerin geçmesi canlı model veya bütün alt ajanların doğrulandığı anlamına gelmez.

## Arıza ve yaşam döngüsü

| Durum | Beklenen davranış | Kanıt durumu |
| --- | --- | --- |
| Kayıtlı endpoint portu dolu | Aynı kayıt korunur; başka adres seçilmez, yabancı süreç sahiplenilmez/sonlandırılmaz | A1 ve manager conflict regresyonu PASS |
| Backend yok / stop başlatıldı | Yeni bağlantının payload'ı okunmadan kapanır | A2 ve gerçek manager no-route0response PASS; Windows reset geçerli kapanıştır |
| Backend PID uyuşmazlığı veya belirsiz peer tablosu | Payload aktarılmaz; yabancı child canlı kalır | A1/A2 PASS |
| Yeni motor nesli | Yeni TCP bağlantısı yeni nesle; açık bağlantı eski nesilde kalır | A2 pinleme PASS; gerçek pinned iki nesil ve native restart aynı endpoint PASS |
| Yarım SSE / backend kopması | Yarım akış biter; başka backend'e taşıma veya tekrar gönderme yok | A2 PASS |
| Geciktirilmiş okuyucu | Sınırlı buffer/partial-write offset; süreler içinde byte sırası korunur | A2 4MiB fixture PASS; gerçek uzun kernel backpressure ölçümü yok |
| Bağlantı sınırı / idle / toplam ömür / drain aşımı | Sınırlı kaynaklar; zorlanan kapanış graceful değildir | A2 PASS |
| Engine stop | Yeni route kapanır → engine request-aware stop → bounded TCP drain → owned cleanup | Gerçek manager+native idle keep-alive stop PASS; aktif pinned-engine gated-SSE stop H029/H030'da açık |
| Uygulama kapanışı | Owned motor ve ingress kapanışı doğrulanır; belirsiz cleanup başarı sayılmaz | Native iki exit0/süreç0/listener0; yeni app süreci aynı kaydı/adresi açtı |
| Eski observation / listener kaybı | Endpoint/engine gözlemleri ayrı; eski ready ve bilinmeyen sayaç başarı/sıfır sayılmaz | TTL/listener Start/poll ve startup sonrası heartbeat regresyonları PASS |

Varsayılan relay sınırları:32 TCP bağlantı, her yönde16KiB buffer,2s connect,300s idle,8saat bağlantı ömrü,2s cancellation grace. TCP bağlantı sayısı aktif HTTP isteği veya SSE stream sayısı değildir. Bu süreler bütün sağlayıcıların sessiz SSE profilleri için ölçülmüş değildir.

Son aday68 Rust testi/0 hata/2 opt-in ignored ve uyarısız embedded debug build ile geçti; pinned lifecycle opt-in ayrıca1 PASS. Fiziksel native click/keyboard ve1224ms idle-close gözlemi [native-proof.json](../../work/proofs/connection-ingress/native-proof.json), fingerprint'ler [final-quality.json](../../work/proofs/connection-ingress/final-quality.json) içindedir. Bu süre tek kontrol isteğinden itibaren gözlemdir; performans garantisi değildir. Windows11/temiz makine/installer ve canlı model/alt ajan kanıtlanmadı.

## Sonraki ürün kapıları

H040 engine restart boyunca sabit adresi hedefler. Uygulama tamamen kapanınca listener bırakılır; istemci hâlâ bu adrese bağlıysa portu başka bir süreç alabilir. H017/H029 bağlantı geri alma ve H030/H034 tray/background yaşam döngüsü bu sınırı çözmeden uygulama kapalıyken koruma iddiası yapılmaz. H011/H028 pause ve CCR sürekliliği de ayrı kapılardır.

Bu matriste doğrudan fail-open için onaylı yol yoktur. İleride eklenirse aynı auth/upstream/model/reasoning, gönderim öncesi kesin durum, tek POST, cache/CCR ve stream sınırları ayrı kabul gerektirir.
