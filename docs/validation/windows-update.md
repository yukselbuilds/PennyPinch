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

H042 ve c2 açık kalır. Disk dolu, dosya yayımlanırken kesinti, erken açılış hatasında otomatik dönüş, kurtarma kaydı sınırında bakım ve Windows 11 x64 kabulü sürmektedir. Üretim yayın kaynağı ve açık anahtarı da yapılandırılmalıdır. Gerçek sağlayıcı isteği yapılmadı.

Kabulden sonra iki QA kurulumu kendi kaldırıcılarıyla kaldırıldı. İki uygulama kaydı ve yalnız bu testlere ait LocalAppData kökleri temizlendi. [Temizlik kaydı](../../work/proofs/update/cleanup-installed.json).
