# 004 — Salt okunur bağlantı önizlemesi

21 Ekim 2026. H017 Windows10 yerel önizleme kabulü tamamlandı. [Kanıt](../proofs/connection-plan.md), [inceleme](../reviews/H017.md). Gerçek config yazımı ve istemci bağlantısı ayrı kapılardır.

## Kullanıcının gördüğü kapsam

Bulunan bir kurulum bağlantı yetkisi değildir. Kullanıcı belirli bir kurulumu ve açıkça adlandırılmış kapsamı seçerek plan ister. İlk önizleme kapsamı kullanıcı config'idir; proje, komut satırı, shell, yönetilen ayarlar ve açık oturumun etkin ayarları ayrıca doğrulanmadıkça bilinmiyor gösterilir. Proje veya açık sohbet veritabanı taranmaz/değiştirilmez. Bu önizleme ücretli model isteği göndermez.

Plan yalnız gerekli endpoint alanını önerir; model, reasoning, provider kimliği, giriş yöntemi ve ajan sayısı değiştirilmez. Config'teki bildirim ile çalışma anındaki etkin ayar ayrı değerlendirilir. Ana/alt ajanlar ancak aynı doğrulanmış rotayı kullanırlarsa kapsam hedefindedir; katalogdaki bilinmeyen durumlar değişmez. Hook/shell varsa içeriği çalıştırılmaz veya gösterilmez; etkisi ve belirsizlik sabit durumlarla özetlenir.

H017 config yazmaz ve bağlantıyı doğrulandı saymaz. H018 sahiplik/atomik yazım/geri alma, H019–H021 gerçek istemci rotası kapılarıdır. Önizlemede uygulanmayan bir eylem başarı döndürmez.

## Güven sınırı

Renderer yalnız opak kurulum kimliği, beklenen tarama nesli ve izinli kapsamı verir. Keyfi dosya, port, URL, shell veya auth değeri kabul edilmez. Kurulum backend keşif kaydından; config konumu backend tarafından izinli kullanıcı köklerinden türetilir. Dosya okuması boyut, şema, sabit disk ve reparse sınırlarıyla kısıtlanır. Belirsiz/bozuk veya değişen belge plan başarısına çevrilmez.

TOML hazır parser ile, JSON/JSONC string/comment/escape/trailing-comma kurallarını bilen parser ile çözülür. Duplicate alanlar belirsiz kabul edilir. Parser hatası, serbest metin veya config içeriği UI/log'a kopyalanmaz. Auth dosyaları ve sohbet DB'si okunmaz; config'te bulunabilen sırlar bellekteki kısa okuma sınırının dışına çıkmaz. Önizleme yalnız sabit etiketleri ve gerekli sırsız metadata'yı taşır; özgün URL/header/komut/yol literal'leri taşınmaz.

Kalıcı hedef yalnız H040 runtime yöneticisinin taze, sahip olduğu ingress kaydından gelir. Motorun geçici portu veya UI'dan alınan hedef kullanılmaz. Endpoint hazır değilse önizleme bunu açıkça bildirir. Uygulama bütünüyle kapandığında bağlantı noktası serbest kalır; bu önizleme arka plan/tray veya bağlantı kaldırma davranışının kanıtı değildir.

Süreli plan kimliği; config fingerprint'i, kurulum/tarama nesli, seçilen kapsam ve kalıcı endpoint kimliği/adresiyle ilişkilendirilir. H018 uygulanırken yeniden okuma ve fingerprint kontrolü gerekecek; H017 sonucunun varlığı yazım yetkisi değildir.

## Kaynak ve destek sınırı

- [Codex configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference): built-in provider için `openai_base_url`, custom provider için `model_providers.<id>.base_url` tanımlıdır. Provider/auth anahtarları proje config'inde etkili sayılmaz. Güncel belge kurulu binary'nin aynı davranışına kanıt değildir; [H008](../proofs/codex.md) yalnız kaydedilen CLI fixture kapsamını taşır.
- [Claude settings ve precedence](https://code.claude.com/docs/en/settings): kullanıcı, proje, local ve managed katmanlar ile komut satırı/ortam etkisi ayrıdır. [H009](../proofs/claude.md) API fixture kapsamı OAuth veya bütün oturumların kanıtı değildir.
- [OpenCode config](https://docs.opencode.ai/docs/config/) ve [provider ayarları](https://opencode.ai/docs/providers/): JSON/JSONC katmanları birleşir; V1 `provider.<id>.options.baseURL` şeması V2'nin çoğul provider şemasıyla karıştırılmaz. [H010](../proofs/opencode.md) pinned V1 tools-off fixture sınırıdır.

Bu kaynaklar 21 Ekim 2026'da ilgili bölümleriyle okundu. Bilinmeyen sürüm, custom gateway, auth, transport ve ayrı alt ajan rotaları açık kalır. Son adayın üç format için gerçek native önizleme yolu geçici config kökleriyle doğrulandı; bu sonuç kurulu istemcilerin canlı uyumluluk durumunu yükseltmez.
