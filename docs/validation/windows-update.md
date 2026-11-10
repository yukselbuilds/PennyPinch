# H042-a — Yerel güncelleme güveni

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
