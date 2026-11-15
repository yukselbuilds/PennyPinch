# OpenCode Desktop bağlantısı

15 Kasım 2026. Windows 10 22H2 x64 (19045), OpenCode Desktop 2.3.0 ve Headroom 0.41.0. [Kabul planı](../decisions/009-opencode-provider-routes.md#h010-j--opencode-desktop-paketli-servis), [programatik kayıt](opencode-v2-desktop.json), [inceleme](../reviews/H010-j.md).

Resmî Windows NSIS paketinin yayın digest'i, SHA256SUMS ve yerel SHA-256 eşleşti. Paketin masaüstü payload'ı proje içinde açıldı. Kurucudaki AppContainer okuma/çalıştırma izni aynı payload'a uygulandı. Bu koşu Electron uygulamasını çalıştırdı. NSIS kurulumu ve kısayol işlemleri ayrıca sınanacak. [Sabit paketleme kaynağı](https://github.com/anomalyco/opencode/blob/a6b15d33daf6ab7d35569f6d711fdb630b9f04e7/packages/desktop/electron-builder.config.ts).

Headroom'daki tanınmış 2.3.0 CLI üzerinden Önizle ve Uygula, paylaşılan kullanıcı JSONC'sini bağladı. Desktop kendi paketli CLI'sini ayrı uygulama profiline kopyaladı ve normal servisini başlattı. Bu dosyanın hash'i paketle eşleşti. Servis kaydı, PID, Windows Job ve loopback dinleyicisi doğrulandı. Testin ev, Documents, config, data ve cache yolları ayrı profile aitti. Windows'ta servis başlangıcının yeniden belirlediği state yolu da bu profildeydi. [Paketli CLI kaynağı](https://github.com/anomalyco/opencode/blob/a6b15d33daf6ab7d35569f6d711fdb630b9f04e7/packages/desktop/src/main/service/desktop-cli.ts).

Gerçek penceredeki model seçimi `openai/slot`, Responses olarak göründü. Mesaj, GUI'deki yazma alanına girilip Send düğmesiyle gönderildi. Sabit yerel sağlayıcı yalnız paketli servise verilen sentetik anahtarı kabul etti. Dört model isteği, üç tamamlanmış araç çağrısı ve arayüzde görünen final eşleşti. Ayrı SQLite kontrolü tek kök oturumun modelini, mesajını, araç sırasını ve başarılı sonucunu doğruladı.

Örnek log **14.999 → 103 bayt**, yani bu blokta **%99,31** azaldı. CCR, özgün bayt ve hash'i geri getirdi. Bu sentetik koşu gerçek token veya fatura ölçümü içermez.

Normal Electron quit, işletim sistemindeki ana süreç tanıtıcısından exit 0 olarak doğrulandı. Bağımsız paketli servis ayrıca normal stop ile durdu. Servis kaydı, port ve test süreçleri kapandı. Headroom Kaldır özgün JSONC'yi birebir döndürdü. Test motor ayarı geri yüklendi. Uygulamanın oluşturduğu geçici `opencode:` protokol kaydı, sahibinin test executable'ı olduğu doğrulanıp temizlendi.

Nihai koşu `work/proofs/opencode-v2-service/native-desktop-9600eec937/result.json`. Önceki hazırlık koşuları kendi başarısız kayıtlarında tutuldu. Windows home yolu ve özel Electron yükleme olayı için test düzeni düzeltildi. Bir koşu, masaüstü turunu bitirip test taşıyıcısının kapanışında süre aşımına girdi. Son aday ana uygulamanın normal çıkışını, arka planda yaşamaya devam eden servisten ayrı denetler. Aradaki bir Uygula hatasının nedeni belirlenemedi. Son adayda aynı Uygula yolu geçti ve bütün önceki koşuların geri alma/temizlik kayıtları doğrulandı.

Bu sonuç, Headroom'daki CLI kurulumu ile paylaşılan config'i kullanan Desktop içindir. Yalnız Desktop kurulu bilgisayarda otomatik keşif/bağlama, Desktop alt ajanı, mevcut servisin ayarı anında benimsemesi, canlı hesap ve başka işletim sistemi ayrı doğrulama bekliyor.

Son native koşuda 22.818 kaynak, paket ve fixture girdisi önce/sonra eşleşti. Katalog güncellemesinden sonraki tek yerel kapı, 344 eşleşen girdiyle 222 Node ve 462 Rust testi geçirdi. 19 test ignored. Rustfmt, frontend ve Tauri embedded derlemeleri geçti.
