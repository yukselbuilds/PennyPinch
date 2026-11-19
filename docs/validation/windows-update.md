# H042 — Windows güncelleme doğrulaması

## H042-a — Yerel güncelleme güveni

10 Kasım 2026. Windows 10 x64 build 19045 üzerinde yerel manifest ve kurucu bütünlüğü kabulü geçti. Ana H042’nin uygulama bağlantısı, gerçek güncelleme ve geri dönüş dilimleri açık. [Karar](../decisions/028-update-trust.md), [doğrulama kaydı](../proofs/windows-update-trust.json).

Rust doğrulayıcı, güvenilen açık anahtarla manifestin ham baytlarını doğruladıktan sonra içeriğini okur. İmzalı sürüm, kanal, uygulama kimliği ve Windows x64 hedefi kurulu uygulamanın bilgileriyle karşılaştırılır. Kurucu adı geçerli uygulama sürümünden türetilir. Motor sürümü ve payload manifestinin SHA-256 değeri mevcut runtime sabitlerinden alınır. İlk güncelleme şeması aynı motor pinini taşıyan uygulama paketlerini kabul eder.

Kararlı ve deneme manifestleri Tauri CLI 2.12.3 ile ayrı ayrı imzalandı. İmzalı çift sürüm alanı ve yanlış motor pini de fixture olarak üretildi. Anahtarların yalnız açık bölümleri ve imzalar depoda tutulur. Geçici özel fixture anahtarları silindi. Git, imzalanan JSON dosyalarını LF baytlarıyla teslim eder. Test kurucusu sentetik baytlardır.

Sekiz odaklı test, doğru kanal ve imzayı, yanlış veya biçimsiz anahtarı, değiştirilen manifesti, yanlış hedef ve şemayı, eski sürüm reddini ve motor pini uyuşmazlığını kapsadı. Kurucu denetimi aynı uzunluktaki değişikliği SHA-256 ile yakaladı. Eksik, fazla ve okunamayan veri ayrı hata verdi. Uzun veri imzalı boyut artı bir baytta kesildi. 281 kaynak ve araç parmak izi doğrulama öncesi ve sonrası eşleşti.

Proje kökünde, belgelenen proje Cargo cache’i ile tekrar:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --offline --locked --manifest-path src-tauri/Cargo.toml desktop::update::tests -- --test-threads=1
```

Ana ajan imza kontrolünün ayrıştırmadan önce yapıldığını, manifestin güven anahtarı veya URL seçemediğini ve doğrulanmış metadata ile kurucu boyut/hash denetiminin aynı Rust tipi üzerinden geçtiğini inceledi. `minisign-verify` 0.3.0, mevcut Base64, SemVer ve SHA-256 bağımlılıklarıyla kullanıldı. H042-b yerel doğrulayıcıyı güvenilen kaynağa, sahipliği korunan indirmeye ve uygulama ekranına bağlar. H042-c kurulu sürüm geçişini ve son çalışan sürüme dönüşü gerçek paketlerle sınar.

## H042-b — Güncelleme ekranı ve indirme

10 Kasım 2026. Windows 10 x64 build 19045 üzerinde 13 odaklı Rust testi, 177 uygulama testi ve 8 native kontrol geçti. Doğrulama öncesi ve sonrası 336 odaklı kaynak/araç girdisi ve 182 native girdi eşleşti. İki koşunun ortak 165 girdisi de aynıydı. [Kanıt](../proofs/windows-update-flow.json).

Türkçe Ayarlar ekranı ayrı bir Tauri/WebView2 test penceresinde kararlı `0.1.1` sürümünü, imzalı notu ve 39 baytlık fixture boyutunu gösterdi. Başlatılmış yerel model akışı, güncelleme indirilip doğrulanırken açık kaldı ve son yanıtı teslim etti. Paket, motor çalıştığı sürece bekledi ve motor durdurulunca hazır göründü. İngilizce ekran hazır durumu, deneme kanalı `0.1.1-rc.1` sürümünü gösterdi. Normal çıkıştan sonra geçici dosya kalmadı. Motor giriş portu ve test WebView portu kapandı. İlk koşulardaki bağlantı sayısı, durdurma düğmesi ve port gözlemi sorunları test sürücüsünde düzeltildi.

Odaklı testler yanlış imzayı, yanlış hash’i, kesilen indirmeyi, HTTP yönlendirmesini ve önceki kanala ait aday kimliğini reddetti. Yanlış motor pini, kurulu sürümle aynı sürüm numarasında da reddedilir. Ana ajan arayüz eyleminden Tauri komutuna, kaynak ve açık dosya doğrulamasından motor durumuna kadar bağlı akışı inceledi. Bekleme kararı, motorun durduğuna dair doğrulama gerektirir. Kurulum sırasında durum yeniden doğrulanacaktır.

Yerel test imzalı sentetik paket ve loopback kullanır. Üretim derlemesi doğrudan HTTPS tabanı ve güvenilen açık anahtarı `HEADROOM_DESKTOP_UPDATE_BASE_URL` ve `HEADROOM_DESKTOP_UPDATE_PUBLIC_KEY` üzerinden alır. Yayın yapılandırması, gerçek kurucu geçişi ve geri dönüş ile Windows 11 doğrulaması açık kalır.

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --offline --locked --manifest-path src-tauri/Cargo.toml desktop::update -- --test-threads=1
pnpm.cmd test:app
```

## H042-c1 — İmzalı NSIS hazırlığı

10 Kasım 2026. Windows 10 x64 build 19045’te NSIS 3.13, önceki doğrulanmış H038 QA exe’sinden 2.943.643 baytlık güncelleme paketi üretti. Paket hazırlama komutu olmadan kod 2 ile çıktı. Hedefteki mevcut dosyayı korudu. Normal çıkarma kaynak exe ile aynı SHA-256 değerini verdi. [Kanıt](../proofs/windows-update-package.json).

Gerçek NSIS paketi üç geçici Tauri imzasıyla yerel HTTP fixture’ında sınandı. İmza ve paket özeti doğrulanınca NSIS ayrı dizinde çalıştı. Çıkarılan uygulama tekrar boyut ve özet kontrolünden geçti. Aynı boyuttaki yanlış özet ve uyumsuz veri biçimi ayrı reddedildi. Veri biçimi uyuşmazlığında paket isteği yapılmadı. İlk koşuda kalan NSIS geçici dizini, çıkarmayı mevcut çalışma dizinini değiştirmeden yapan yerleşik NSIS komutuyla düzeltildi. Fixture özel anahtarı silindi.

Ayrı Tauri penceresinde `0.0.9` test sürümü, Ayarlar üzerinden `0.1.0` paketini indirip hazırladı. Deneme kanalındaki yanlış özet hata gösterdi ve yeniden indirme düğmesi kullanılabilir kaldı. Motor durmuş kaldı. Normal kapanışın ardından hazırlık dosyaları ile iki test portu temizlendi. 14 odaklı Rust, 1 gerçek NSIS, 177 uygulama testi, üretim arayüzü derlemesi ve 5 native kontrol geçti. 314 paket/araç girdisi, 189 native girdi ve iki koşunun ortak 166 girdisi eşleşti.

Paket derlemesi, sabitlenmiş NSIS ve aynı sürümün gerçek release exe’siyle `src-tauri/windows-update.nsi` şablonuna `APPLICATION` ve `OUTPUT` tanımlarını geçirir. Üretimde bununla aynı sürüme ait imzalı v2 manifest ve uygulama özeti kullanılır. Sonraki doğrulama ayrı QA kurulumunda exe değişimi, açılış ve geri dönüştür. Disk dolu ve kesinti senaryoları, gerçek yayın kaynağı ve Windows 11 masaüstü kabulü açık kalır.

## H042-c2 — Kurulu geçiş ve kurtarma

10 Kasım 2026. Windows 10 x64 build 19045 üzerinde ayrı `dev.headroom.desktop.h042-c9eb52e7` kimliğiyle gerçek NSIS kurulumu yapıldı. Yeni ve önceki uygulama aynı kaynaklardan, farklı sürüm bilgisiyle derlendi. Güncel motor kaynakları ayrı paketleme alanında pinlerle doğrulandı. QA derlemesinde yalnız yerel yayın fixture’ını kabul eden derleme koşulu ve fixture anahtarı değiştirildi. Release LTO kapalı, codegen birimi 16 idi. Üretim HTTPS kaynağı bu koşunun kapsamı dışındadır. [Kanıt](../proofs/windows-update-install.json).

Normal koşunun 8 kontrolü Ayarlar’da imzalı paketi hazırlama, motor çalışırken kurulumu reddetme, onayı iptal etme, yeniden onaylayıp yeni sürümü açma ve önceki sürüme dönüşü kapsadı. `0.1.0 → 0.1.1 → 0.1.0` geçişlerinde tercih dosyasının SHA-256 değeri aynı kaldı. Örnek kullanıcı dosyası korundu. Windows kayıtlı sürümü geri döndü. Kurulu ilk exe ile geri dönen exe’nin SHA-256 değerleri eşleşti.

İkinci koşunun 5 kontrolünde yeni sürüm açıldıktan sonra yalnız test Job’una ait uygulama süreci kod 42 ile sonlandırıldı. Saklanan kurtarma exe’si, imzalı kayıt ve dosyaları tekrar denetleyerek önceki sürümü açtı. Tercih ve kullanıcı dosyası özetleri korundu. Her iki koşunun sonunda test portları kapandı ve hazırlık dosyası kalmadı. Dosya yayımlama anındaki kesinti ayrı kabulde açık.

Son yerel kapı 179 uygulama testi, TypeScript/Vite, fmt, 411 Rust testi ve custom-protocol derlemesiyle geçti. 11 opt-in test bu kapıda çalıştırılmadı. Dosya testleri normal değişim/geri dönüş, yarım hazırlık, oynanmış kaynak, kilitli hedef ve kullanıcı tarafından değişen canlı dosyayı korumayı kapsar. Motor rezervasyonu ve kapanış kabulü ayrıca sınandı. Kalite kapısı ile release QA derlemesinin ortak 165 girdisi eşleşti. Test çıktıları kendi önce/sonra parmak izlerine bağlıdır.

İlk denemedeki NSIS paket türü işareti farkı, sabit Tauri Utils 2.10.2 kaynağındaki `__TAURI_BUNDLE_TYPE_VAR_NSS` sözleşmesiyle doğrulandı. Eski staging kaynağı da motorun pin kontrolünde reddedildi ve güncel kaynaklardan tekrar hazırlandı. Bu teşhis koşuları kabul kanıtı olarak kullanılmadı.

Bu ilk kurulu adaydan sonraki Windows 10 hata kabulü aşağıdaki bölümde kaydedildi. Windows 11 x64 ve üretim yayın kaynağı/açık anahtarı ayrı kabulde sürer. Gerçek sağlayıcı isteği yapılmadı.

Kabulden sonra iki QA kurulumu kendi kaldırıcılarıyla kaldırıldı. İki uygulama kaydı ve yalnız bu testlere ait LocalAppData kökleri temizlendi. [Temizlik kaydı](../../work/proofs/update/cleanup-installed.json).

## H042-c2 — Kalan Windows 10 hata kabulü

10 Kasım 2026. Son kaynakta Windows 10 x64 build 19045 yerel kabulü geçti. 19 odaklı güncelleme/dosya testi ve iki imzalı exe/HKCU fixture testi başarılı. Son kapı 413 Rust testi, fmt ve custom-protocol derlemesiyle geçti. Arayüz kaynakları ve araçları aynı kaldığı için 179 test ve derleme, 123 girdi eşleşmesiyle yeniden kullanıldı. [Kanıt](../proofs/windows-update-faults.json).

Test derlemesi kısmi yazıdan sonra `ERROR_DISK_FULL` verdi. İşlem yarım dosyayı kaldırdı ve canlı exe aynı kaldı. Ayrı bir sahipli test süreci `MoveFileExW` çağrısından hemen önce ve hemen sonra sonlandırıldı. Hedefte tam önceki veya tam yeni dosya bulundu, önceki yedek korundu ve yeni dosyadan eskiye dönüş geçti. Bu, API sınırlarında süreç kesintisi deneyidir. Ani güç kaybı durumu ölçülmedi.

Gerçek bir Windows fixture exe’si ön kontrolü geçtikten sonra normal açılışta kod 7 ile çıktı. Üretimin uygulama ve geri dönüş fonksiyonları, geçerli imzalı manifest ve ayrı HKCU kurulum kaydıyla çalıştı. Önceki exe, kayıtlı sürüm ve örnek kullanıcı dosyası aynı kaldı. Fixture anahtarı geçiciydi ve özel kısmı silindi.

32 kayıtlık geçmişte güncel geri dönüş saklandı, eşdeğer tekrarlar bir kayda indirildi ve imzalı eski kayıtlar temizlendi. Kilitli kayıt, yanlış imza ve yabancı kullanıcı dosyası korundu. Kilit bırakıldıktan sonra eski kayıt yeniden denemede kaldırıldı. Doğrulama boyunca dosyalar yazmaya/silmeye karşı açık tutulur. Silme erişimi alınırken aynı özetler tekrar kontrol edilir. Son silme sabitlenmiş dizinde açık dosya handle’ları üzerinden yapılır.

Güncel aday `dev.headroom.desktop.h042-7088752f` kimliğiyle ayrı gerçek NSIS kurulumunda sınandı. Sekiz normal ve beş çökme/kurtarma kontrolü geçti. Tercih ve kullanıcı dosyası özetleri korundu. İkinci gerçek güncellemede eski kayıt temizlendi ve bir yeni kayıt kaldı. Son durumda test portları kapandı. QA uygulaması kendi kaldırıcısıyla kaldırıldı, kurulum kaydı ve test state kökü temizlendi.

Bu QA derlemesi yerel yayın fixture’ını kabul eden açık derleme koşulunu kullanır. LTO kapalı ve codegen birimi 16’dır. Motor kaynakları önceki doğrulanmış paketle dosya ve kaynak özetleri eşleşerek yeniden kullanıldı. Windows 11 x64, üretim yayın kaynağı ve gerçek açık anahtar H042’nin açık dış kabulleridir.

## H042-d — Yerel yayın hazırlama komutu

10 Kasım 2026. Windows 10 x64 build 19045’te 7 Python ve 9 normal Rust testi ile fmt geçti. Gerçek NSIS 3.13, Tauri CLI 2.12.3 ve üretimdeki Rust doğrulayıcı kullanılarak kararlı ve deneme kanalları ayrı hazırlandı. Yanlış özel imza anahtarı, mevcut sürümle eşit sürüm ve var olan çıktı dizini reddedildi. İki yerel paketin çıkarılan x64 exe’si hash ve sürüm/kimlik/veri biçimi probe’unu geçti. 231 girdi önce/sonra eşleşti. [Kanıt](../proofs/windows-update-release-preparation.json).

Bu komut geliştiricinin yayın hazırlığı içindir; uygulamayı kullanan kişinin çalıştırması gerekmez. Derlenmiş gerçek release exe’si ve onunla aynı sürüm/kimliği taşıyan config gerekir. Açık anahtar exe’ye derleme sırasında gömülmüş olmalıdır. Komut uygulamayı derlemez, sürüm artırmaz veya anahtar üretmez. Parola gerekiyorsa `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` süreç ortamından yalnız signer’a aktarılır; komut satırına veya makbuza yazılmaz. Özel anahtarın içeriği veya özeti makbuza alınmaz.

Örnek yollar bir şablondur; bu girdiler gerçek yayın anahtarı ve release derlemesi belirlendikten sonra sağlanır. Repo kökünden:

```powershell
& .\work\proofs\engine\toolchain\cpython-3.13.13-windows-x86_64-none\python.exe -I -B .\scripts\prepare-windows-update.py `
  --application .\work\releases\inputs\headroom-desktop.exe `
  --config .\work\releases\inputs\tauri.release.conf.json `
  --output .\work\releases\0.1.1 `
  --public-key .\work\releases\keys\release.key.pub `
  --private-key .\work\releases\keys\release.key `
  --notes .\work\releases\inputs\notes.md `
  --channel stable --from-version 0.1.0
```

Çıktı proje `work` kökü içindedir ve mevcut dizinin üzerine yazılmaz. `stable` veya `preview` kanalında yalnız `manifest.json`, `manifest.json.sig` ve `Headroom_<sürüm>_x64-update.exe` bulunur. Bir üstteki `release-receipt.json`, dosya/kaynak/araç özetlerini ve `published: false` bilgisini tutar. JSON ve imza ham baytları teslimde korunmalıdır. Kanal dizinine fazladan dosya karışması veya girdi değişimi hazırlamayı iptal eder. Çakışan çıktı korunur; sahipli yarım dizin temizlenir.

Test exe’si, yalnız güncelleme probe’unu uygulayan küçük bir native fixture’dır; gerçek Headroom release exe’si veya kurulu güncelleme kabulü değildir. Özel fixture anahtarları koşu sonunda silindi. Üretim exe’sinin gerçek HTTPS kaynağına bağlanması, aynı güven anahtarına sahip kurulu sürümden teslim ve Windows 11 x64 kabulü açık kalır. Kullanıcının yerel hazırlık seçimine uygun olarak public depo veya yayın oluşturulmadı, CI tetiklenmedi. Ürün çalışma zamanı ve UI değişmediği için tam ürün kapısı tekrarlanmadı; H042-c2’nin önceki ürün kanıtları kendi kapsamını korur.

### H042-d2 — Gerçek Headroom exe’si

10 Kasım 2026, Windows 10 x64 build 19045. Güncel kaynak değiştirilmeden ayrı `dev.headroom.desktop.h042d-557cd46e` kimliği, `0.1.1` sürümü ve geçici açık anahtarla gerçek Headroom exe’si derlendi. Derleme yalnız süreç bazlı `TAURI_CONFIG` ve açık anahtar girdisini kullandı. `update-fixture` özelliği ve HTTPS kaynağı eklenmedi. Release-mode QA profili LTO kapalı/16 codegen birimi/strip açık olarak kaydedildi. [Kanıt](../proofs/windows-update-real-application.json).

H042-d komutu, 9.915.392 baytlık uygulamayı NSIS ile paketledi. İmza, motor pini, uygulama ve paket özetleri gerçek Rust doğrulayıcıdan geçti; açılan gerçek exe’nin kimlik/sürüm/veri biçimi probe’u başarılıydı. Gerçek exe yanlış sürüm ve kimlikte kod 2 verdi; bu config’lerle hazırlama kod 1 ve hazır çıktı yok sonucunu verdi. Yanlış açık anahtar da reddedildi. Kaynak/test/araç/gömülü asset için 236 girdi ve araç sürümleri önce/sonra eşleşti. Önceki 7 Python/9 Rust testi, 231 kaynak/araç girdisiyle ve aynı araç sürümleriyle yeniden kullanıldı. Frontend’in 60 kaynak/asset/lock girdisi aynıydı.

Özel fixture anahtarı silindi. Yarım hazırlık dizini ve QA uygulama veri klasörü kalmadı. Bu koşu yalnız exe probe’unu çalıştırdı; kullanıcı arayüzünü açma, kurulum, motor trafiği veya kurulu sürüm geçişi deneyi değildir. Gerçek yayın profili/HTTPS teslimi/anahtarı ve Windows 11 kabulü açık kalır. Yerel QA artefaktı yayınlanmaz. Ürün kaynağı değişmediği için mevcut testler, tam yerel kapı veya CodeGraph gereksiz yere tekrarlanmadı.

### 19 Kasım 2026 — Güncel 0.44.0 Windows 10 QA paketi

Önceki gerçek uygulama hazırlığı 0.41.0’a aitti. 6702b64 kaynağı ve 0.44.0 piniyle gerçek Headroom release exe’si yeni bir test uygulama kimliğinde derlendi. Yerel NSIS güncelleme paketi sürüm 0.1.1 için üretildi. Manifest imzası, motor pini ve iki dosya özeti üretimdeki Rust doğrulayıcıda geçti. Çıkarılan exe kaynakla birebir eşleşti ve kimlik, sürüm ve durum biçimi kontrolünü geçti. Yanlış kimlik, sürüm ve açık anahtar reddedildi. Geçici özel test anahtarı iş sonunda kaldırıldı. [Kanıt](../proofs/windows-update-044-qa.json).

Yedi odaklı Python testi geçti. 364 build girdisi ve araç sürümleri önce/sonra eşleşti. Önceki kaynak kapısının 304 girdisi ve gömülü ön yüz dosyaları eşleşti. Yerel kabul imzalı paket hazırlama ve çıkarma kapsamındadır. Windows 11 x64 kurulu geçiş ve üretim HTTPS kaynağı/anahtarı açık.

### 19 Kasım 2026 — Açılış bildirimi ve tek depodan dağıtım

Açılışta güncelleyici boşta ve yapılandırılmışsa bir kez kurulu sürümün kanalı kontrol edilir. SemVer ön sürümleri deneme kanalında, kararlı sürümler kararlı kanalda aranır. İmzası geçerli yeni sürüm bulununca uygulama sürümü gösterir ve Ayarlar’ı açar. İndirme kullanıcının eylemiyle başlar.

Windows 10’da gerçek WebView ve yerel imzalı 0.1.1 manifesti kullanıldı. Bildirim Ayarlar açılmadan görüldü. Ayarlar’da indirme düğmesi hazırdı. Sunucu yalnız manifest ve imza için iki GET gördü. Normal çıkış ve girdi eşleşmesi geçti. [Native kanıt](../../work/u42/ad38ae49/result.json)

Tek açık GitHub deposu kaynak kodu ve Releases kurulum dosyalarını barındırabilir. Uygulamanın imzalı güncelleme akışı için aynı deponun GitHub Pages sitesinde sabit HTTPS adresi kullanılır. Yayın derlemesine `HEADROOM_DESKTOP_UPDATE_BASE_URL` ve açık imza anahtarı `HEADROOM_DESKTOP_UPDATE_PUBLIC_KEY` verilir. Kaynak, kanal altındaki `manifest.json`, `manifest.json.sig` ve manifestteki güncelleme paketine yönlendirmesiz erişim sağlamalıdır. Üretim anahtarı, Pages yayını ve gerçek HTTPS uçtan uca testi açık. [GitHub Pages](https://docs.github.com/en/pages/getting-started-with-github-pages/what-is-github-pages)
