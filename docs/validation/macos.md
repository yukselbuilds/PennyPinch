# macOS doğrulaması

## H048-a — Apple Silicon kaynak ve derleme

İlk hedef GitHub'ın standart macos-15 ARM64 runner'ında Apple Silicon için gömülü frontend içeren bir Tauri ikilisi. Pencere sahipliği isteyen Win32 dosya seçicileri Windows yolunu korur. macOS'ta güvenli karşılığı olmayan ayar işlemleri desteklenmiyor sonucu verir. Motor veya istemci yetenekleri derleme sonucundan çıkarılmaz.

| Senaryo | Beklenen |
| --- | --- |
| Dahil kullanım ve hesap harcama durdurucusu doğrulanmış, elle dispatch edilmiş aday | Kilitli frontend bağımlılıklarıyla typecheck/build ve kilitli Tauri derlemesi geçer. İkili yalnız ARM64 Mach-O'dur. |
| Onay alanı false veya doğrulama eksik | İş başlamaz. Derleme veya destek durumu geçer sayılmaz. |
| Rust/macOS hedefi farklı, girdiler değişmiş ya da derleme başarısız | İş başarısız olur. Önce/sonra girdi parmak izi ve kısa derleyici tanısı saklanır. |

Workflow pinned checkout/setup-node, .node-version, pnpm 11.25.0 ve Rust 1.92.0 kullanır. 20 dakikalık tek standart job sınırı, cache ve artefakt yükleme olmadan uygulanır. Her dispatch öncesi [free-only kararına](../decisions/006-free-ci.md) göre güncel dahil kullanım ve hesap düzeyinde Actions 0 dolar durdurucusu kontrol edilir. [Workflow](../../.github/workflows/macos-build.yml).

10 Kasım 2026. İlk koşuda frontend derlendi ve Rust kontrolü Windows'a özgü motor sağlık alanında durdu. Platform koşulu düzeltildikten sonra macOS 15.8.2 ve Xcode 16.4 üzerinde kilitli Tauri derlemesi başarılı oldu. Önce/sonra girdi parmak izleri eşleşti ve lipo ikiliyi ARM64 olarak doğruladı. [İlk koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/42867872838), [başarılı koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/42868691135), [makine kanıtı](../proofs/macos-build.json).

## H048-b — İlk motor kanıtı

Standart ARM64 runner üzerinde sabitlenmiş CPython 3.13.13 ve Headroom 0.41.0 motoru kurulur. Girdi Windows payload'ında kayıtlı aynı bağımlılık kilididir. Deney proje içi özel bir dizinde motoru açar. /readyz ve /stats okunmadan önce soketin karşı ucundaki PID, bu testin başlattığı çocuk süreçle eşleşmelidir. Hazır sürüm, yerel telemetri, kapalı anonim beacon ve temiz kapanış gözlenir. Başka PID dinleyicisine HTTP iletilmez. Kurulu istemci ayarları ve sağlayıcı hesapları kullanılmaz.

10 Kasım 2026. macOS 15.8.2 ARM64 üzerinde taşınmış CPython 3.13.13 ve Headroom 0.41.0 gerçek süreçle açıldı. Kilitteki ONNX Runtime 1.31.0 native modülü yüklendi. Hazır yanıtından önce LOCAL_PEERPID ile doğrudan çocuk süreç doğrulandı. Yabancı sokete HTTP baytı gönderilmedi. Yerel telemetri açık, anonim beacon kapalı ve normal kapanış başarılıydı. Bu deneyde sağlayıcı erişilebilirlik kontrolü kapalıydı. [Koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/42870582936), [makine kanıtı](../proofs/macos-engine.json).

## H048-b — Tam paket envanteri geçti

10 Kasım 2026. scripts/macos-payload.py paket kökünden bağımsız içerik özeti üretir. Normal dosyaların içeriği ve izinleri, göreli dosya bağlantılarının metni ve dizin izinleri ölçülür. Kök dışı, mutlak veya bozuk bağlantı, dizin bağlantısı, hardlink, özel dosya, casefold isim çakışması ve sır dosyaları reddedilir. Paket girdileri mevcut kullanıcıya ait olmalı ve grup/genel yazma izni taşımamalıdır.

İlk iki native envanter koşusunda dosya sistemi sözleşme testleri geçti, paket hazırlığı izin kontrolünde durdu. Sabitlenmiş Python arşivinin 1.635 girdisi grup/genel yazma izni taşıyor. Arşivi özel umask ile açmak tek başına yeterli olmadı. Wheel kurulumu tamamlandıktan sonra kopyalanmış normal dosyaların ve dizinlerin grup/genel yazma izinleri kaldırıldı. Hazırlama sırasında sembolik bağlantılar izlenmez. Son envanterde aynı sahiplik ve izin kabulü uygulanır.

Son native koşuda 16.640 dosya ve 597.415.285 bayt ölçüldü. Motor açılışından önce ve temiz kapanıştan sonra bütün paket özeti eşleşti. Envanterin symlink sözleşme testleri macOS'ta atlanmadı. Normal motor readiness, yabancı soketin reddi, yerel telemetri ve kapalı beacon tekrar doğrulandı. [Başarılı koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/42875188912), [iki hata ve son aday kanıtı](../proofs/macos-payload.json).

Windows'ta envanter kodunun 9 testi çalıştırıldı. Casefold dosya sistemi, symlink yetkisi ve POSIX izinleri nedeniyle 5 test skipped kaldı. Windows stat ve fstat çağrılarının yeniden yazılmış dosyada farklı ctime göstermesi, dosya kimliği karşılaştırmasında platforma göre ele alındı. macOS karşılaştırması ctime dahil kalır. Tek bayt içerik değişimi ve hardlink regresyonu yerel kontrole dahildir.

## Sıradaki dikey dilim

H048-b3 geçti. Ortak sınırlı HTTP ayrıştırıcısı UnixStream üzerinde kullanılır. Kontrol ve metrik istekleri özel 0700 dizin, soket inode'u ve UID'si, gerçek LOCAL_PEERPID ve süreç yaşamı doğrulanmadan iletilmez. Bağlantı kurma bütçesi 300 ms, HTTP I/O bütçesi 1.500 ms'dir. Mevcut Windows TCP kontrol akışının doğrulama ve yanıt sözleşmesi kullanılır. Bu bağlantı diliminden sonra yönetilen motor oturumunun uygulama ve relay bağlantısı gelir.

10 Kasım 2026. macOS 15.8.2 ARM64 üzerinde 9 native test geçti, ignored yok. Yanlış PID durumunda kontrol ve metrik yollarında sıfır HTTP baytı, bağlantı öncesi/sonrası liveness kaybı, yanıt sırasında liveness kaybı, yanıt kimliğinin uyuşmaması, symlink soket ve güvenli olmayan dizin reddi doğrulandı. Eski metrik örneği UnknownBasis olarak kaldı ve private fixture alanları projection'a geçmedi. Aynı koşuda Tauri ikilisi ARM64 olarak üretildi. Windows'ta 40 odaklı test geçti, 3 opt-in test ignored. [Koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/42883847159), [önceki hatalar ve son adayın parmak izleri](../proofs/macos-unix-transport.json).

İlk derleme, doğrudan Socket → UnixStream dönüşümünde durdu. Kilitli sürümdeki OwnedFd dönüşümüyle düzeltildi. Testlerin Mac hata ayrıntısı, timeout ayarının zaten kapalı bir Unix soketinde EINVAL verdiğini gösterdi. Test düzeneği yalnız gerçek EOF veya kapalı bağlantı doğrulandığında sıfır bayt kabul eder. Canlı fixture stream'i blocking ve süre sınırlıdır. Kabul edilen kimlik veya bayt koşulu gevşetilmedi. Native testler sabitlenmiş model/sağlayıcı hesabına bağlanmaz.

## H048-b4 — Yönetilen motor oturumu geçti

11 Kasım 2026. macOS 15.8.2 ARM64 üzerinde sabitlenmiş CPython 3.13.13 ve Headroom 0.41.0, Rust'ın doğrudan çocuk süreci olarak doğrulandı. Kaynaklar derlemeye gömülen manifest özetiyle açılır; izlenmeyen overlay ve değiştirilmiş dosya reddedilir. Geçici kontrol sırrı argv, ortam veya dosyaya yazılmadan stdin üzerinden gider. Private UDS'nin gerçek PID doğrulaması HTTP öncesinde kalır. Yetkili /health ve /stats okumaları ayrıntılı metadata alır; yanlış sırda yalnız redakte edilmiş cevap gelir.

Son koşuda 18 Python kontrolü tamamlandı, 1 kontrol skipped; native symlink kontrollerinin atlanması workflow tarafından reddedilir. 9 Unix protokol ve 3 yönetilen oturum Rust testi geçti. Normal stop, tekrarlı stop, aynı state'te yeniden açılış ve sentetik cache işaret dosyasının korunması doğrulandı. Pause/resume revizyonu ve eski revizyonlu yazım çatışması geçti. Gerçek CCR retrieval veya aktif model stream'i bu dilimde sınanmadı.

Parent kaybı testinde yalnız testin kendi motoru durduruldu; sahipli ebeveyn süreç sonlandırıldıktan sonra miras alınan flock kilidi hâlâ ikinci açılışı engelledi. Motor devam ettirilince parent pipe EOF'u sonrası gerçek süreç çıkışı kqueue NOTE_EXIT ile doğrulandı; ardından state kilidi açıldı. Gerçek süreç çıkışını kernel doğrular. Model/sağlayıcı trafiği ve çıkmış parent'ın endpoint kalıntılarının otomatik temizliği bu dilimin kapsamı dışındadır.

İlk native koşu CLI'nin macOS malloc re-exec davranışında durdu. Gerçek embedding giriş adı ve Python başlamadan verilen allocator ortamı, bootstrap hook'ları ile -I/-B bayraklarını korur. Başarılı koşuda 16.640 dosya / 597.415.256 bayt payload, önce/sonra aynı CI girdi parmak izi ve ARM64 Tauri ikilisi doğrulandı. [Koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/42990619381), [makine kanıtı ve ilk hata](../proofs/macos-managed-session.json).

## H048-b5a — Sabit giriş ve Unix aktarımı geçti

11 Kasım 2026. macOS 15.8.2 ARM64 üzerinde private 0700 dizin, 0600 kayıt ve kilit, yeniden açılışta aynı loopback kimliği/portu, çatışmada kayıt ve yabancı listener'ın korunması doğrulandı. TCP giriş, istemci baytlarını iletmeden önce Unix peer PID'sini ve sahip olunan süreci doğrular. Native fixture, terminalden önce delta, half-close ve drain sırasında mevcut bağlantının tamamlanmasını doğruladı.

İlk native koşu Darwin wildcard listener çakışmasını ve bağlantı kapanışında kaybedilen sahipliğin yanlış hata olarak dönmesini buldu. Kayıtlı portta bağlama öncesi ve sonrası bounded listener gözlemi eklendi. Kontrol, metrik ve sağlık okumalarında süreç kaybı I/O hatasından önce değerlendirilir. Son koşuda 18 native protokol/ingress/relay ve 3 yönetilen motor oturumu testi geçti. Python kontrolleri 17 başarılı, 1 skipped. Önce/sonra kaynak ve 16.640 dosyalı payload parmak izleri aynı. Windows'ta 30 odaklı test geçti, 1 opt-in test ignored. [Koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/43002575695), [makine kanıtı ve ilk hata](../proofs/macos-stable-ingress.json).

## H048-b5b — Uygulama yöneticisi ve gerçek motor geçti

11 Kasım 2026. macOS 15.8.2 ARM64 üzerinde uygulamanın kullandığı RuntimeManager ve tercih deposu gerçek sabitlenmiş motorla çalıştı. Kaydedilen korumalı araç ayarı açılışta doğrulandı. Uzun kullanıcı veri yolunda private Unix soketi açıldı ve temizlendi. Sabit loopback girişten /readyz yanıtı geldi. Metrikler aynı motor neslinden okundu. Duraklatma ve devam etme revizyonu değiştirirken eski yazım ve yanlış nesle durdurma isteği reddedildi. Durdurma sonrası giriş yeni isteği motora iletmedi. Yeniden başlatma ve yöneticinin yeniden açılışı aynı portu ve kimliği korudu. Yabancı port sahibine dokunulmadı.

İlk koşu Windows'a özgü tercih kilidi nedeniyle uygulama başlangıcında durdu. macOS'a 0600 sahipli dosya kilidi ve özel geçici yazım eklendi. Son koşuda 31 protokol/aktarım/tercih ve 4 yönetilen motor/uygulama testi geçti. Python kontrolleri 17 başarılı, 1 skipped. ARM64 Tauri derlemesi, 16.640 dosyalı payload ve aynı önce/sonra girdi parmak izi doğrulandı. Windows odaklı 69 test geçti, 5 opt-in ignored. [Koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/43009523997), [makine kanıtı](../proofs/macos-application-manager.json).

Bu koşu yerel readiness ve metrik trafiği kullandı. Canlı sağlayıcı trafiği ile tasarruf ölçümü yapılmadı.

## H048-b6a — APFS config yayını deneyi

H048-b6'nın ilk adımı APFS dosya işlemlerini standart Apple Silicon runner'da sınar. Uygulamanın mevcut plan/Apply/Undo sözleşmesi [005 kararında](../decisions/005-config-transaction.md) tanımlıdır. Apple, `RENAME_EXCL` için hedef varsa çakışma ve `RENAME_SWAP` için iki adın atomik takasını tanımlar. `flock` işbirlikli kilittir. [Apple rename kılavuzu](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/man/man2/rename.2) · [Apple flock kılavuzu](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/flock.2.html).

| Başlangıç ve eylem | Beklenen gözlem |
| --- | --- |
| Mevcut dosya ve parent descriptor açıkken ayrı süreç kilit, yazma, replace ve unlink dener | İkinci işbirlikli kilit reddedilir. Normal dosya işlemlerinin gerçek sonucu ve dosya kimliği doğrulanır. |
| Hedef boşken ve doluyken exclusive yayın, son okumadan sonra bağımsız replace ve swap | Boş hedefe tam dosya yayınlanır. Dolu hedef korunur. Swap sonrası taşınan gerçek dosya kimliği ve baytlarıyla yarış saptanır. Ek yazma yokken geri swap, yarışan dosyayı korur. |
| Config swap tamamlanınca çocuk süreç ayrı sahiplik yazımından önce sonlanır | Yeni config, taşınmış eski config ve eski sahiplik kaydı gözlenir. Sonuç süreç kaybı sınırını ölçer. |

Deney yalnız özel proje dizinindeki sabit sentetik dosyaları kullanır. Windows NTFS kabulü ayrı kalır. Uygulama düzeyinde kesinti kurtarma, istemci ayar yazımı ve undo H048-b6b kabulünde açık. [Manuel workflow](../../.github/workflows/macos-config-transaction.yml).

## Açık kabuller

- H048-b için platforma ve mimariye özel sabitlenmiş motor ve doğrulanabilir süreç/loopback sahipliği, atomik ayar işlemleri ve çatışmayı koruyan geri alma gerekir.
- H048-c için paketlenmiş uygulamanın gerçek açılışı, başlat/durdur ve temiz state kabulü ile paket ve motor imzasının ayrı doğrulanması gerekir. Apple Silicon sonucu Intel, Windows 11 veya Linux'a aktarılmaz. Üretim yayını veya ücretli imzalama bu koşuların kapsamı dışındadır.
