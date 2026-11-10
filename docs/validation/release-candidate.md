# H039 — Yerel son aday doğrulaması

10 Kasım 2026. Kullanıcı ücretli Windows 11 ortamını kullanmadan devamı seçti. H041/H042’nin dış kabulleri açık tutularak H039’un yerel incelemesine geçildi. Windows 11 x64 `not_tested`; Windows 10 sonucu onun yerine geçmez. Kamuya yayın, üretim anahtarı ve CI çalıştırması bu görev diliminde yapılmaz.

## Kapsam ve sahiplik

Ana ajan son kaynak/paket/süreç/ölçüm güvenini, gerçek native akışı ve kabulü üstlenir. Aynı tek Luna xhigh çalışanı yalnız kılavuz/README iddialarını dar inceler ve `USER_GUIDE.md` taslağını yazar. Model ve sağlayıcı ayarları değiştirilmez. İnceleme, son değişiklik ve bağlı akışlar üzerindedir; yeni bir bütün depo denetimi değildir.

Son aday, güncel kaynak ve sabit payload ile ayrı QA kimliğinde gerçek current-user NSIS paketidir. Debug discovery trace kapalıdır. Varsayılan release derleme ayarları ve NSIS sıkıştırması kullanılır; üretim kaynağı/anahtarı tanımlanmaz. Kurulum, test dosyaları ve kanıtlar `work/rc39/` içindedir; yalnız QA uygulamasına ait state/registry/installer yönetilir. Diğer uygulama ve hesapların ayarları korunur.

## Kabul senaryoları

1. **Normal:** Son paket ayrı dizine kurulur. Uygulamalar → OpenCode / Dosyadan ekle ile çalıştırılmayan boş `opencode.cmd` seçilir. Art arda üç yeniden tarama ve başka ekrana gidip dönme sonrasında Dosyadan eklenen / Selected file satırı görünür kalır. Bu, uygulama oturumu kapsamındaki keşiftir; açıkça kaydetmeden yeniden başlatmada kalıcı tercih iddiası kurulmaz.
2. **Hata/iptal:** Dosya seçimini iptal etmek önceki satırı korur. Kaynak/UI incelemesinde picker ve scan yarışları, stale sonuç koruması, ayar/ölçüm semantiği ve kullanıcıya sunulan iddialar incelenir. Somut kritik hata bulunursa bu görev içinde sınırlı düzeltme ve odaklı regresyon yapılır; kabul gevşetilmez.
3. **Sınır/teslim:** Manuel aday gerçek native paket ve debug trace kapalı koşuda gözlenir. Kaynak/test/config/araç ve paket/kurulu dosya özetleri aynı kabul adımında bağlanır. Son adayın gerçek yerel kalite kapısı tamamlanır; değişmeyen PASS yalnız ilgili fingerprint/araç eşleşmesiyle yeniden kullanılır. QA kurulum ve süreçleri sonunda kendi sahiplik sınırında temizlenir.

## Doğrulama yolları

- Mevcut kapı: `docs/development/WORKFLOW.md` içindeki `test:app`, frontend build, fmt, offline Rust test ve custom-protocol build komutları. Tek son adayda, inceleme ve düzeltmelerden sonra. Frontend kaynak/araçları değişmediyse 179 test ve build kanıtı korunur; yeni Rust verifier testinin eklendiği girdi değişimi nedeniyle Rust kapısı yenilenir.
- Paket: pinned Python ile `scripts/package-windows.py`, Tauri CLI 2.12.3 ve mevcut NSIS cache’i. Yeni QA config’i, kaynak değişiklikleri olmadan derleme/paket sürecine verilir. Doğrulanmış motor dosyaları tam envanterle bağlanır.
- Native UI: Computer Use üzerinden gerçek pencere, dosya seçici, yeniden tarama ve gezinme. Mock, CDP fixture veya debug trace bu native kabulün yerine geçmez.
- Destek tablosu Windows 10 yerel / Windows 11 denenmedi / macOS-Linux portu açık / gerçek API-key ve üretim yayını açık ayrımını korur. Geliştirme token tasarrufu ölçülmezse oran yazılmaz.

## Sonuç — yerel kabul geçti

10 Kasım, Windows 10 x64 build 19045. [Toplu kanıt](../proofs/release-candidate.json), [inceleme](../reviews/H039.md), [kullanıcı kılavuzu](../USER_GUIDE.md).

- Release exe’sindeki konsol türü hatası gerçek PE kontrolüyle bulundu. Test önce başarısız oldu; Windows GUI niteliğinden sonra subsystem=2 kontrolü geçti. Paketleme komutu aynı guard’ı içerir. İlk kesilmiş aday ve koşucu hatası PASS sayılmadı.
- NSIS/LZMA paketi varsayılan LTO açık/codegen 1/strip/panic abort profiliyle ayrı QA kimliğine kuruldu. 22.333 resource dosyası doğrulandı. Tauri’nin tek NSIS paket türü işareti dönüşümünden sonra exe’nin tüm baytları beklenenle eşleşti; başka fark kabul edilmedi.
- Kullanıcı yalnız hazır yolun Open onayını yaptı. Ana ajan üç taramayı tamamladı, Etkinlik’e gidip döndü ve manuel kaynağın kaldığını native erişilebilirlik ağacından okudu. Hata/stale bandı yoktu. Çıkıştan vazgeçme satırı korudu; gerçek onaydan sonra süreç kod 0 ile kapandı. Discovery trace yoktu. Native dosya seçici iptali yeniden denenmedi; iptal/yarış testleri kaynak kapısındadır.
- Son kapı: 18 Python ve 413 Rust testi, fmt ve custom-protocol derlemesi geçti. 15 opt-in Rust testi ignored. Değişmeyen 179 frontend testi/build, 112 girdi ve aynı araç sürümleriyle korundu. 232 kaynak girdisi önce/sonra eşleşti; tekrar ürün testi çalıştırılmadı.
- Kaldırıcı kod 0 verdi. QA registry kaydı, exe, state ve sidecar lock kalmadı. Kesilmiş ilk kurulum yalnız proje içindeki başarısız artefakt alanında korunur. İstemci ayarlarına yönlendirme uygulanmadı; sağlayıcı isteği 0. Public yayın ve CI çalıştırması yok.

| Ortam / kapsam | Sonuç |
| --- | --- |
| Bu Windows 10 x64 hostunda release/NSIS/native akış | Geçti; temiz OS kabulü değildir |
| Windows 11 x64 | `not_tested`; kullanıcı ücretli ortamı seçmedi |
| macOS / Linux | Port ve native kabul açık |
| Canlı Codex API-key | H041’de açık |
| Üretim update feed/key/HTTPS teslimi | H042’de açık |
| Temiz Windows / WebView2 bulunmayan ortam | H037’de açık |

Bazı arka plan görüntüleri öndeki Codex penceresini gösterdi; bunlar kullanılmadı. Native sonuçlar hedef Headroom erişilebilirlik ağacı, gerçek kurulum ve sahipli süreç çıkış kaydıyla sınırlıdır. H039 yerel kabulü; kamuya sürüm, temiz kurulum veya ek platform desteği değildir.
