# OpenCode V2 kullanıcı bağlantısı

15 Kasım 2026. Headroom, Windows 10 22H2 x64’te sabit OpenCode 2.3.0 CLI için mevcut sağlayıcı uç noktalarını Önizle ve Uygula üzerinden bağladı. Plan, model, efor, kimlik doğrulama ve izin ayarlarının mevcut değerlerini korudu. Test edilen kurulum resmî Windows x64 baseline paketidir. [Karar](../decisions/009-opencode-provider-routes.md#h010-g--v2-kullanıcı-bağlantısı-ve-şemalı-geri-alma)

| Senaryo | Gözlem |
| --- | --- |
| Gerçek Headroom penceresinde Önizle ve Uygula | İki sağlayıcı uç noktası ve MCP köprüsü aynı JSONC dosyasına kaydedildi. Ortak sahiplik kaydı V2 şemasını taşıdı. |
| Chat Completions ve Responses | Her protokolde dört yerel model isteği, üç tamamlanan araç, doğru model ve sahte API anahtarı, beklenen son yanıt ve CLI exit 0. |
| Sıkıştırılmış logu geri çağırma | 14.999 baytlık örnek blok iki protokolde de 103 bayta indi. `headroom_retrieve` özgün baytları ve hash’i doğruladı. |
| Headroom yeniden açıldıktan sonra Kaldır | MCP tanımına yapılan düzenleme çatışma üretti. Ayar dosyası ve sahiplik kaydı aynı kaldı. Test düzenlemesi silinince Kaldır özgün JSONC baytlarını döndürdü ve sahipliği bıraktı. |
| Kapanış | İki Headroom penceresi normal kapandı. Sahipli süreçler ve bağlantı portları serbest kaldı. |

Gerçek OpenCode binary’si kendi `--standalone` servisini başlattı ve Headroom’un kaydettiği V2 ayar dosyasını okudu. Yerel upstream sabit sentetik veri döndürdü. Bayt karşılaştırması örnek log bloğuna aittir. Canlı kullanımda token, fatura ve geliştirme sohbeti tasarrufu ölçülmedi.

Pencere ve model kanıtı `work/proofs/opencode-v2-binding/native-d2a0ee959b/result.json` içindedir. Test penceresi ayrı uygulama kimliği ve profil kullandı. Arayüz düğmelerine yalnız bu pencereye ait WebView üzerinden erişildi. Gerçek OpenCode TUI, Desktop, paylaşılan servis, alt ajan, canlı hesap ve diğer işletim sistemleri H010 kapsamında açık.

Son ürün kapısında 222 Node ve 462 Rust testi geçti. 19 opt-in Rust testi normal koşuda atlandı. Biçim kontrolü, ön yüz ve gömülü uygulama derlemeleri başarılı. V2 gerçek dosya testi ayrıca çalıştırıldı ve köprü bulunmadığında yazımın reddedildiğini doğruladı. Native koşuda 22.685, son kapıda 344 girdi eşleşti. Native koşu sonrasında uyumluluk kataloğuna Windows 10 CLI kanıtı eklendi. Bağlantı kaynakları, testler ve WebView dosyaları aynı hash'leri taşıyor. Son yerel derleme yeni katalog kaydını içeriyor. [JSON kanıtı](opencode-v2-binding.json)

İlk pencere koşusu dar GUI ortamıyla açılmadı. Sonraki koşular Windows ortam girdileri ve aynı diskteki izole ayar dosyasıyla açıldı. Test yardımcıları paket yolu için dosya kimliğini ve hash'i karşılaştıracak biçimde düzeltildi. Responses doğrulamasında upstream akış biçimi ve beklenen araç dizisi, gözlenen V2 çıktısına göre düzeltildi. İki protokolün `headroom_retrieve` tamamlanması yalnız test profiline ait SQLite kayıtlarından da doğrulandı. Başarısız koşular ve bu tanı sonucu JSON kanıtına bağlıdır.
