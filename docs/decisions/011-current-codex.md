# 011 — Güncel Codex keşfi ve ortak kullanıcı bağlantısı

3 Kasım 2026. H008 kullanım düzeltmesi; ana ajan auth/rota/süreç sahipliğini üstlendi. Tek Luna yalnız bağımsız, sentetik CLI/app-server fixture yardımcılarını yazdı.

## Sorun

Eski iki binary boyut/hashine bağlı keşif, güncel CLI0.166.0 ve Desktop backend0.166.0-alpha.5.1'i tanımıyordu. Plan, Apply, durum okuyucusu ve frontend ayrıca eski CLI sürüm/yüzey listeleri kullanıyordu. Standalone kurulum yerel junction kullanıyor. Desktop paketi WindowsApps kökü listelenemediği için bulunamıyor; paketteki Codex.exe küçük bir launcher, gerçek pencere ChatGPT.exe ve ürün metadata'sı Codex. Uzun PATH ise Desktop kontrolünden önce aday deneme bütçesini tüketiyordu.

## Karar

- Legacy tam hash kanıtları korunur. Diğer bounded binary'lerde WinVerifyTrust ile OS güven zinciri, cached revocation ve tam OpenAI OpCo, LLC signer O/CN kontrol edilir. Zincir/publisher doğrulanmadan sürüm komutu yürütülmez. Yeni binary hashini bir izin listesine eklemek gerekmez; yayıncı değişirse yeniden inceleme gerekir.
- Yerel Codex junction hedefleri en fazla8 adımda çözülür. Ağ/device/relative/parent hedefleri ve son hedefte kalan reparse bileşenleri reddedilir. Canonical hedef, dosya kimliği, boyut ve SHA256 kontrolü; yazma/silmeyi engelleyen read-only handle; doğrulama sürecindeki parent pin'leri korunur. Başka istemcinin path kuralları gevşetilmez.
- İmzalı dosyanın --version komutu shellsiz, auth/config ortamı devralmadan, kapalı stdin/stderr,256 bayt stdout,5 saniye ve owned JobObject ile çalışır. Küçük digest→version cache'i yalnız tekrar version process'ini azaltır; imza ve dosya tekrar doğrulanır. Offline trust yeni çevrimiçi revocation kontrolü değildir.
- Windows GetPackagesByPackageFamily/GetPackagePathByFullName, mevcut kullanıcıya kayıtlı OpenAI.Codex_2p2nqsd0c76g0 paketini salt okunur bulur. Admin izni veya WindowsApps kök listeleme gerektirmez. İmzalı uygun GUI metadata'sı olan executable seçilir; sürüm package kimliğinden gösterilir. Kayıtlı Desktop, PATH deneme bütçesinden önce kontrol edilir. Sayı/boyut/path sınırları büyütülmez.
- CLI0.x minor>=163 ve geçerli bounded sürüm, mevcut openai_base_url kullanıcı config sözleşmesini deneyebilir. Bu bir canlı protokol/model garantisi değildir; diğer major sürüm inceleme bekler. Desktop'ın3/4 bileşenli uygulama sürümü CLI sürümüyle karşılaştırılmaz. Diğer istemcilerin sürüm/surface sınırları korunur.
- Codex CLI ve Desktop kullanıcı bağlantısı aynı atomik Plan→Apply→Undo sözleşmesinden geçer. Model/reasoning/provider/auth ve kullanıcı ekleri korunur; sahiplik, fingerprint, runtime lease, stale seçim ve çatışmayı koruyan undo değişmez. Frontend metadata'yı eski binary sürümlerinden oluşan ikinci bir listeyle reddetmez.
- Yeni süreç doğrulaması CLI içindir. Desktop seçimi aynı bağlantı kapsamını ve yeniden açılış ihtiyacını açıklar; CLI yanıtı açık Desktop sohbetinin adoption kanıtı olmaz. Model adı veya alt ajan modeli için izin listesi eklenmez.
- Verifier workspace'i debug ve release'te uygulamanın local state kökündedir. Checkout içindeki CodeGraph/project config'i testi engellemez; ancestor config kontrolü kaldırılmaz. Mevcut kullanıcı ayarı yalnız okunur, geçici endpoint override'ı yalnız yeni CLI sürecine verilir. Eksik app-state kökü güvenli biçimde kullanılamazdır.
- Debug taramanın büyük binary hashleri sha2 dependency optimizasyonuyla hızlandırılır; release zaten optimize edilir. Bu token tasarrufu ölçümü değildir.

## Kabul sınırı ve kaynaklar

Keşif/config yeteneği, fixture transport'u, yeni CLI sürecinin gerçek yanıtı ve açık Desktop/alt ajan trafiği ayrı kanıtlardır. Generic CLI hata bildirimleri başarılı terminalden bağımsız tutulur. Yerel başarı Win11/diğer OS veya bütün modellerin bütün özellikleri yerine geçmez. Tarihsel manual-codex launcher eski sabit binary kanıtıdır; güncel ürün doğrulaması olarak sunulmaz.

- [Güncel kanıt](../proofs/codex-current.md), [inceleme](../reviews/H008-current.md).
- OpenAI [app-server](https://learn.chatgpt.com/docs/app-server), [ayar katmanları](https://learn.chatgpt.com/docs/developer-settings), [built-in provider endpoint](https://learn.chatgpt.com/docs/config-file/config-advanced).
- Microsoft [WinTrust offline seçenekleri](https://learn.microsoft.com/en-us/windows/win32/api/wintrust/ns-wintrust-wintrust_data), [kayıtlı kullanıcı paketleri](https://learn.microsoft.com/en-us/windows/win32/api/appmodel/nf-appmodel-getpackagesbypackagefamily).
