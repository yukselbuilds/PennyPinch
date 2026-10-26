# Geliştirme akışı ve görev kabulü

26 Ekim 2026. Bu sözleşme `TASKS.md` içindeki görevleri ve geliştirme araçları işlerini yönetir. Ürün kapsamı spesifikasyonda, değişmez sınırlar `AGENTS.md` içindedir. Buradaki kurallar sonraki kabullere uygulanır; tarihsel kanıtın kapsamını genişletmez.

## Araç seçimi

| İhtiyaç | Kullanım |
| --- | --- |
| Bilinen dosya veya sembol | Önce `rg` ve gereken satırlar. |
| Çağıranlar, bağımlılıklar, değişiklik etkisi | CodeGraph; tek soru, başlangıçta `maxFiles: 1`, gerekirse 2–3. Sonucu kaynakla doğrula. |
| Kod yazma veya düzeltme | Projedeki `ponytail`, `full` seviyesi. Mevcut yapıyı ve gerekli kontrolleri koruyan en küçük tamamlanmış değişiklik. |
| Değişiklik kabulü | Ana ajan `ponytail-review` ile farkı, ilgili çağıranları ve testleri tek incelemede değerlendirir. Mevcut kalite incelemesiyle birleştirir. |
| Geçmiş kodu değerlendirme | `ponytail-audit`; belirlenmiş kapsamda bir defalık rapor. Her görevde bütün depo yeniden taranmaz. |
| Sıkıştırılmış veya eksik araç çıktısı | İnceleme için gereken özgün bölümü `headroom_retrieve` ile getir. Özet veya CCR işareti, okunmuş kaynak ya da PASS değildir. |

CodeGraph'ın indeksi kaynak veya dışlama ayarı değiştiğinde güncellenir; aynı anda tek güncelleyen olur. MCP araç listesinde görünmüyorsa `scripts/codegraph.ps1` kullanılır. Graf keşif sağlar; Tauri IPC, gerçek sağlayıcı trafiği ve ajan kimliği ayrıca kanıtlanır. [Kurulum ve komutlar](codegraph.md).

Ponytail kuralları ana ajan ve mevcut tek çalışan için geçerlidir. Rapor dili Türkçe, varsayılan seviye `full`dür. Skill yalnız talimat sağlar; CI veya test çalıştırıcısı değildir. Güvenlik, sahiplik, atomik yazım, çatışmayı koruyan geri alma, hata yönetimi ve gerekli testler sadeleştirme uğruna kaldırılmaz. [Sabitlenen kaynak ve kullanım](ponytail.md).

## Göreve giriş

1. `TASKS.md` ve kısa checkpoint'ten aktif kimliği, son kanıtı ve sıradaki eksik davranışı bul. İlgili kabul koşulu için gerekmedikçe tüm spec/geçmiş çıktıları yeniden yükleme.
2. Başlamadan sonucu en fazla üç gözlenebilir senaryoyla tarif et: başlangıç durumu → eylem → beklenen sonuç. Etkilenen dosyaları ve odaklı doğrulamayı seç. Ana görev büyükse mevcut kimlik altında 1–5 kaynak dosyalık dilimlere ayır.
3. Çalışana aşağıdaki kısa devir ver. Ana ajan aynı uygulamayı paralel yazmaz; bağımsız inceleme ve belge işini yürütür.

```text
Kimlik / amaç:
Başlangıç ve beklenen davranış:
İzinli dosyalar / korunacak sınırlar:
Kabul senaryoları:
Odaklı kontrol ve yeniden kullanılacak kanıt:
Dönüş: değişiklik, sonuç/fingerprint, bilinmeyen, sonraki adım.
```

Uygulamada önce mevcut yardımcı/standart yapı kullanılır. Yeni bağımlılık, soyutlama veya ayar ancak görevin somut ihtiyacını karşılıyorsa eklenir. Kod düzeltmesi ve geniş temizlik aynı dilimde birleştirilmez.

## Bir görevin bitiş kabulü

Aşağıdakilerin hepsi sağlanmadan ana görev `[x]` olmaz:

1. **Davranış:** Görevin bütün kabul senaryolarında beklenen sonuç gösterilmiş; kullanıcı arayüzü değişiyorsa ilgili UI → yönetici → motor/istemci yolu incelenmiş. Mock yalnız test kapsamını kanıtlar.
2. **Hata ve kapsam:** Değişikliğin etkilediği iptal, hata, çatışma veya yeniden açılış senaryoları doğrulanmış. Değişmeyen yollar için gereksiz yeni test üretilmez. Model, auth, proje ve süreç sahipliği sınırları korunmuş.
3. **İnceleme:** Ana ajan değişikliği ve ilgili çağıranları incelemiş. Kabulü bozan somut bulgular giderilmiş. İsteğe bağlı temizlik, kanıtlı hata gibi sunulmamış.
4. **Doğrulama:** Odaklı kontroller geçmiş. Ürün kodunda, inceleme düzeltmeleri bittikten sonra son adayın tam yerel kapısı bir kez geçmiş. Belge/skill/araç ayarı işinde bunun yerine ilgili kurulum, yapılandırma ve bağlantı kontrolleri yeterli. Beklenen kritik dış ortam kanıtı eksikse görev kısmi kalır.
5. **Kayıt ve durma:** TASKS/checkpoint'te durum, kanıt, sınırlar ve sonraki adım tutarlı. Ana görev tamamlandı kullanıcıya açıkça söylenir ve çalışma durur. Alt dilim bittiğinde ana görev tamamlandı denmez.

Kanıt satırı kısa tutulur: **senaryo | beklenen | gözlenen | kanıt yolu | kaynak/config + araç/ortam fingerprint'i | sınır**. Ham prompt, auth, HTTP gövdesi veya hata dökümü kalıcılaştırılmaz.

PASS ancak ilgili dosya/config, bağımlılık/araç zinciri, komut ve ortam eşleşiyorsa yeniden kullanılır. Aynı başarılı kontrolü yinelemek için önce somut değişim nedeni yazılır. Compact, yeni gün veya kredi kesintisi geçersizleşme nedeni değildir. Yeni düzeltme ilgili kanıtı geçersiz kılar; etkilenmeyen kanıt korunur.

## Durumların anlamı

| Durum | Kural |
| --- | --- |
| Planlı | Henüz uygulama başlamadı; kutu açık. |
| Aktif | Tek ana görev veya kabul edilmekte olan geliştirme işi. |
| Kısmi / dış kanıt bekliyor | Tamamlanan yerel dilim yazılır; eksik kabul açık kalır. Bağımsız ilerleme sürer. |
| Tamamlandı | Bütün görev kapsamı ve ortak bitiş kabulü karşılandı. |

Hesap eksikliği veya ikinci işletim sisteminin yokluğu destek durumu olarak kaydedilir. Yerel başarı, sağlayıcı doğrulaması, Windows 11/macOS/Linux başarısı ve yayın adayı birbirinin yerine kullanılmaz. Denetimde bulunan her konu yeni ana görev olmaz: ilgili açık göreve bağlanır; bağımsız somut hata gerekiyorsa küçük düzeltme maddesi açılır.

## Bugün çalışabilen yerel ürün kapısı

Bu komutlar mevcut `package.json` ve son H021 kanıtıyla eşleşir. Yeni kod kabulünde son aday üzerinde, repo kökünden çalıştırılır:

```text
pnpm.cmd test:app
pnpm.cmd build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --offline --manifest-path src-tauri/Cargo.toml -- --test-threads=1
cargo build --offline --manifest-path src-tauri/Cargo.toml --features custom-protocol
```

Cargo çağrılarında süreç ortamı `CARGO_HOME=<repo>/work/caches/cargo-home` ve `CARGO_TARGET_DIR=<repo>/work/caches/cargo-target` olur. Kaydedilmiş araç zinciri ve mevcut offline cache kullanılır. `build` TypeScript kontrolünü de içerdiği için aynı adayda ayrıca aynı typecheck tekrarlanmaz.

Şu anda `lint`, `test`, `test:e2e` adlı pnpm scriptleri yoktur. Planlanan bir komut başarılı kontrol diye işaretlenmez; eklendiği görevde bu tablo güncellenir. UI davranışı gereken görevlerde ilgili native kullanıcı akışı ayrıca doğrulanır. Bu kapı debug embedded ürünü doğrular; H038/H039 installer, güncelleme ve yayın kabulünün yerine geçmez. CI, push ve yayın kendi aşamasında ele alınır; mevcut ücretsiz kullanım sınırı geçerlidir.

## Geçmiş kod denetiminin sınırı

DEV002'nin ilk raporu config Apply/Undo ve sahiplik → runtime/proxy → UI bağlantı yollarını önceliklendirir. CodeGraph ilişkileri keşfeder; raporlayacak ajan ilgili gerçek kaynağı okur. Her bulgu dosya/satır, oluşma koşulu, kullanıcı etkisi ve en küçük düzeltmeyle desteklenir. Kontrol edilmeyen alanlar yazılır. Rapor üretmek, bütün düzeltmeleri uygulamak veya ana görevleri kapatmak değildir.

Geliştirme token tasarrufu ancak aynı kapsama ait gerçek ölçüm varsa raporlanır. Ponytail benchmarkları, CodeGraph kullanım adedi ve silinen satır sayısı tasarruf yüzdesine dönüştürülmez.
