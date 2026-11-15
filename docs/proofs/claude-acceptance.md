# H009 — Claude Code yerel teknik kabulü

15 Kasım 2026. **H009 tamamlandı.** Kapsam Windows 10 22H2 x64, Claude Code 2.1.158, Headroom 0.41.0 ve sentetik Anthropic API yoludur. H009'un bağlantı/koruma/streaming/araç/retrieval kabulü mevcut kanıtlarla karşılaştırıldı. TASKS K2, erişilemeyen yüzey/sürüm/auth birleşimlerini deneysel tutar. Hesap olmadığı durumda sınır kaydı görevde açıkça izinlidir.

| Kabul | Gözlenen davranış | Kanıt |
| --- | --- | --- |
| Etkin ayar kapsamı ve bağlantı | Gerçek native UI'de kullanıcı ve açıkça seçilmiş proje kapsamı, Önizle/Uygula, sahipli sabit ingress ve gerçek CLI isteği doğrulandı. Proje ayarı CLI tarafından ek model/endpoint/auth/efor bayrağı olmadan yüklendi. | [H020](claude-product.md) |
| Mevcut ayarları koruma | Sentetik auth, seçili model/efor, hooks ve ilgisiz üç ayar dosyası korundu. Kullanıcı hesabı veya secret deposu kopyalanmadı. Gerçek OAuth hesabının korunması bu sentetik deneyden çıkarılmaz. | [H020](claude-product.md) |
| Streaming ve final | Gerçek partial text, terminal olayından önce CLI'de görüldü; final başarı ve normal CLI exit 0 doğrulandı. | [H009-a](claude.md), [native H020](claude-product.md) |
| Araç ve geri çağırma | İzinli tek MCP fixture aracına iki gerçek CLI çağrısı; üç giriş isteği/dört upstream mesajı; call_id/model/efor/thinking/sentetik auth, final ve graceful motor kapanışı geçti. 14.999 bayt log 103 bayt oldu; server-side CCR özgün baytları ve hash'i geri getirdi. | [H009-b JSON](claude-tools.json), [inceleme](../reviews/H009-b.md) |
| İptal, çatışma ve geri alma | Native seçici iptali kapsamı korudu; Apply/Undo üç özgün ayar dosyasını byte-exact döndürdü. H022'de yanlış proje Undo'su asıl dosyaları korudu; doğru projede kullanıcı ekleri kaldı. | [H020](claude-product.md), [H022](onboarding.md) |
| İlk kullanım ve yeniden açılış | Salt okunur keşif/önizleme dosya oluşturmadı. Açık Bağlan eylemi eksik ebeveyni transaction içinde oluşturdu; normal yeniden açılış doğru proje/kurulum seçimini hatırladı. H020'nin eski eksik-ebeveyn/bellek-içi seçim sınırı aşıldı. | [H022](onboarding.md) |
| Model ve alt ajan sınırı | Bağlantı yalnız endpoint alanını yönetir; model allowlist'i, otomatik model/efor seçimi veya ayrı ajan kurulumu eklenmez. Claude alt ajanı, farklı role özgü endpoint/provider ve model değişimi gerçek trafikle doğrulanmadı; tam ajan kapsamı iddia edilmez. | [Karar 008](../decisions/008-claude-connection.md), [katalog](../../compatibility/clients.json) |

Native UI akışındaki CLI deneyi araçsızdır. H009-b araç/CCR deneyi ayrı izole CLI/MCP profili ve test gözlemcisi kullanır; Tauri/Rust ingress üzerinden araçlı uçtan uca ürün kabulü sayılmaz. Fixture'ın `protected_tools=[]` ayarı ürünün korumalı araç varsayılanını değiştirmez. Server-side CCR, CLI'nin kendi başlattığı retrieval desteği değildir. Büyük MCP çıktısının istemci tarafından dosyaya taşınması ayrıca gözlendi; buna Headroom kazancı atanmaz.

Canlı Anthropic/API-key/OAuth, gerçek model özellikleri ve yanıt kalitesi, genel hooks/plugins/MCP, Claude Desktop/eklenti, alt ajanlar, farklı rota sağlayıcıları, Windows 11 ve diğer OS birleşimleri `not_tested` kalır. Kullanıcı Claude girişini daha önce erteledi; yeni giriş veya ücretli çağrı yapılmadı. Tek bloktaki bayt azalması token veya fatura tasarrufu değildir. Geliştirme sohbetinin tasarrufu ölçülmedi.

Bu dilimin planı mevcut kanıtı ana kabul ve K2 ile uzlaştırmak, katalogdaki eski tools-off ve onboarding sınırlarını düzeltmek, tarihsel artefaktları kendi fingerprint'leriyle korumaktı. Auth/rota/süreç kabulünü ana ajan üstlendi. Model, auth, süreç/ayar sahipliği ve atomik geri alma sözleşmeleri değişmedi. Uygulama kapsamı uyumluluk kataloğuyla sınırlıdır. Yeni motor, adapter veya UI davranışı eklenmedi.

Katalog gömülü ürün girdisi olduğu için son adayda WORKFLOW'un beş komutlu yerel kapısı bir kez yenilendi. 222 Node ve 462 Rust testi geçti, 19 Rust testi ignored; fmt, frontend ve embedded custom-protocol derlemesi geçti. Son kapının 344 kaynak/test/config/araç girdisi, ortam ve komutları önce/sonra bağlandı. H020/H022'nin eski exe ve kaynak snapshot'ları güncel binary diye sunulmaz. H009-b'nin değişmeyen kaynakları ve tutulan native/focused/onboarding sonuçları ayrıca hash ile kontrol edildi.

Programatik [ana kabul](claude-acceptance.json), doğrulama komutu ve önce/sonra kayıtları `work/proofs/claude-acceptance/` içindedir. [Ana inceleme](../reviews/H009.md). Bu kabul H006/H037/H041/H042 dış ortam/yayın kapılarını kapatmaz. **Toplam 45/54.** Ana görev sonunda durulur.
