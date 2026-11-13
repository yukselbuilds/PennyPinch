# macOS Codex Desktop kanıt kapsamı

H048-c7b, macOS 15.8.2 ARM64 üzerinde resmî Desktop 26.1106.51428 ve bundled CLI 0.167.0-alpha.4.1 ile geçti. Gerçek GUI isteği, paketli Headroom 0.41.0 ve yerel WebSocket sağlayıcısında koşuya özel nonce ile eşleşti. Terminal öncesi kısmi ve tam yanıt render edildi. Endpoint, provider, model/efor ve sentetik auth korundu; Desktop dışındaki TOML alanları aynı kaldı. Gerçek collector aynı modeli kaydetti. Normal çıkışta gözlenen çocuklar, motor, port ve kilitler bırakıldı; paket ve ortam fingerprint'leri eşleşti.

[Makine kanıtı](macos-desktop-traffic.json) · [Native koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/43284385567) · [Kabul ve tanı sınırları](../validation/macos.md#h048-c7b-kontrollü-gui-akışı-geçti).

Bu tarihsel kayıt `local_fixture_partial` düzeyindedir. Testte `gpt-5.2-codex`/`medium` kullanıldı; bu bir ürün model listesi değildir. Canlı hesap/sağlayıcı, diğer modeller/gerçek alt ajan kimliği, diğer macOS sürümleri ve Intel doğrulanmadı. Tam uygulama state yalıtımı da kurulmuş sayılmaz. Tasarruf veya performans benchmark'ı değildir.

H048-c7'de ürün bağlantı planı ve yeni yöneticide geri alma kabulü tamamlandı. Config desteği yalnız kanıtlanan uygulama sürümü için açıldı. Native 15 Rust testi, frontend ve ARM64 derlemesi geçti; Apply/Undo ilgisiz ayarları ve sonradan eklenen Desktop tercihini korudu, çatışmada dosya/sahiplik değişmedi. [Ürün bağlantı kanıtı](macos-desktop-connection.json) · [Sabit resmî arşiv doğrulaması](macos-desktop-archive.json). Bu kayıt kendi başına güncel hesap veya aktif sohbet doğrulaması değildir.
