# H010 kabulü

15 Kasım 2026. H010, Windows 10 22H2 x64 üzerindeki aşağıdaki teknik kanıt kapsamıyla tamamlanır. Motor 0.41.0, sağlayıcı yerel sentetik fixture'dır. [Programatik kapanış](opencode-acceptance.json), [ana ajan incelemesi](../reviews/H010.md), [bağlantı kararı](../decisions/009-opencode-provider-routes.md).

| Ana kabul | Kanıt ve kapsam |
| --- | --- |
| Sağlayıcı ve model seçimi korunur | 1.19.3 CLI/TUI'de aynı ana oturumda model değişimi, yeni alt ajanın mirası ve farklı provider/model çifti geçti. 2.3.0 CLI'de model mirası ve açık alt model seçimi geçti. [V1](opencode-subagents.md), [V2](opencode-v2-subagents.md) |
| JSON/JSONC ve auth kapsamı kayıtlıdır | Native Önizle/Uygula, yorumlar ve kullanıcı alanları, yeniden açılış, çatışmalı Kaldır ve özgün baytlara dönüş doğrulandı. Sentetik auth wire üzerinde eşleşti. Gerçek OAuth/hesap ayrı. [V1](opencode-retrieval.md), [V2](opencode-v2-binding.md) |
| Streaming, araç ve CCR korunur | Chat streaming ve ayrı Responses rotaları, doğru call_id, tamamlanmış araçlar, logun sıkıştırılması ve özgün içeriğin birebir geri çağrılması geçti. [V1](opencode-retrieval.md), [V2](opencode-v2-binding.md) |
| Ana/alt ajan ve model değişimi izlenir | Araç metadata'sı gerçek ayrı oturumla eşleşti. V1'in tam TUI'sinde model menüsü ve aynı parent session'da değişim geçti. V2 arka plan alt ajanının tamamlanma bildirimi ayrıca doğrulandı. [TUI](opencode-tui.md), [V2 alt ajan](opencode-v2-subagents.md) |
| Normal servis ve masaüstü sınırı bellidir | 2.3.0 CLI ve tam TUI aynı kayıtlı serviste geçti. Resmî paketten çıkarılan Desktop, CLI ile kaydedilmiş config'i kendi paketli servisi üzerinden kullandı. GUI finali ve normal çıkış ayrı doğrulandı. [Servis/TUI](opencode-v2-service.md), [Desktop](opencode-v2-desktop.md) |
| Hata, sahiplik ve kapsam korunur | İptal/çatışma, bridge yokluğu, duraklatma, yeniden açma, yanlış sürüm ve normal/başarısız koşu temizliği kayıtlı. Son katalog, gerçek yüzey/sürüm/OS/auth eşleşmesiyle sınırlı. [Keşif](opencode-v2-discovery.md), [geri alma](opencode-retrieval.md) |

K2, erişilemeyen yüzey/sürüm/auth birleşimlerinin deneysel kalmasını öngörür. Karar 009 başlangıç profillerini standart OpenAI Chat/Responses yollarıyla sınırlar. Bu nedenle aşağıdaki doğrulanmamış birleşimler destek kaydında açık kalır. Önceki “H010 kalan kabulü” satırının bütün yeni birleşimleri zorunlu kapanış adımına dönüştürmesi düzeltilir.

- Canlı sağlayıcı/auth, özel reasoning varyantları ve gerçek yanıt kalitesi.
- Yalnız Desktop kurulu ortamda otomatik keşif ve kurucu işlemleri.
- V2 TUI/Desktop içinden alt ajan, aynı alt oturumda model değişimi ve nested alt ajan.
- Çalışan servisin ayarı anında benimsemesi, diğer provider/özel upstream ve ek işletim sistemi birleşimleri.

Bu kapanışta ürün veya katalog davranışı değişmedi. Son yerel kapının 344 girdi ve araç/OS fingerprint'i eşleşirse 222 Node/462 Rust, 19 ignored, fmt/frontend/embedded build sonucu yeniden kullanılır. Her native kayıt kendi tarihsel executable ve kaynak fingerprint'ini korur. Güncel ürün adayının regresyon kanıtı son yerel kapıya aittir. Bu inceleme canlı trafik veya yeni işletim sistemi testi başlatmaz.

Desktop testindeki bir Uygula hatasının nedeni bilinmiyor. Aynı eylem son adayda geçti. Başarısız koşuda özgün config ve test ayarı geri döndü. [Koşu sınırları](opencode-v2-desktop.md)

Log bloklarındaki bayt azalması ile gerçek token/fatura tasarrufu ayrı ölçümlerdir. H010 gerçek token/fatura sonucu üretmedi. Mevcut yerel token, cache, bütünlük ve gecikme kıyasları H041'in kendi senaryo ve ölçüm temeliyle kayıtlıdır. [Optimizasyon ölçümleri](../validation/optimization-benefit.md)
