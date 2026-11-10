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

## H042-c1 — İmzalı uygulama paketi

10 Kasım 2026. Ana ajan paket biçimi, çalıştırılan dosyanın güveni ve geçici alan sahipliğini üstlenir. Mevcut tam NSIS kurucusu temiz kurulum ve onarım yolunu korur. İlk güncelleme protokolü aynı motor pini, aynı kurulum kaynakları ve aynı yerel veri biçimiyle yalnız gömülü arayüzü taşıyan uygulama exe’sini hazırlar.

İmzalı `schema_version: 2` manifest, `application` alanında `state_format: 1`, uygulamanın tam bayt sayısı ve SHA-256 değerini taşır. Boyut sınırı 128 MiB’dir. Paket adı `Headroom_<sürüm>_x64-update.exe` olur. v1 manifestleri H042-a/b’nin indirme sözleşmesini sürdürür. Kurulu uygulamayı değiştirme yetkisi için v2 hazırlanmış uygulama gerekir. Uyumsuz veri biçimi paket indirilmeden reddedilir.

`src-tauri/windows-update.nsi` yalnız `/HEADROOM-EXTRACT` komutuyla çalışır. Tam kurucu yolu ve registry işlemleri ayrı kalır. Paket doğrulandıktan sonra, uygulama veri kökünde sabitlenmiş ayrı bir dizinde yalnız okumayı paylaşan açık handle ile çalıştırılır. NSIS dosyayı geçici dizinde çıkarır ve hedefte aynı ad varsa hata verir. Çıkan dosyanın boyutu ve SHA-256 değeri imzalı manifestle eşleşirse bir oturumluk, kapanınca silinen dosyaya aktarılır. [NSIS taşıma sözleşmesi](https://nsis.sourceforge.io/Reference/Rename).

Yerel kabul için gerçek NSIS 3.13 paketi ve daha önce doğrulanmış QA exe’si kullanılır. İmzalı normal paket, aynı boyutta yanlış uygulama özeti, uyumsuz veri biçimi, komutsuz çalıştırma ve dolu hedef adı sınanır. Normal hazırlama ve bozuk çıktı reddinde geçici dosyaların kaldırıldığı gözlenir. Kullanıcının gerçek kurulum dosyaları ve veri kökü ayrı kalır.

Sonraki dilim kurulu Windows uygulamasının sahipliğini doğrular. Motor ve bağlantı işlemleri güvenle kapandıktan sonra özel `.desktop-use` kilidini alır. Aynı diskteki tek uygulama dosyası atomik değiştirilir. Önceki çalışır exe ve kurulum sürüm bilgisi, mevcut kullanıcı verisine dokunmadan geri dönüşe bağlanır. Süreç kesintisi, disk dolu ve açılış uyumsuzluğu ayrı hata senaryolarıdır. İki masaüstü Windows kabulü açık kalır.

## H042-c2 — Kurulu uygulama geçişi ve geri dönüş

Ana ajan süreç, dosya ve kalıcı kurtarma kaydını üstlenir. Uygulama yalnız kendi HKCU NSIS kaydı, geçerli kurulum dizini ve o dizinden çalışan exe ile kurulum eylemi sunar. Geliştirme derlemesinde bu eylem kapalıdır. İlk geçiş aynı kaynak paketini ve veri biçimi 1’i taşıyan uygulama sürümleri içindir.

Kullanıcı Ayarlar’da sürümü seçip yeniden başlatmayı onaylar. Motor yöneticisi durmuş ve temiz durumu bir mutex altında tekrar kontrol eder. Aynı mutex motorun yeniden başlamasını engeller. Devam eden ayar veya dışa aktarma işlemi varsa çıkış kabul edilmez. Önceki exe, yeni exe, imzalı manifest ve içeriksiz kurtarma kaydı uygulamanın `update-recovery` kökünde hazırlanır. Uygulamanın normal kapanışı başarılı olunca önceki exe’nin bir kopyası kurtarma süreci olarak başlar.

Kurtarma süreci yalnız kendi UUID dizinini ve uygulamaya ait `.desktop-use` kilidini kabul eder. İmzalı manifest ve dosya özetleri tekrar doğrulanır. Yeni exe aynı kurulum dizininde hazırlanır, kimlik/sürüm/veri biçimi için kısa bir alt süreç kontrolünden geçer ve `MoveFileExW` ile tek dosya olarak yayımlanır. HKCU sürümü güncellenir ve uygulama tekrar açılır. Erken açılış hatasında önceki exe’ye dönülür. Sonraki açılışta Ayarlar, kayıtlı önceki sürüme dönüş sunar. [Windows dosya geçişi](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw), [paylaşım kilidi](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew).

Normal kabul, iki release derlemesiyle ayrı bir NSIS uygulama kimliğinde kur→güncelle→aç→geri dön→aç akışıdır. Tercihler ve örnek kullanıcı dosyası iki geçişte aynı özetleri korumalıdır. Çalışan motor kurulum isteğini reddetmeli, iptal eylemi çıkışı başlatmamalıdır. Odaklı dosya testleri yarım hazırlığı, değişen dosyayı ve kilitli hedefi kapsar. Disk dolu, geçiş sırasında süreç sonlandırma, açılış hatası ve Windows 11 x64 ayrı kabul senaryoları olarak kaydedilir.

### H042-c2 hata kabulü

Kalan yerel kabulde ana ajan imza, kayıt ve süreç sahipliğini üstlenir. Disk dolu, yalnız test derlemesinde gerçek dosyaya kısmi yazmadan sonra Windows `ERROR_DISK_FULL` hatası verilerek sınanır. Önceki exe aynı kalmalı ve yarım dosya kaldırılmalıdır. Yayımlama API’sinden hemen önce ve hemen sonra ayrı sahipli test süreci sonlandırılır. Hedefte tam önceki veya tam yeni dosya bulunmalı ve yedekten geri dönüş mümkün olmalıdır. Bu deney API içindeki ani güç kaybını kapsamaz.

Erken açılış testinde gerçek bir Windows exe’si kimlik kontrolünü geçip normal açılışta hata koduyla çıkar. İmzalı paket ve ayrı HKCU test kaydı üzerinden üretimin uygulama/geri dönüş yolu çağrılır. Önceki exe, kayıtlı sürüm ve örnek kullanıcı dosyası korunmalıdır.

Geçmiş bakımı yalnız sabitlenmiş UUID dizinlerindeki imzalı kayıtları ve doğrulanan dosyaları temizler. Güncel exe’ye dönüş sağlayan kayıt saklanır. Aynı yedeğe ait tekrarlar bir kayda indirilebilir. Bilinmeyen dosya, yanlış imza veya kilitli kayıt korunur. 32 sınırı, temizlenemeyen kayıtlarla birlikte hâlâ doluysa yeni hazırlama güvenle hata verir. Windows 11 ve gerçek yayın kaynağı kendi kabulünde kalır.

## H042-d — Yerel yayın hazırlığı

Kullanıcı 10 Kasım’da ayrı public depo yerine şimdilik yerel hazırlığı seçti. Ana ajan paket/imza güvenini üstlenir. `scripts/prepare-windows-update.py`, açıkça verilen release exe, uygulama config’i, anahtar dosyaları ve notlardan tek kanal dizini hazırlar. Public depo, ağ yüklemesi, gerçek yayın anahtarı üretimi veya CI çalıştırma kapsam dışıdır. Mevcut kurulum ve hesap ayarlarına dokunulmaz.

Sürüm/kimlik/veri biçimi exe’nin güncelleme probe’uyla, açık anahtar exe içindeki değerle karşılaştırılır. Aynı NSIS şablonu uygulamayı paketler. Manifestin imzası, motor pini, kanal, SemVer ve paket/uygulama özetleri üretimdeki Rust doğrulayıcıyla kontrol edilir. Paket açıldıktan sonra çıkan exe’nin özeti ve probe’u tekrar geçmelidir. Kaynaklar ve araçlar önce/sonra fingerprint’leriyle bağlanır. Özel anahtar/parola çıktıya veya makbuza kopyalanmaz; parola yalnız imzalayan alt sürece aktarılır.

Kabul senaryoları:

1. Yerel, geçici anahtarlı x64 fixture → hazırlama → kanalda yalnız manifest, imza ve paket; yanında içeriksiz doğrulama makbuzu. Bu, üretim release exe’si veya gerçek HTTPS teslimi kanıtı değildir.
2. Yanlış kanal/anahtar/sürüm, geçersiz imza veya farklı çıkan exe → hata → hazır çıktı yok.
3. Mevcut çıktı, araç hatası veya hazırlık sırasında değişen girdi → reddet → önceki çıktı ve özgün girdiler korunur, sahipli yarım dizin kaldırılır.

Odaklı kapı pinned Python ile `test_windows_update_package.py`, normal güncelleme Rust testleri ve gerçek NSIS/Tauri signer/Rust doğrulayıcı koşusudur. Ürün çalışma zamanı/arayüzü değişmez. Windows 11 x64 ve üretim kaynağı/anahtarı ana H042’de açık kalır.
