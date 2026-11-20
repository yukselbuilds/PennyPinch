# Geliştirme akışı ve görev kabulü

3 Kasım 2026. Bu sözleşme `TASKS.md` içindeki görevleri ve geliştirme araçları işlerini yönetir. Ürün kapsamı spesifikasyonda, değişmez sınırlar `AGENTS.md` içindedir. Buradaki kurallar sonraki kabullere uygulanır; tarihsel kanıtın kapsamını genişletmez. Yönerge bakımı ürün görevi başlatmaz veya tamamlamaz.

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

1. `TASKS.md` ve kısa checkpoint'ten aktif kimliği, son kanıtı ve sıradaki eksik davranışı bul. Bağlamdaki güncel kaynak/belgeyi tekrar okuma; değişim, eksik bağlam veya somut belirsizlik varsa ilgili kısmı yenile.
2. Mevcut plan/spec bölümünde amaç/kapsam dışını, korunacak sözleşmeleri, kritik varsayımları ve doğrulama yollarını kontrol et; yalnız eksik/çelişen kararı güncelle. Normal/hata/sınır davranışını en fazla üç gözlenebilir senaryoyla tarif et: başlangıç → eylem → beklenen sonuç. Yeni plan dosyası her görevde zorunlu değildir.
3. Uygulayıcıyı kısa gerekçeyle seç. Kritik auth/rota/sahiplik, ölçüm veya kurtarma kararı ve belirsiz performans kök nedenini ana ajan üstlenir. Davranışı/doğrulaması net ve devir maliyetini karşılayan işleri tek Luna xhigh çalışanına ver; küçük işi sırf devir için bölme.
4. Dilimi tek davranış ve birlikte doğrulanabilir kabulle sınırla; 1–5 dosya yaklaşık devir rehberidir, zorunlu bölme eşiği değildir. Luna seçildiyse aşağıdaki kısa devri ver. Aynı alanda tek ajan yazar; ana ajan devralmadan çalışanı durdurur ve durduğunu doğrular.

```text
Kimlik / amaç:
Uygulayıcı / kısa gerekçe:
Başlangıç ve beklenen davranış:
Dosya sahipliği / kapsam dışı / korunacak sözleşmeler:
Kabul senaryoları:
Kritik varsayımlar / ilgili kaynaklar:
Mevcut doğrulama komutları / yeniden kullanılacak kanıt:
Dönüş: değişiklik, sonuç/fingerprint, bilinmeyen, sonraki adım.
```

Uygulayıcı kodlamadan etkilenen gerçek arayüz/kayıt/ölçüm ve UI → yönetici → motor/istemci sözleşmesini kontrol eder. Önce mevcut yardımcı/standart yapı kullanılır. Yeni bağımlılık, soyutlama veya ayar ancak görevin somut ihtiyacını karşılıyorsa eklenir. Kod düzeltmesi ve geniş temizlik aynı dilimde birleştirilmez.

Çalışan çelişen sözleşme, yeni kanıt üretmeyen tekrar veya kabulü zayıflatma gereğinde gözlem/denenen yaklaşım/gereken kararı ana ajana taşır ve karara bağlı yazmayı durdurur. Ana ajan tek düzeltme paketi verir; engel sürerse devralır veya kabulü koruyarak işi yeniden sınırlar. Timeout yeni çalışan veya model değişikliği gerekçesi değildir; gerçek oturum sonu/handle kaybında checkpoint'ten tek yeni Luna açılabilir. İnceleme çalışan özetine dayanmaz; ana ajan farkı ve kritik bağlı akışı okur. Kozmetik tercihler yeni tur üretmez.

## Bir görevin bitiş kabulü

Aşağıdakilerin hepsi sağlanmadan ana görev `[x]` olmaz:

1. **Davranış:** Görevin bütün kabul senaryolarında beklenen sonuç gösterilmiş; kullanıcı arayüzü değişiyorsa ilgili UI → yönetici → motor/istemci yolu incelenmiş. Mock yalnız test kapsamını kanıtlar.
2. **Hata ve kapsam:** Değişikliğin etkilediği iptal, hata, çatışma veya yeniden açılış senaryoları doğrulanmış. Değişmeyen yollar için gereksiz yeni test üretilmez. Model, auth, proje ve süreç sahipliği sınırları korunmuş.
3. **İnceleme:** Ana ajan değişikliği ve ilgili çağıranları incelemiş. Kabulü bozan somut bulgular giderilmiş. İsteğe bağlı temizlik, kanıtlı hata gibi sunulmamış.
4. **Doğrulama:** Plandaki mevcut komut; plan yoksa TASKS kabulü ve aşağıdaki yerel kapı kullanılmış, komutun varlığı doğrulanmış. Odaklı kontroller geçmiş. Ürün kodunda, inceleme düzeltmeleri bittikten sonra ana görevin son adayının tam yerel kapısı bir kez geçmiş; her ara dilimde tekrarlanmamış. Belge/skill/araç ayarı işinde ilgili kurulum, yapılandırma ve bağlantı kontrolleri yeterli; yalnız yönerge bakımında ürün testleri tekrar çalıştırılmaz. Beklenen kritik dış ortam kanıtı eksikse görev kısmi kalır.
5. **Kayıt ve durma:** TASKS/checkpoint'te durum, kanıt, sınırlar ve sonraki adım tutarlı. Ana görev tamamlandı kullanıcıya açıkça söylenir ve çalışma durur. Alt dilim bittiğinde ana görev tamamlandı denmez.

Kanıt satırı kısa tutulur: **senaryo | beklenen | gözlenen | kanıt yolu | kaynak/config + araç/ortam fingerprint'i | sınır**. Sonuç ve ilgili önce/sonra fingerprint'leri aynı doğrulama adımında programatik kaydedilir; hash elle kopyalanmaz, ölçüm altyapısı yoksa sınır yazılır. Ham prompt, auth, HTTP gövdesi veya hata dökümü kalıcılaştırılmaz.

PASS ancak çalışma alanı, ilgili kaynak/test/config, bağımlılık/araç zinciri, komut ve ortam eşleşiyorsa yeniden kullanılır. Aynı başarılı kontrolü yinelemek için önce somut değişim nedeni yazılır. Compact, yeni gün veya kredi kesintisi geçersizleşme nedeni değildir. Yeni düzeltme ilgili kanıtı geçersiz kılar; etkilenmeyen kanıt korunur. Canlı bağlantı/CI/dağıtım/migration/veritabanı/üretim kontrolü kendi aşamasında yenilenir. Kısa koşu uzun süreli kararlılık sayılmaz; performans verisi, çalışma durumu, yük, süre ve eşik önceden belirlenir. Başarısızlığı gizlemek için kabul gevşetilmez.

## Durumların anlamı

| Durum | Kural |
| --- | --- |
| Planlı | Henüz uygulama başlamadı; kutu açık. |
| Aktif | Tek ana görev veya kabul edilmekte olan geliştirme işi. |
| Kısmi / dış kanıt bekliyor | Tamamlanan yerel dilim yazılır; eksik kabul açık kalır. Bağımsız ilerleme sürer. |
| Tamamlandı | Bütün görev kapsamı ve ortak bitiş kabulü karşılandı. |

Hesap eksikliği veya ikinci işletim sisteminin yokluğu destek durumu olarak kaydedilir. `not_tested` veya bilinmiyor, çalışmıyor/uyumsuz demek değildir. Yerel başarı, sağlayıcı doğrulaması, Windows 11/macOS/Linux başarısı ve yayın adayı birbirinin yerine kullanılmaz. Denetim salt okunurdur; düzeltme ayrı sınırlı iştir ve kritik doğruluk/veri kaybı bulgusu kapanmadan bağımlı işe geçilmez. Başlamamış özellik hata sayılmaz. Her bulgu yeni ana görev olmaz: ilgili açık göreve bağlanır; bağımsız somut hata gerekiyorsa küçük düzeltme maddesi açılır.

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

H035/H036 ek kapıları: `pnpm.cmd test:a11y` gerçek App'i izole transport ile Edge/headless üzerinde denetler; browser kurulumu ayrı ortam ön koşuludur. Viewport karşılığı gerçek OS DPI kanıtı değildir. `pnpm.cmd test:compatibility` strict katalog istemcisini ve keşif fixture sözleşmesini odaklı çalıştırır; aynı testler `test:app` listesinde de vardır. Bu komutlar var olmayan `test:e2e` ile karıştırılmaz. Native ürün kabulü ayrıca yapılır.

`pnpm.cmd build:windows` mevcut pinned payload'ı tam envanterle `work/build/windows/pennypinch` içine doğrular ve current-user NSIS önizlemesini üretir. Proje içindeki sabitlenmiş Python ve motor payload'ı gerekir. Odaklı paket testleri aynı Python ile `-I -B -m unittest discover -s tests/runtime -p 'test_windows_*.py'` komutuyla çalışır. Teslim ZIP’i aynı Python ile `-I -B scripts/package-windows-delivery.py --receipt docs/proofs/windows-installer-044-notices.json` komutuyla doğrulanmış 0.44.0 makbuzuna bağlanır. `windows_bundle_real_staged_payload_starts_and_stops_without_development_layout` opt-in Rust testi gerçek staging kaynaklarını kullanır. Kurulum, kaynak eşitliği, native motor ve normal kapanış ayrıca gözlenir. Temiz Windows 10, Windows 11 ve WebView2 yokluğu ayrı kaydedilir. Testler izole istemci ayarlarını kullanır. [Windows doğrulaması](../validation/windows-install.md).

H038 kaldırma kabulü, Windows 10’da gerçek NSIS ve ayrı uygulama kimliğiyle yapıldı. Yerel kapıdan sonra ürün girdileri aynıysa testler yeniden çalıştırılmaz. Kaldırıcıyla ilgili değişiklikte açık uygulama, config kilidi/çatışması ve yerel veriyi saklama/silme yeniden gözlenir. Etkileşimsiz kaldırma veriyi saklar. Güncelleme yolu ayrı kabul edilir. [Kaldırma doğrulaması](../validation/uninstall.md).

H039 Windows release kontrolü: `build-windows.ps1`, paketleme sonrası `HEADROOM_RELEASE_EXE_CANDIDATE` değişkenini sahipli release exe’sine ayarlayıp `test_windows_release_executable.py` çalıştırır ve önceki süreç ortamını geri yükler. Test gerçek dosyanın PE/x64/PE32+/GUI subsystem=2 alanlarını okur; kaynakta nitelik metni aramak değildir. Doğrudan testte bu ortam değişkeni verilmezse kontrol açıkça skipped olur ve release kabulü sayılmaz. [Son aday kanıtı](../validation/release-candidate.md).

## Geçmiş kod denetiminin sınırı

DEV002'nin ilk raporu config Apply/Undo ve sahiplik → runtime/proxy → UI bağlantı yollarını önceliklendirir. CodeGraph ilişkileri keşfeder; raporlayacak ajan ilgili gerçek kaynağı okur. Her bulgu dosya/satır, oluşma koşulu, kullanıcı etkisi ve en küçük düzeltmeyle desteklenir. Kontrol edilmeyen alanlar yazılır. Rapor üretmek, bütün düzeltmeleri uygulamak veya ana görevleri kapatmak değildir.

Geliştirme token tasarrufu ancak aynı kapsama ait gerçek ölçüm varsa raporlanır. Ponytail benchmarkları, CodeGraph kullanım adedi ve silinen satır sayısı tasarruf yüzdesine dönüştürülmez.
