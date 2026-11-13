# H050 — Hesap durumu ve resmî giriş

13 Kasım 2026. Uygulama ve auth/süreç kararlarının sahibi ana ajandır. Aynı tek Luna yalnız sabit Claude/OpenCode komut sözleşmelerini salt okunur inceler. H049 kullanıcı kararıyla ertelenmiştir; CI kullanılmaz.

## Sözleşme

- Kurulum, yerel giriş bilgisi, Headroom yönlendirmesi ve ölçülen trafik ayrı durumlardır. Hesap okuması model/reasoning/provider seçmez ve model isteği göndermez. Yerel credential kaydı uzaktaki geçerli hesap/abonelik kanıtı değildir.
- Yalnız taze keşif seçimi ve yeniden pin'lenen güvenilir gerçek CLI çalıştırılır. İstemci path'i, argv veya URL webview'den alınmaz. Desktop/diğer OS veya bilinmeyen sürüm sözleşmesi desteklenmiyorsa durum bilinmiyor kalır.
- Codex resmî status komutu 10 saniye/akış başına 64 KiB sınırları ve yalnız kendi süreç ağacıyla çalışır. Claude Code 2.1.158 status komutu izole deneyde ayar/yedek yazdığı için pasif okuma olarak kullanılmaz: durum bilinmiyor, neden `read_not_passive`. OpenCode 1.19.3 `auth list` global DB/WAL/SHM oluşturduğu için kullanılmaz; sabit V1 şemasındaki yerel provider metadata'sı salt okunur, sınırlı ve dosya kimliği denetlenerek okunur.
- Raw stdout/stderr, key parçaları, email, token veya auth dosyası Headroom state/loglarına kopyalanmaz. OpenCode değerleri akış sırasında yalnız var/yok bilgisine indirgenir; bilinmeyen provider adları `other` olur. UI'ya yalnız sabit kategori/durumlar geçer. Şifre, API-key veya OAuth sırrı Headroom'da girilmez.
- Resmî giriş yalnız açık kullanıcı eylemidir. Etkilenen CLI profili ve resmî istemcinin mevcut oturumu değiştirebileceği gösterilir. İptal, yalnız başlatılan CLI sürecini durdurur; tarayıcıya, başka oturumlara veya auth deposuna geri alma uygulanmaz. İptal edilmiş girişten başarı çıkarılmaz; durum yeniden okunur.
- Hesap durumu, geçmiş kurulum kaydı veya süreç varlığıyla tahmin edilmez. Yeniden açılışta taze metadata okunur; canlı hesap erişimi doğrulanmadığı açık kalır. Kullanıcının önceki Claude giriş yapmama tercihi korunur.
- Giriş ayrı görünür resmî CLI konsolunda açılır; Headroom yalnız kendi başlattığı kök süreç handle'ını yönetir. Tarayıcı kill-on-close job'a alınmaz. Normal tam çıkış ve 600 saniyelik süre sınırı kendi CLI kökünü durdurur. Ani süreç çökmesinde girişin otomatik sonlanacağı iddia edilmez. Kapanış doğrulanamazsa sahiplik korunur, yeni işlem engellenir ve durdurma tekrar denenebilir. Ekran değişikliği/tepsiye alma akışı iptal etmez; IPC yanıtı kaybolursa yenileme mevcut akışı bulur.
- Codex giriş eylemi ChatGPT, Claude Code giriş eylemi Claude hesabını açar; OpenCode sağlayıcı/yöntemi kendi ekranında seçtirir. Mevcut API-key yönteminin değişebileceği onay ekranında açıkça yazılır. Başarı kodu bile uzaktaki geçerli oturum kanıtı sayılmaz; yerel kayıt yeniden okunur.

## Kaynak ve kritik sınır

[OpenAI kimlik doğrulama](https://learn.chatgpt.com/docs/auth), [CLI login/status](https://learn.chatgpt.com/docs/developer-commands?surface=cli) ve [resmî CLI login kaynağı](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/cli/src/login.rs) incelendi. Status credential varlığını bildirir; API-key biçimlenmiş parçalarını da yazabilir, bu parçalar tutulmaz. Login mevcut auth'u akış tamamlanmadan temizleyebilir; iptal eski auth'un korunduğu anlamına gelmez. İncelenen sürüm kaynakları ve hash'leri görev kanıtında sabitlenir.

Güncel [OpenCode belgeleri](https://opencode.ai/docs/cli/providers/) V2 davranışını mevcut V1 1.19.3'e aktarmayız. [Sabit V1 auth şeması](https://github.com/anomalyco/opencode/blob/e3c4eb242585dcd77be1a867391b7c25fd47ce52/packages/opencode/src/auth/index.ts), global data kökü ve xdg-basedir 5.1.0 kaynakları incelendi. `OPENCODE_AUTH_CONTENT` veya güvenli olmayan/bilinmeyen depo varsa durum bilinmiyor kalır. Claude komut sözleşmesi sabit kurulu istemcinin resmî help çıktısıyla incelendi; kullanıcı hesabında komut çalıştırılmadı.

Windows CLI auth kabiliyetleri Codex `0.163.0-alpha.11.1`, `0.163.0-alpha.11.2`, `0.167.0-alpha.4.1`; Claude Code `2.1.158`; OpenCode `1.19.3` ile sınırlandırılmıştır. Bunlar model/alt ajan desteğinin sınırı değildir. Gerçek `.exe`, sürüm/digest veya imza doğrulaması ve pinlenen üst dizinler zorunludur; wrapper'lar, Desktop hesapları ve başka OS için güvenilir adapter yoksa durum bilinmiyor kalır.

## Kabul ve doğrulama

1. Taze kurulum seçimi → durum oku → yalnız allowlist metadata; var/yok/bilinmiyor ve Desktop/unsupported ayrımı. Bozuk, aşırı uzun, başarısız/timeout çıktı bağlantı başarısına dönüşmez.
2. Kullanıcı kapsamı görür → resmî giriş açılır → iptal/çıkış/yeniden durum okuma. Stale seçim, değiştirilmiş executable ve rakip login reddedilir. Başka süreç/tarayıcı kapatılmaz.
3. Accounts UI gerçek IPC yolunu kullanır; TR/EN, boş/unknown/hata, klavye ve son native render kontrolü yapılır. Fixture ve gerçek istemci kanıtı ayrı tutulur; canlı giriş yapılmadığında açıkça belirtilir.

Odaklı Rust `integrations::accounts` ve pin regresyonları, frontend accounts sözleşmesi ve etkileşimi, son adayda WORKFLOW.md tam yerel kapısı bir kez. Yerel kanıt Windows 10'dur; Windows 11, yeni macOS özelliği veya Linux'a aktarılmaz. Başarılı eski testler yalnız ilgili fingerprint eşleşiyorsa kullanılır.
