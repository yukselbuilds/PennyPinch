# H042-a — Güncelleme manifesti ve güven sınırı

10 Kasım 2026. Kullanıcı H041 ve H037’nin dış doğrulamalarını açık bırakarak H042-a’nın yerel dilimine geçişi onayladı. Ana ajan imza, kaynak ve sürüm güvenini üstlenir. H042-b ve H042-c ayrı dilimlerdir.

Windows x64 için bir uygulama güncellemesi `manifest.json`, bu dosyanın ayrık Tauri Minisign imzası ve mevcut NSIS biçimindeki kurucu dosyasıyla tanımlanır. İmza manifestin ham baytlarına aittir. Manifest, uygulama kimliğini, hedefi, kanalı, SemVer sürümünü, düz metin sürüm notunu, kurucunun tam boyutunu ve SHA-256 değerini birbirine bağlar. Tauri anahtar ve imza dosyalarının Base64 biçimi kullanılır. Doğrulama için kanonik Ed25519 imzası denetimi ekleyen bağımlılıksız `minisign-verify` 0.3.0 sabitlenir. [Tauri imzalama sözleşmesi](https://v2.tauri.app/plugin/updater/#signing-updates), [Tauri’nin 0.3 geçiş kaydı](https://v2.tauri.app/release/updater/v2.13.3/).

Güvenilen açık anahtar ve kurulu uygulamanın kimliği/sürümü çalıştıran derlemeden gelir. İndirilen manifest bu değerleri seçemez. İmza boyutu sınırlanır, imza JSON okunmadan doğrulanır ve v1 alanları kapalı şemayla ayrıştırılır. Kararlı kanal düz SemVer, deneme kanalı ön sürüm kabul eder. Aday kurulu sürümden kesinlikle yeni olmalıdır. Yanlış kimlik, hedef, kanal ve tekrar/eşit/eski sürüm reddedilir. Yerel son çalışan sürüme dönüş H042-c’nin ayrı yolu olacaktır.

İmzalı metadata, kurucunun taşıması gereken motor sürümünü ve payload manifestinin SHA-256 değerini içerir. Doğrulayıcı bunları runtime yöneticisinin mevcut sabitleriyle karşılaştırır. İlk güncelleme sözleşmesi bu motor pinini koruyan uygulama paketleri içindir. Kurucu indirme adı doğrulanmış uygulama sürümünden türetilir. Manifest şeması key, URL veya ayrı motor kurulum komutu taşımaz. Dağıtım adresi ve indirme sahipliği H042-b’de ayrıca bağlanır.

Kabul sırası:

1. Gerçek Tauri CLI ile imzalanan kararlı ve deneme fixture’ı kendi kanalında kabul edilir. Beklenen uygulama ve motor kimliği ile imzalı boyut/SHA eşleşir.
2. Yanlış anahtar, oynanmış veya bozuk imza, geçerli imzalı ama hatalı şema/kimlik/kanal/sürüm ve motor pini doğrulanmış aday üretmez.
3. Kurucu akışında eksik, fazla, okunamayan veya SHA değeri farklı içerik reddedilir. Akış imzalı boyutun en fazla bir bayt fazlasını okur. H042-b doğrulanan dosyanın sahipliğini ve çalıştırma öncesi kimliğini koruyacaktır.

Sınırlar koşu öncesi sabitlenir. Manifest en fazla 32 KiB, imza metni 4 KiB, sürüm notu 4 KiB ve kurucu 1 GiB olabilir. Şimdiki imzalı örnekler yalnız fixture anahtarına ve sentetik kurucu baytlarına aittir. Üretim güven anahtarı, indirme ve kurulum eylemi sonraki dilimlerin kapsamındadır. Odaklı doğrulama `cargo test --offline --manifest-path src-tauri/Cargo.toml desktop::update::tests -- --test-threads=1`.

## H042-b — Güncelleme ekranı ve indirme

Kullanıcı H042-a sonrasında devamı onayladı. Ana ajan kaynak güveni, dosya sahipliği ve motor durumuna bağlı bekleme davranışını üstlenir. Bu dilim kullanıcının seçtiği kanalı kontrol eder, doğrulanmış sürüm/not/boyutu gösterir ve imzalı paketi bu oturum için hazırlar. Kurucuyu çalıştırma, kalıcı geçiş kaydı ve geri dönüş H042-c’de uygulanır.

Yayın tabanı ve açık anahtar Windows x64 derlemesine birlikte verilir. URL kullanıcı ayarından veya manifestten alınmaz. Kaynak doğrudan HTTPS sunar. Otomatik yönlendirme, kullanıcı kimliği ve otomatik kontrol yoktur. Her kanal kendi dizininden `manifest.json`, `manifest.json.sig` ve doğrulanmış sürümden türetilen kurucuyu alır. Üretim kaynağı tanımlanmayan derlemede durum açıkça kullanılamıyor görünür. Yerel fixture yalnız açıkça seçilen debug özelliğinde, `127.0.0.1` ve ayrı fixture anahtarıyla etkinleşir.

Doğrulanmış adayın kimliği oturuma özeldir. Yeni kanal kontrolü eski adayı bırakır. İndirme, sabitlenen uygulama veri kökünde yazma ve silmeyi paylaşmayan geçici bir Windows dosyasına imzalı boyut artı en fazla bir bayt yazar. Boyut ve hash aynı açık dosya üzerinden doğrulanır. Hata ve uygulamadan çıkışta dosya handle kapanışıyla temizlenir. Arayüz yerel dosya yolu, imza veya anahtar almaz. H042-c’de yayımlama/çalıştırma anındaki dosya ve süreç sahipliği ayrıca doğrulanır.

Motorun aktif stream sayısı bilinmiyor. Bu yüzden indirme sonrası durum yalnız sahipli motorun durduğu doğrulanırsa hazır görünür. Çalışan, geçişteki veya durumu belirsiz motorda bekler. Bu gözlem kurulum izni olarak kullanılmaz. Motoru durdurma veya trafik kesme eylemi bu kontrolden çıkmaz.

Kabul, geçerli yerel fixture’la kontrol→metadata→indirme→doğrulama, geçersiz imza/yanlış hash/yarım indirme/yönlendirme reddi, beklemede motor durumunun korunması ve TR/EN ayarlar akışını kapsar. Üretim imzası ve gerçek güncelleme/geri dönüş ayrı kanıttır.
