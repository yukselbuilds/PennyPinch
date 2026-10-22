# Yerel uygulama güven sınırları

19 Ekim 2026 — H005 öncesi inceleme temeli. Kontroller ilgili dilimlerde sınanır; bu belge uygulanmış güvenlik kanıtı değildir.

| Sınır / varlık | Somut kötüye kullanım | Gereken kontrol ve kanıt |
| --- | --- | --- |
| Masaüstü UI → Rust yönetici | UI'dan keyfi komut, dosya yolu veya uzak URL gönderilmesi | Tipli dar komutlar; yönetici tarafından belirlenen yollar/argümanlar; uzaktan içerik çalıştırmayan CSP; genel shell izni yok. |
| Yönetici → motor listener | Aynı porttaki başka HTTP servisinin Headroom sanılması veya kapatılması | Çocuk süreç handle/PID, loopback sahiplik kontrolü, beklenen readiness gövdesi; port meşgulse hata/başka port; yalnız sahip olunan süreci kapatma. H005. |
| İstemci ayarı → adaptör | Kurulumdan sonra kullanıcının yaptığı değişikliğin kaldırma sırasında silinmesi | Önerilen değişikliği önce hesapla; atomik yazım; değiştirilen alanların sahipliği ve fingerprint'i; çatışmada bütün dosyayı eski yedekle ezme. H017–H018. |
| Mevcut giriş → proxy → sağlayıcı | Auth bilgisinin log, state, UI veya yanlış upstream'e sızması | Mevcut auth akışını koru; sabit ve izinli sağlayıcı hedefleri; keyfi yönlendirme yok; yalnız gereken başlıkların iletimi; içeriksiz rapor. H008–H010. |
| Motor → ölçüm/depolama | Prompt/tool çıktısının etkinlik kaydına veya dışa aktarmaya karışması | İzinli metadata alanları; içerik ve auth filtreleme; boyut/süre sınırı; bilinmeyen alanları UI/log'a dökme. H023–H027, H033. |
| CCR özgün içeriği | Süreç kapanışı veya duraklatmada özgün içeriğin beklenmeden kaybolması | TTL ve sahiplik; pause sırasında aynı retrieval deposu ve aktif stream'in korunması; açık silme/kapanış semantiği. H007, H011, H028. |
| Paket kaynağı → kurulum | Yanlış paket, değiştirilmiş motor veya beklenmeyen bağımlılık | `headroom-ai` sabit sürümü, lock/hash, resmî kaynak; yalnız gereken extras; paket lisansları; temiz ortam çalıştırma. H005–H006, H037. |
| Program çıkışı → açık istemci | Proxy'ye yönlenmiş açık oturumun sessizce bağlantısız kalması | Tepsiye küçültme; etkin bağlantı/stream bilgisi; sahip olunan ayarları güvenle kaldırma veya çıkış etkisini açık gösterme. H029–H030, H038. |

İlk teknik deney mevcut kullanıcı istemci ayarlarını değiştirmez ve gerçek sağlayıcı çağrısı gerektirmez. Gizli değerler okunarak teşhis çıktısına kopyalanmaz. Başarısız testin ham günlüğü kalıcı kanıt olarak saklanmaz.

Tauri sidecar ve Windows paketleme mekanizmaları aday yaklaşımı destekler; Headroom'un taşınabilirliği ayrıca kanıtlanmalıdır. Kaynaklar: [Tauri sidecar](https://v2.tauri.app/develop/sidecar/), [Windows installer](https://v2.tauri.app/distribute/windows-installer/). Son kontrol: 19 Ekim 2026.
