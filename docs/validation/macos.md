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

11 Kasım 2026. macOS 15.8.2 ARM64 üzerinde yerel APFS ve 0700 fixture dizini doğrulandı. Ayrı süreç, ikinci `flock` alamazken tuttuğumuz hedefe normal yazma, replace ve unlink yapabildi. Exclusive rename dolu hedefte `EEXIST` ile iki dosyayı korudu. Swap, son kontrolden sonra değişen dosyayı eski adından taşırken kimliği ve baytları korudu. Ayrı süreç takastan önce açtığı dosya tanıtıcısından taşınmış sürüme yazabildi. Bu düzenleme kontrollü geri swap ile hedefte korundu. Çocuk süreç config takasından sonra öldüğünde config yeni, taşınmış config ve sahiplik kaydı eski kaldı. Önce/sonra girdileri eşleşti. İlk koşudaki derleme hatası açık macOS SDK yolu ile düzeltildi. [Başarılı koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/43013886985) · [Makine kanıtı](../proofs/macos-config-transaction-probe.json).

Bu gözlemlerle `flock` ve rename kullanarak mevcut istemci dosyasını yazan üretim yolu açılmaz. H048-b6b için kesinti sonrası sahiplik ve hedef durumunu sırsız kimlik ve fingerprint ile doğrulayan kurtarma modeli gerekir. Eski tanıtıcı üzerinden yazılan düzenlemenin de korunabildiği ölçülür. Kanıt eksikse ilgili istemci yazımı kapalı kalır. Deney yalnız sentetik dosyalarda süreç kaybını ölçtü. Güç kesintisi, gerçek istemci ayar yazımı ve undo açık. [Manuel workflow](../../.github/workflows/macos-config-transaction.yml).

## H048-b6b — APFS kurtarma deneyi geçti

11 Kasım 2026. Yerel APFS fixture'ında uygulamanın Codex ayar yaması ve sahiplik kaydı kullanıldı. Bekleyen işlem kaydı iki dosya sürümünün kimliklerini ve özetlerini tutar. Ayrı süreç config takasını tamamladıktan sonra sahiplik yayını öncesinde çıktı. Yeniden açılan kurtarma aynı sürümleri ve hazırlanan sahiplik kaydını doğrulayıp yalnız sahiplik kaydını yayımladı. Yeniden deneme aynı sonucu verdi.

Eski tanıtıcıdan kurtarma öncesi veya sonrası gelen yazım iki sürüm korunarak çatışma verdi. Aynı baytlarla yeniden oluşturulan dosya da sahiplik kanıtı sayılmadı. Gerçek Undo yaması sonradan eklenen ayarı korudu ve kesintiden sonra bırakılmış sahiplik kaydı yayımlandı. macOS 15.8.2 ARM64 üzerinde dört test ve kilitli Tauri derlemesi geçti. Doğrudan çalıştırılması atlanan çocuk testini ilgili testler ayrı süreçte çağırdı. CI girdilerinin önce ve sonra parmak izleri eşleşti. [Koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/43020691783) · [Makine kanıtı](../proofs/macos-config-recovery.json).

Kurtarma prototipi native testlere dahil edildi. Taşınmış dosya ve bekleyen kayıt test boyunca saklandı. Gerçek istemciye Apply/Undo bağlanmadan önce bu dosyaya geç gelen yazımı koruyan çözüm ve uygulama komutlarıyla native doğrulama açık.

## H048-b6b — Gerçek okuyucu ve önizleme kimliği geçti

11 Kasım 2026. Uygulamanın kullandığı config okuyucusu ve app-data sahiplik factory'si özel native fixture'larda çalıştırıldı. macOS HOME ve kayıpsız POSIX yol bağlaması kullanılır. Yerel APFS, UID, dosya türü, izin, link sayısı ve açık dizin/dosya kimliği kontrol edilir. Keşif uygulama veri dizinini oluşturmaz. Kullanıcının gerçek istemci ayarları ve hesapları bu koşuda kullanılmadı.

| Başlangıç ve eylem | Beklenen ve gözlenen |
| --- | --- |
| Kullanıcıya ait 0755 dizin ve 0644 Codex config okunur; olmayan config keşfedilir | Baytlar ve izinler korunur. Eksik klasör/dosya oluşturulmaz. Gerçek app-data factory verilen kökü kullanır. |
| Önizlemeden sonra aynı baytlarla inode değişir, dosya yerinde düzenlenir, parent değiştirilir veya eksik dosya oluşturulur | Tutulan önizleme kimliği geçersiz olur. Eski görünüm geçerli sayılmaz. |
| Symlink, ara dizin symlink'i, hardlink, FIFO veya grup/genel yazılabilir hedef okunur | Güvensiz yol reddedilir; FIFO beklemeye yol açmaz. OpenCode'un çift config'i belirsiz kalır ve Claude iç içe env baytları değiştirilmez. |

İlk iki native koşuda beş okuyucu/factory testi UnsafePath ile durdu. Yalnız dizin derinliği, UID, izin ve errno kullanan tanı EINVAL 22 gösterdi. Apple XNU, O_NOFOLLOW ile O_NOFOLLOW_ANY birlikte verildiğinde EINVAL döndürür. Yalnız bütün yolda symlink takibini engelleyen O_NOFOLLOW_ANY bırakıldı; ara dizin symlink reddi native testle korundu. [Apple çekirdek kaynağı](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/vfs/vfs_vnops.c).

Son macOS 15.8.2 ARM64 koşusunda yedi gerçek okuyucu/factory testi, bir yol codec testi ve dört mevcut kurtarma testi geçti. Bir çocuk süreç yardımcısı doğrudan ignored kaldı ve kurtarma testlerinde ayrı süreçte çağrıldı. Frontend, kilitli Tauri derlemesi ve mimari kontrolü geçti. CI girdileri önce/sonra aynı ve 264 kaynak girdisi adayla eşleşti. Windows'taki 91 odaklı testin sonrasındaki farklar yalnız macOS modülündedir; diğer fingerprint'ler ve araç zinciri eşleştiği için Windows testleri yinelenmedi. Son aday fmt ve actionlint kontrolünü geçti. [Koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/43025755374) · [İki başarısız koşu ve son adayın makine kanıtı](../proofs/macos-config-reader.json).

Bu kabul üretim ayarı okuma ve önizleme kimliğini kapsar. macOS FileTransaction ve gerçek Apply/Undo akışı henüz açılmadı. Kurulu istemci keşfi, sağlayıcı hesabı, gerçek paketli pencere yaşam döngüsü veya Intel macOS desteği bu testlerden çıkarılmaz.

## H048-b6b — Ortak manager ile config yayını geçti

11 Kasım 2026. Ortak FileTransaction ve OwnershipStore macOS'a bağlandı. Ana ajan üretim kodu ve gerçek PlanManager kabulünü üstlendi. Aynı tek Luna xhigh store/transaction testlerini hazırladı; ana ajan çalışanın durduğunu doğrulayıp metadata ve yayın yarışı kontrollerini tamamladı. İlk yazım kapsamı mevcut, kullanıcıya ait APFS dizini içindeki Codex/Claude/OpenCode config dosyasıdır. Windows TxF yolu korunur. Eksik Claude proje dizinini oluşturma ayrı kabul olarak açık kalır. Model/auth/reasoning/ajan ayarları yamanın kapsamına girmez.

Config ile sahiplik dosyası ayrı atomik rename adımlarıdır; çok dosyalı katı CAS iddia edilmez. Özel uygulama dizinindeki sırsız intent, gerekli config yolunu kayıpsız bayt metadata'sı olarak, parent/dosya kimliklerini ve içerik özetlerini tutar. Config içeriği yalnız kendi istemci dizinindeki eşsiz staging dosyasında kalır. Taşınan eski inode otomatik silinmez; geç yazım korunur ve sonraki aynı-hedef işleminde conflict verir. Geçmiş 64 intent ile sınırlıdır; sınırda sessiz veri silmek yerine yeni yazım reddedilir. Keşif kurtarma yazımı başlatmaz. Açık kullanıcı eylemi yarım kalmış config yayınının yalnız sahiplik adımını tamamlayabilir; henüz yayımlanmamış eski plan yeniden çalıştırılmaz.

| Başlangıç ve eylem | Beklenen ve gözlenen |
| --- | --- |
| Üç adapterin özel dosyaları gerçek PlanManager'da Apply → yeniden açılış → snapshot → Undo yolundan geçer | Yalnız sahipli alan değişir; özgün baytlar geri gelir. Ayrı testte ilgisiz düzenleme, 0644 izin, ACL ve xattr örnekleri korunur. Uygulama state'indeki dosyalarda sentetik auth/model/prompt içeriği yoktur. |
| Commit öncesi hata/rollback ve seal sonrası süreç kaybı, ardından açık kullanıcı eylemi | Kaynak değiştirilmez. Eski plan config yayınlamak için tekrar kullanılmaz ve abort işareti alır. Config yayını sonrası exit 42 ile kesilen ayrı süreçte yeni eylem yalnız eşleşen sahiplik adımını tamamlar. |
| Eski descriptor'a yayın öncesi/sonrası geç yazım, aynı baytlı inode değişimi veya dış reverse-swap | İki sürüm korunur ve çatışma döner. İptal edilmiş işlem başarılı commit sayılmaz; başarı yanıtında son config/owner kimliği ile committed marker doğrulanır. |

İlk native koşuda 18 test geçti; ACL fixture kurulumu UID metnini kullanıcı adı olarak yorumladığı için durdu. getpwuid_r ile kısa ad çözülüp düzeltildi. İkinci aday geçtikten sonra ana ajan reverse-swap başarı koşulunu düzeltti ve gerçek regresyon ekledi. Son macOS 15.8.2 ARM64 koşusunda 19 test, frontend, kilitli Tauri derlemesi ve mimari kontrolü geçti. İki çocuk süreç yardımcısı doğrudan ignored, ilgili kurtarma testlerinde ayrı süreçte çağrıldı. CI girdileri önce/sonra aynı ve son kaynak adayıyla eşleşir. Windows'ta 129 odaklı test geçti, 2 opt-in ignored. Sonraki yalnız macOS farkları dışındaki kaynak/araç/ortam girdileri eşleştiği için Windows testleri yinelenmedi. fmt ve actionlint son adayda geçti. [Koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/43034274808) · [Önceki koşular ve makine kanıtı](../proofs/macos-config-apply.json).

Odaklı Windows kapısı `cargo test --offline --manifest-path src-tauri/Cargo.toml -- integrations::config_io state::ownership integrations::connection_actions --test-threads=1`; native kapı aynı manuel macOS workflow'unda ilgili macOS transaction test filtresidir. Her yeni CI öncesi güncel kota ve sıfır ücret sınırı doğrulanır. Son koşu öncesinde 12 dolar dahil compute kullanımının 3,16 doları tüketilmiş, ücretli kullanım 0 ve hesap Actions bütçesi 0 dolar ile Usage blocked durumundaydı. Bu tutar ücret değildir.

Bu kabul gerçek dosya backend'ini ve ortak manager kodunu kapsar. Npm istemci metadata'sı ve ready endpoint nesnesi test girdisidir. Kurulu istemci keşfi, canlı sağlayıcı, pencere etkileşimi, eksik Claude proje dizini, korunmuş sürümlerin kullanıcı tarafından güvenli çözümü ve 64 intent sınırının sunumu açık. Tam yerel ürün kapısı H048'in son ana görev adayında çalıştırılır; bu alt dilimde odaklı doğrulama kullanıldı.

## H048-b6b — Eksik Claude proje dizini geçti

11 Kasım 2026. Ana ajan eksik `.claude/settings.local.json` için dar dizin yayını ekledi. Mevcut dosya akışı korundu. Aynı tek Luna xhigh dört transaction/store testi ve ayrı süreç yardımcısını hazırladı; ana ajan manager akışını ve farklı transaction token'ı kontrolünü ekledi. Native RED koşusunda önceki 19 test geçti, altı yeni test desteklenmeyen proje seçimi/dizin oluşturma yolunda durdu. Üretim değişikliği bundan sonra uygulandı.

| Başlangıç ve eylem | Beklenen ve gözlenen |
| --- | --- |
| Proje seçimi → eksik config önizlemesi → Apply → manager yeniden açılışı → kullanıcı ekleriyle Undo | Önizleme/create/seal görünür `.claude` oluşturmadı. Commit özel 0700 staging dizinini yeni 0600 ayarıyla birlikte exclusive yayımladı. Kullanıcı JSON alanları, yan dosya ve dizin korundu; user config değişmedi. |
| Commit öncesi fault/rollback veya seal sonrası drop; dizin yayını sonrası ayrı süreçte exit 42 | Kaynak hedef yok kaldı. Yeni açık eylem yayımlanmamış eski planı replay etmeden abort etti; yayımlanmış dizinde yalnız eşleşen sahiplik tamamlandı. |
| Başka süreç `.claude` oluşturur, proje path'i başka inode ile değiştirilir veya klasör token'ı başka transaction'da kullanılır | Yayın reddedildi. Rakip içerik ve eski/yeni proje dizinleri korundu. Sahiplik oluşturulmadı. |

Proje parent kimliği ve yeni dizin kimliği intent'e eklenir; eski dosya intent'leri opsiyonel alan olmadan okunur. Kaydedilmiş path, seçilmiş proje kimliği ve aktif hedef eşleşmeden açılmaz. Klasör yayını ve sahiplik ayrı rename adımlarıdır; çok dosyalı katı CAS veya güç kesintisi garantisi verilmez. Apple RENAME_EXCL mevcut hedefte EEXIST döndüğünü belgeler. [Apple rename kılavuzu](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/man/man2/rename.2).

Son macOS 15.8.2 ARM64 koşusunda 25 test, frontend, kilitli Tauri derlemesi ve mimari kontrolü geçti. Üç çocuk süreç yardımcısı doğrudan ignored, ilgili testlerde ayrı süreçte çağrıldı. CI girdilerinin önce/sonra parmak izleri eşleşti ve 266 kaynak girdisi adayla aynı. Ortak proje seçimi dosyası değiştiği için Windows odaklı kapı yenilendi; 129 test geçti, 2 opt-in ignored. fmt ve actionlint geçti. Her yeni dispatch öncesi ücretsiz kota ve 0 dolar hard-stop doğrulandı. [RED koşusu](https://github.com/Protocol272/headroom-desktop/actions/runs/43062156522) · [GREEN koşusu](https://github.com/Protocol272/headroom-desktop/actions/runs/43063416122) · [Makine kanıtı](../proofs/macos-claude-project-directory.json).

Bu sonuç özel native fixture, gerçek manager/transaction/ownership kodu ve test picker/endpoint verisini kapsar. Gerçek picker penceresi, kurulu istemci/sağlayıcı trafiği ve paket kabulü değildir. Korunmuş sürümlerin çözümü ile 64 intent sınırının kullanıcıya sunumu açık. Yeni auth, gerçek kullanıcı ayarları, global ortam veya ücretli çağrı kullanılmadı.

## H048-b6b — Korunan sürümler ve kapasite alt kabulü geçti

H048-b6b korunan sürümler dilimi. Ana ajan kayıt/yol/ölçüm ve sahiplik kararlarını üstlenir. Salt okunur inceleme seçili client/scope/config bağlamında güncel config ve intent/marker kimlikleriyle review fingerprint üretir; ham config, auth veya başka proje içeriği renderer'a çıkmaz. Kullanıcının “Mevcut ayarla devam et” eylemi bu fingerprint'i tekrar doğrular. Config dosyası veya korunan eski inode değiştirilmez/silinmez. Bekleyen sahiplik ancak typed receipt'in güncel belgeyle ve kayıttaki sahiplik geçişiyle eşleşmesi hâlinde açık eylemle tamamlanabilir; doğrulanamayan durum sabit hata verir. Geçmiş kayıtları dikkate almadan önce arşiv metadata'sı özel 0700 dizine kopyalanıp doğrulanır ve arşiv manifesti atomik yayımlanır. Bundan sonra eski metadata fd-relative exclusive rename ile arşive taşınabilir; kesinti config verisini kaybetmez ve tekrar açılış aynı manifesti doğrular. Eski config sürümleri istemci klasöründe kalır.

Bir defada en çok 64 aktif kayıt ve en çok 64 arşiv grubu işlenir. Otomatik veri silme yoktur. Kullanıcıya saklanan kayıt ve kapasite sade biçimde gösterilir; sınırda sessiz generic hata yerine inceleme yolu sunulur. Mevcut Windows TxF yolu ve global/başka proje ayarları korunur. Normal kabul inceleme → açık kabul → yeniden açılış → normal Apply/Undo; hata kabulü incelemeden sonra config veya kayıt değişimi ve arşiv sırasında süreç kesintisi; sınır kabulü 64 kayıt ve dolu arşiv kapasitesidir. Native fixture ve gerçek manager yolu kanıtı, renderer sunumu ve gerçek pencere/paket kabulünden ayrı raporlanır.

11 Kasım 2026. Native RED'de eski 25 test geçti, üç yeni arşiv testi Unsupported ile durdu. Son aday macOS 15.8.2 ARM64 üzerinde 34 testi, frontend derlemesini ve kilitli ARM64 Tauri derlemesini geçti. Dört doğrudan ignored çocuk yardımcısı ilgili testlerde ayrı süreçte çağrıldı. 275 kaynak girdisi adayla ve CI önce/sonra fingerprint'i kendi içinde eşleşti. Toplam native test süresi 59,96 saniyedir; bu bir performans veya uzun süreli kararlılık kabulü değildir.

| Senaryo | Gözlenen |
| --- | --- |
| Gerçek manager ile üç adapter: pasif inceleme → açık arşiv → yeniden açılış → Undo | Mevcut config ve sahiplik korunur; yeniden açılan manager arşivi okur, typed Undo özgün ayarı geri verir. Başka config'e veya eski fingerprint'e açık kabul reddedilir. |
| Geç eski descriptor yazımı; bekleyen sahiplik; değişmiş metadata/kopya; arşivin iki yanında süreç çıkışı | Eski inode düzenlemeleri saklanır. Uygun typed sahiplik tamamlanır, değişmiş owned endpoint reddedilir. Bozuk kopya manifest yayımlamaz. Ayrı süreçte kopyalar hazırken veya manifest sonrası exit 43, yeniden açılışta veri kaybetmeden sürdürülür. |
| 64 aktif kayıt ve 64 arşiv grubu | İlk durumda veri silmeden arşiv sonrası yeni Apply/Undo çalışır. İkincide yeni arşiv sabit sınır hatası verir; önceki gruplar ve config değişmez. Yarım hazırlanmış gruplar da koruyucu grup sınırını tüketir. |

Windows odaklı config_io/ownership/connection_actions filtrelerinde 72/33/38 test koşumu başarılı, 2 opt-in ignored (filtreler örtüşebilir). Başarılı 210 frontend testi/build ilgili girdi ve araç/ortam eşleşmesiyle korundu; sonradan düzeltilmiş Rust derleme hatası PASS sayılmadı. İzole Edge panelinde açık checkbox onayı, tam scope/fingerprint ile tek çağrı, pending/parent-busy kilidi, gerçek gecikmiş yanıt, TR/EN klavye ve hata/kapasite durumlarında beş test geçti. İlk worker fixture'ının mount'ta sıfırlanan gecikme kapısı ana ajan incelemesinde düzeltildi ve değişen beş test yenilendi. Görüntü incelendi; native pencere kanıtı sayılmaz.

Odaklı Windows komutları `cargo test --offline --manifest-path src-tauri/Cargo.toml config_io -- --test-threads=1`, aynı komutta `ownership` ve `connection_actions_tests` filtreleridir. Panel komutu `pnpm exec playwright test --config tests/e2e/retained-versions.config.ts --max-failures=1`. Native job mevcut `macos-build.yml`, `verify_config_recovery=true` seçeneğidir. Son dispatch öncesi dahil compute 3,78/12 dolar, ücretli kullanım 0, hesap Actions bütçesi 0 dolar ve Usage blocked doğrulandı. Bu tutar ücret değildir. [RED](https://github.com/Protocol272/headroom-desktop/actions/runs/43068865595) · [GREEN](https://github.com/Protocol272/headroom-desktop/actions/runs/43074294190) · [Makine kanıtı](../proofs/macos-retained-versions.json) · [İnceleme](../reviews/H048.md).

## H048-c1 — Kurulu CLI keşfi

Standart Apple Silicon runner'da üç resmî sabit CLI ikilisi proje içindeki özel test dizinine yerleştirilir. DiscoveryManager, PATH ile `.local/bin` ve `.opencode/bin` altındaki uzantısız komutları salt okunur tarar. Sürüm etiketi eşleşen dosya boyutu ve SHA-256 ile verilir. Keşif, dosyanın çalıştırılabilir olduğunu izinlerinden okur. [Codex sürümü](https://github.com/openai/codex/releases/tag/rust-v0.163.0-alpha.11.1) · [Claude kurulum yerleşimi](https://code.claude.com/docs/en/setup#auto-updates) · [OpenCode V1 kurulum kaynağı](https://github.com/anomalyco/opencode/blob/v1.19.3/install).

| Başlangıç ve eylem | Beklenen gözlem |
| --- | --- |
| Sabit Codex, Claude Code ve OpenCode V1 ikilileri PATH, `.local/bin` ve `.opencode/bin` altında bulunur | Gerçek ortamdan tarama üç aracı doğru sürümle bulur. Aynı hedefe iki yerel bağlantı tek satıra iner ve IPC yanıtında yol görünmez. |
| Sürümü bilinmeyen çalıştırılabilir dosya, yazılabilir dosya, çalıştırılamayan dosya ve döngülü bağlantı vardır | Bilinmeyen sürüm ayrı görünür. Güvensiz adaylar elenir. Hiçbiri çalıştırılmaz ve tarama ayar dosyası oluşturmaz. |
| Keşif sonucundaki kurulum kimliği bir sonraki taramadan sonra kullanılır | Eski taramaya ait seçim reddedilir. Sürüm tanıma, canlı bağlantı durumu olarak gösterilmez. |

Doğrulama `macos-build.yml` içindeki isteğe bağlı native CLI adımında yapılır. Sabit paketler, ikili özetleri, kaynak commit, araç zinciri ve ortam kaydedilir. Windows'ta odaklı keşif testleri ortak kaynak farkını kontrol eder. Ana ajan yol ve sürüm güvenini üstlenir. Luna yalnız resmî test artefaktlarını ve hazırlama komutunu hazırlar. Codex Desktop, gerçek picker, kalıcı kurulum seçimi ve paket yaşam döngüsü sonraki H048-c dilimleridir.

11 Kasım 2026. macOS 15.8.2 ARM64 üzerinde dört test, frontend ve kilitli ARM64 Tauri derlemesi geçti. Gerçek DiscoveryManager, ayrı sürecin PATH ve HOME ortamından Codex CLI 0.163.0-alpha.11.1, Claude Code 2.1.158 ve OpenCode V1 1.19.3'ü buldu. Claude bağlantısı sürüm klasörüne, OpenCode doğrudan kullanıcı bin dizinine yerleştirildi. İki Codex bağlantısı tek satır verdi. Aynı boyutta farklı içerik taşıyan dosya sürümü bilinmiyor kaldı. Yazılabilir, çalıştırılamayan ve döngülü adaylar elendi. Yanıtta dosya yolu görünmedi. Eski tarama kimliği reddedildi ve test istemci komutu çalıştırmadı. Üç ikili ve CI girdilerinin önce/sonra özetleri eşleşti.

Windows `cargo test --offline --manifest-path src-tauri/Cargo.toml discovery -- --test-threads=1` koşusunda 30 test geçti, iki opt-in test atlandı. fmt ve actionlint geçti. Manuel macOS koşusu öncesi dahil compute kullanımı 3,97/12 dolar, ücretli kullanım 0 ve Actions bütçesi 0 dolar ile Usage blocked durumundaydı. [Koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/43079382814) · [Makine kanıtı](../proofs/macos-cli-discovery.json).

## H048-c2 — Kurulum seçiminin kalıcılığı

macOS'ta kurulum seçimi mevcut `ChoiceStore` ve aynı IPC sözleşmesini kullanır. Seçim, uygulamanın kendi `onboarding/selections.json` dosyasına yazılır. İlk okuma pasiftir. Yazma yalnız açık seçimden sonra 0700 dizin ve 0600 dosyayla yapılır. Yerel APFS dizin tanıtıcısı, süreçler arası kilit ve atomik yayın kullanılır. Mevcut kayıt yapısı ve Windows TxF yolu korunur.

| Başlangıç ve eylem | Beklenen gözlem |
| --- | --- |
| Boş uygulama verisi → tarama → bir istemci seç → yeni manager ile yeniden aç ve tara | İlk okuma dosya oluşturmaz. Seçim taze tarama kimliğine bağlanır. Diğer istemci seçimleri ve varsa Claude kapsamı korunur. |
| Seçilen ikili aynı boyutta düzenlenir veya başka inode ile değiştirilir | Eski seçim kullanılamaz görünür. Yeni bir dosya kendiliğinden seçilmez. Eski taramadan kayıt isteği reddedilir. |
| İki yazıcı aynı anda erişir, süreç yayın öncesi veya sonrası kesilir, mevcut kayıt bozulur veya yol güvenliği değişir | Kilit çakışmayı durdurur. Yeniden açılış yalnız tam eski veya yeni kaydı okur. Bozuk kayıt ve yabancı dosyalar otomatik değiştirilmez. |

Apple Silicon macOS 15.8.2 üzerinde resmî üç CLI ikilisiyle gerçek tarama ve yeni yöneticiye yeniden bağlama geçti. Claude proje seçimi de yeni `PlanManager` ile geri yüklendi ve geri alma kullanıcının sonraki eklerini korudu. APFS testleri yarışan yazıyı, süreçler arası kilidi, atomik yayının iki yanında süreç çıkışını ve geçersiz kayıtları kapsadı. 15 macOS testi, frontend ve kilitli ARM64 derlemesi geçti. İki çocuk süreç yardımcısı ilgili testlerde çağrıldı. 107 odaklı Windows testi geçti, iki opt-in test atlandı. [Makine kanıtı](../proofs/macos-selection-store.json) · [Koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/43083203659).

Yönetici yeniden oluşturularak açılış eşlemesi doğrulandı. Claude proje testi, seçilen yolu testten aldı. Gerçek dosya seçici ve paketlenmiş pencerenin yeniden açılışı kendi kabullerinde sınanacak.

## H048-c3 — Codex Desktop keşfi

macOS'ta sistem ve kullanıcı `Applications` dizinlerindeki `Codex.app` ve `ChatGPT.app` adayları incelenir. Başlatıcı ve `Info.plist` imzası OpenAI kimliğiyle yerel olarak doğrulanır. Sürüm bu metadata'dan okunur. Tarama istemciyi çalıştırmaz. Sürüm görünümü keşif kanıtıdır. macOS Desktop bağlantı planı canlı istemci kabulüne kadar kapalı kalır.

| Başlangıç ve eylem | Beklenen gözlem |
| --- | --- |
| İzole uygulama dizininde gerçek imzalı OpenAI paketi → tarama | Codex masaüstü kaydı doğru sürümle görünür. CLI kayıtları ayrı kalır. Yanıtta yerel dosya yolu bulunmaz. |
| Açık seçim → yeni yöneticiyle tarama → paketin değişimi | Aynı başlatıcı taze kimliğe bağlanır. Değişen veya kaldırılan başlatıcı eski seçimi geçersiz kılar. |
| İlgisiz, imzasız veya değişmiş metadata, sembolik bağlantı, güvenli olmayan konum | Aday görünmez ve tarama sürer. İstemci dosyası veya ayarı değişmez. |

Ana ajan paket doğrulama, seçim ve bağlantı sınırlarını üstlenir. Tek Luna resmî disk imajının sürüm ve özet kanıtını hazırlar. Native kabul ücretsiz `macos-build.yml` koşusunda gerçek Apple Silicon disk imajından kurulan izole paketle çalışır. Windows keşif/plan ve frontend sözleşme kontrolleri odaklı çalışır.

12 Kasım kabulü tamamlandı. Resmî 26.1106.51428 disk imajı SHA-256 ile sabitlendi ve izole dizine açıldı. Security framework OpenAI takımını, paket kimliğini, başlatıcıyı ve Info.plist imzasını doğruladı. Kullanıcı Applications dizini ve yönetici grubuna yazılabilir sistem Applications dizini test edildi. Aynı baytlı yeni başlatıcı eski seçimi geçersiz kıldı. Değişmiş sürüm metadata'sı eski seçme/kaydetme isteğinde ve taze taramada reddedildi. İmzasız kopya, sembolik bağlantı ve yazılabilir metadata adayları elendi.

İlk özel dizin koşusunda 17 test geçti. Ek sistem dizini testi mevcut izin kontrolünde başarısız oldu. Contents dizini üzerinden yerel dosya sistemi kontrolü sonrası son macOS 15.8.2 ARM64 koşusunda 17 test, frontend ve kilitli derleme başarılı. İki çocuk süreç yardımcısı ilgili testlerde çağrıldı. Windows'ta 61 Rust testi, 29 frontend testi ve typecheck geçti. İki sonraki macOS dosyası farkı dışındaki 282 Windows girdisi ve 89 frontend girdisi eşleşti. [Makine kanıtı](../proofs/macos-desktop-discovery.json) · [Son koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/43196478298).

İmza denetimi yereldir. Çevrimiçi sertifika iptali, paket kaynaklarının tamamı, Intel mimarisi ve gerçek Desktop trafiği ayrıca doğrulama gerektirir. Bu aşamada macOS Desktop bağlantı planı kapalıdır.

## H048-c4 — Yerel dosya ve klasör seçici

İstemci dosyası ve Claude proje klasörü için mevcut IPC, sahiplik ve seçim deposu kullanılır. macOS AppKit paneli ana iş parçacığında açılır. Dosya seçimi uzantısız CLI komutlarını kabul eder ve mevcut dosya kimliği kontrolüne bağlanır. Klasör seçimi güvenli yerel projeyi kaydeder. Bu dilim rapor dışa aktarma veya yeni bağlantı yöntemi eklemez.

| Başlangıç ve eylem | Beklenen gözlem |
| --- | --- |
| Dosyadan ekle → gerçek panelde CLI seç → tara ve yeniden aç | Doğru istemci dosyası doğrulanır. Seçim taze kimliğe bağlanır. Panel istemciyi çalıştırmaz. |
| Claude proje kapsamı → gerçek panelde klasör seç → yeniden aç | Klasör mevcut PlanManager ve ChoiceStore akışına girer. Keşif veya seçim config dosyası oluşturmaz. |
| Paneli iptal et, yanlış dosya seç veya dosyayı değiştir | İptal önceki seçimi korur. Yanlış dosya sabit hata verir. Değişmiş dosya eski seçime bağlanmaz. |

Ana ajan ana iş parçacığı, yol doğrulama ve gerçek yönetici akışını uygular. Tek Luna sabitlenmiş AppKit API'lerini salt okunur inceler. Doğrulama mevcut ücretsiz macOS Actions koşusunda gerçek panel, kontrollü yerel dosyalar ve sınırlı süreli ayrı süreçle yapılır. Girdi/araç/ortam parmak izi alınır. Windows'ta etkilenen keşif ve Claude kapsamı testleri odaklı çalışır.

12 Kasım'daki son macOS 15.8.2 ARM64 koşusunda dört gerçek panel senaryosu geçti. Dosya ve klasör iptali, resmî uzantısız Codex ikilisinin dosya seçimi ve Unicode proje klasörünün ana iş parçacığında seçimi doğrulandı. Test uygulaması LaunchServices ile açılır. Dış System Events sürücüsü kendi test penceresinin ön planda olduğunu, yol alanını ve etkin Open düğmesini kontrol eder. Panelin döndürdüğü yollar beklenen dosya ve klasörle eşleştirilir. Seçilen ikilinin önce/sonra SHA-256 değeri sabitlenen resmî dosyayla aynıdır.

16 odaklı test ve gerçek panel sonuçlarını kullanan bir ek yönetici testi geçti. Ek test, panelden dönen CLI seçimini DiscoveryManager üzerinden kaydeder ve yeni yöneticinin taze taramasında doğrular. Aynı panelden dönen proje klasörü Claude kapsam seçimine taşınır ve yeni PlanManager ile yeniden okunur. Seçim proje config dosyası oluşturmaz. Hatalı dosya, iptal ve değiştirilmiş istemci mevcut odaklı testlerde sınanır. Windows'ta 46 testin kanıtı ve 281 ilgili girdi korunur. Frontend ve kilitli ARM64 derleme geçti. [Makine kanıtı](../proofs/macos-file-picker.json) · [Başarılı koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/43217388795).

Bu kabul üretim seçicisini kullanan AppKit test paketini ve gerçek yönetici akışını kapsar. Paketlenmiş Tauri ana penceresinin etkileşimleri H048'in sonraki kabulüne aittir.

## H048-c5 — Paketlenmiş pencere ve motor yaşam döngüsü

Amaç, Tauri `.app` paketinin kendi sabitlenmiş kaynaklarıyla gerçek pencere ve motor akışını doğrulamaktır. Ana ajan paketleme, yerel state ve süreç sahipliğini üstlenir. Tek Luna xhigh, paket içindeki runtime envanterinin kaynakla eşitliğini doğrulayan salt okunur yardımcıyı ve odaklı testlerini hazırlar. Mevcut payload hazırlama ve atomik staging kullanılır. Native GUI, önceki panel kabulündeki dış System Events yöntemiyle sürülür.

| Başlangıç ve eylem | Beklenen gözlem |
| --- | --- |
| Yeni ve projeye ait QA state ile `.app` paketini aç | Yerleşik arayüz açılır. Motor kaynakları paket altında bulunur. Runtime manifesti, dosya içerikleri, izinler ve sembolik bağlantılar sabitlenmiş kaynakla eşleşir. |
| Pencereden Başlat → hazır → Durdur → tekrar Başlat | Gerçek motor ve sahipli yerel uç nokta doğrulanır. İlk süreç kapanır, yeni başlangıç yeni sürece bağlanır. UI durumu gözlenen süreçlerle tutarlıdır. |
| Gerçek çıkışı onayla ve paketi yeniden aç | Uygulama ve kendi motor süreçleri kapanır. State kilidi ve uç nokta sonraki açılışı engellemez. Başka süreç veya istemci ayarı değiştirilmez. |

Derleme mevcut kilitler ve standart ücretsiz Apple Silicon Actions koşusuyla yapılır. QA build, uygulama verisini proje içindeki özel dizine bağlayan Tauri yapılandırmasını kullanır. Release profili bu koşuda LTO kapalı ve 16 codegen units ile derlenir; son üretim profilinin performans veya yayın kabulü değildir. Motor kaynakları imzalamadan önce eklenir ve doğrulanır. Uygulama için yerel ad-hoc imza kullanılır. Bu dilim üretim yayını, Apple notarizasyonu, Intel veya canlı sağlayıcı trafiğini kapsamaz. Kaynak, paket ve ortam parmak izleri programatik alınır. Süreç kontrolleri çalıştırılan paketin ve doğrulanmış çocuklarının kimliğine bağlıdır.

12 Kasım 2026. `c036f72` adayı macOS 15.8.2 ARM64 üzerinde gerçek `.app` paketiyle geçti. LaunchServices üzerinden iki ayrı açılışta normal arayüz ve Tauri komutları kullanıldı. Başlat, hazır durumu, Durdur, tekrar Başlat ve onaylı çıkış üç farklı doğrudan çocuk motor süreciyle eşleşti. Aynı kayıtlı loopback portu yeniden açılışta korundu. Motor ve ingress kilitleri çalışırken tutuldu, normal kapanışta serbest kaldı; port yeniden bağlanabildi. Çıkış kabulünde zorla süreç sonlandırma kullanılmadı.

Paketin 16.640 dosya ve 597.415.256 baytlık runtime envanteri, manifest, overlay'ler, izin ve bağlantıları önce/sonra sabit kaynakla eşleşti. Uygulama ikilisinin hash'i değişmedi, ad-hoc imza strict denetimi geçti. Native Python kontrollerinde 28 geçti, 1 atlandı; native symlink kontrollerinin atlanması hazırlama adımında hata sayılır. Windows'ta 8 test geçti, 3 POSIX/symlink kontrolü atlandı. 292 yerel kaynak/workflow girdisi ve native önce/sonra parmak izleri eşleşti.

İlk denemelerde testin umask varsayımı, aynı dinleyicinin kopya dosya tanıtıcıları ve render sırasında kaybolan AX nesnesi görüldü. Ana ajan fixture'ı gözlenen izinlere bağladı; port denetimi tek beklenen adres kümesini doğrular ve farklı adresi reddeder. Sürücü yalnız kaybolmuş okuma nesnesinde yeni snapshot alır; tıklama hatasını tekrar etmez. Tek Luna xhigh envanter yardımcı ve regresyonlarını hazırlayıp durdu; paketleme, süreç/ayar sahipliği, native sürücü ve sonuç kabulünü ana ajan yürüttü. [Makine kanıtı](../proofs/macos-package-lifecycle.json) · [Başarılı koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/43227012806) · [İnceleme](../reviews/H048.md#paketlenmiş-pencere-ve-motor-yaşam-döngüsü).

## H048-c6 — macOS rapor dışa aktarma

Amaç mevcut Genel bakış → Dışa aktar → ReportManager → AppKit kaydetme paneli → güvenli yeni dosya yolunu macOS'a taşımaktır. Ana ajan dosya/panel sahipliği ve atomik yayın kararlarını üstlenir. Mevcut rapor alan izin listesi, snapshot kimliği, boyut sınırı, TR/EN CSV ve çıkış bariyeri korunur. Yeni ölçüm, sağlayıcı çağrısı, auth veya UI tasarımı eklenmez. Linux writer bu dilimde açılmaz.

| Senaryo | Kabul |
| --- | --- |
| Gerçek pencereden istatistik JSON, CSV ve tanı JSON kaydet | Kaydet paneli görünür; seçilen yeni Unicode isimli dosyanın tamamı gerçek writer ile kaydedilir. JSON/CSV biçimi ve mevcut içerik dışlama sözleşmesi korunur. |
| Kaydet panelini iptal et | Arayüz iptali gösterir; yeni dosya veya staging artığı oluşmaz, sonraki dışa aktarma çalışır. |
| Hedef zaten var veya başka yazıcı aynı hedefi yayımlar | Üzerine yazılmaz. Tek tam dosya kalır, rakibin baytları korunur; başarısız staging temizlenir. |
| Yanlış uzantı, boş/aşırı büyük rapor, symlink/alias parent veya güvenilmeyen dosya sistemi | Yazmadan reddedilir. Yerel APFS ve mevcut dizin kimliği denetimleri kullanılır. |

Önce macOS writer testleri mevcut `unavailable` davranışında RED olarak çalıştırılır. Son aday `macos-package.yml` içindeki `verify_report_export` ile odaklı Rust rapor testlerini ve gerçek paket/panel akışını ücretsiz Actions'ta doğrular. Yerel Windows'ta değişen ortak picker yönlendirmesi ve rapor davranışının odaklı testleri kullanılır; tam ürün kapısı ana H048'in son adayına aittir. Kaynak, araç, ortam ve sonuç önce/sonra fingerprint ile kaydedilir. QA ad-hoc paket ve canlı trafik sınırları H048-c5 ile aynıdır.

12 Kasım 2026. Son `cf4f4bc` adayı macOS 15.8.2 ARM64 üzerinde geçti. Gerçek Genel bakış menüsünden açılan NSSavePanel'de iptal, ardından istatistik JSON, TR CSV ve tanı JSON kaydı yapıldı. Üç dosya Unicode adlarıyla okundu; biçim/ölçüm kapsamı, 0600 izin ve staging artığı olmaması doğrulandı. Bunlar izole uygulamanın gözlediği yerel metriklerdir; canlı sağlayıcı trafiği veya tasarruf benchmarkı değildir.

11 native Rust rapor testi geçti: gerçek ReportManager ile tüm biçimlerde kayıt ve mevcut dosyanın korunması; aynı hedefe altı eşzamanlı yazıcıdan tek tam kazanan; parent/hedef symlink, güvensiz dizin, yol/uzantı ve boyut sınırı; değişmiş staging kimliği ve yabancı değişimin korunması. Mevcut beş rapor testi negatif/sıfır/bilinmeyen ölçüm, içerik dışlama, snapshot, iptal ve çıkış bariyerini kapsadı. Native Python kontrollerinde 28 geçti/1 atlandı. Windows'ta 8 odaklı test geçti. Sonraki yalnız macOS/sürücü değişiklikleri dışında Windows girdileri ve picker dalı eşleşti; aynı başarılı testler tekrarlanmadı.

Rapor panellerinden sonra gerçek paket iki kez açıldı, üç farklı motor süreci Başlat/Durdur/tekrar Başlat ve normal çıkış yoluyla doğrulandı. Aynı kayıtlı port ve kilitler serbest kaldı. Paket runtime envanteri ve ikili hash'i değişmedi. 294 yerel kaynak/workflow girdisi ile native önce/sonra fingerprint'leri eşleşti. Native koşular her dispatch öncesi gözlenen ücretsiz kota ve sıfır dolar harcama engeli altında başlatıldı.

Önce dört test RED verdi. İlk uygulama derlemesinde bağımsız picker örneğinin rapor DTO'suna bağımlılığı ve URL dönüşüm tipi düzeltildi. Sonraki koşu raporları geçti; motor Başlat sonrası AX okuma dizini değişti. Dış sürücü yalnız -1728/-1719 eski nesne/dizin okumalarında yeni snapshot alır, tıklama hatasını tekrar etmez. [Makine kanıtı](../proofs/macos-report-export.json) · [Başarılı koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/43261107448) · [İnceleme](../reviews/H048.md#macos-rapor-dışa-aktarma).

## H048-c7 — Gerçek macOS Desktop bağlantısı

Ana ajan auth, rota ve süreç sahipliğini üstlenir. İlk adım, sabitlenmiş resmî Desktop paketinin ayrı Electron/Codex test state'iyle gerçek macOS açılışını ve paket içindeki app-server config sözleşmesini gözlemektir. Tek Luna xhigh mevcut DMG'nin salt okunur paket/başlatma envanterini inceler. Windows'taki aynı backend veya CLI sonucu macOS Desktop kanıtı sayılmaz. Keşif ve mevcut bağlantı engeli canlı istemci kanıtı gelene kadar korunur.

| Aşama | Kabul ve sınır |
| --- | --- |
| Resmî paket, tam imza ve izole app-server config okuma | Sabit paket/ikili doğrulanır. Yeni özel CODEX_HOME içindeki tek endpoint ayarı gerçek bundled backend tarafından okunur. Model çağrısı, gerçek kimlik bilgisi ve kullanıcı config değişimi yoktur. |
| Gerçek GUI'yi ayrı uygulama state'iyle aç | Çalışan executable ve çocukları bu pakete bağlanır. Görünür başlangıç/giriş durumu ve normal kapanış gözlenir. Profil yalıtımı yalnız geçerli flag/sabit kaynak ve native gözlemle kabul edilir. |
| Gerçek Desktop → Headroom → kontrollü yerel sağlayıcı | Native istemci tarafından oluşturulan istek/yanıt, model/efor/auth türü ve rota korunması gözlenir. Bu, canlı sağlayıcı veya mevcut hesap kabulü değildir. |
| Yetkili gerçek hesap/sağlayıcı ve yeniden açılış | Canlı yanıt ve kayıtlı endpoint'in benimsenmesi ayrı kanıtlanır. Hesap/sır CI'a taşınmaz. Ortam yoksa bu kabul açık kalır; fixture veya app-server sonucu Desktop canlı trafik PASS'ine yükseltilmez. |

İlk native koşu yalnız başlangıç ve config kabiliyetini araştırır; Rust ve ürünün tam kalite kapısını yeniden çalıştırmaz. Standart ücretsiz Actions, gözlenen kota ve sıfır dolar harcama engeli gerekir. Kaynak/komut/ortam/paket girdilerinin önce/sonra fingerprint'leri kaydedilir. Hata tanısı yalnız sabit kodlar ve sayılar içerir. Başka kullanıcı profili, model/efor veya auth ayarı değiştirilmez. Yeni auth akışı, güvenlik engelini aşma veya giriş ekranını sahte hesapla atlama bu kapsamda değildir.

Ön kontrolün uygulama state kökü yalnız CI'da oluşturulan kısa `/private/tmp/hr-c7-<rastgele>` dizinidir; macOS Unix soket uzunluğu sınırı nedeniyle bu açık QA state istisnası kullanılır. Codex/Electron/home/temp alt dizinleri özeldir; ortam gerçek API/GitHub hesap sırlarını devralmaz. Normal config ve uygulama profilleri başlangıçta yok olmalı ve sonda da oluşmamalıdır. Paket/geliştirme dosyaları proje `work` altında kalır. Yalnız gözlenen sahipli süreçler kapandıktan sonra UID/inode/mode doğrulanan QA state temizlenir. Gözlem bir model turu başlatmaz.

İlk native gözlem tam state yalıtımı koşulunu karşılamadı: `.codex` ile normal Codex/ChatGPT profilleri yok kalırken özel Electron profilinde Local State ve Default oluştu; `~/Library/Application Support/com.openai.codex` altında tek, sınıflandırılmamış kayıt belirdi. Bu vendor alanının amacı bilinmiyor. Dar devam kabulü yalnız temiz CI kullanıcısında, başlangıçta bulunmayan bu açık vendor state köküne izin verir; UID/inode/canonical dizin denetimi ve normal çıkış sonrası temizleme ister. Core profiller değişmemeli ve özel Electron profili gerçekten kullanılmalıdır. Bu daha dar ön kontrol tam uygulama state yalıtımı, GUI config benimsenmesi veya trafik başarısı olarak raporlanmaz; ilk katı koşulun başarısız kanıtı korunur.

12 Kasım 2026. `2a2e0b4` adayı dar ön kontrolü macOS 15.8.2 ARM64 üzerinde geçti. Sabitlenmiş resmî Desktop 26.1106.51428 strict/deep imza denetiminden geçti. Paket içindeki CLI 0.167.0-alpha.4.1 ayrı bir stdio app-server oturumunda açık built-in `openai` sağlayıcısını ve loopback endpoint'ini okudu; hesap yok ve normal çıkış gözlendi. İlk provider guard, yazılmamış alanın açık `openai` döneceğini varsaydı; raw okumada bu eşitlik sağlanmadığı için test profili açık sağlayıcı değeri kullanır. Birebir okuma koşulu korunur.

Gerçek Electron penceresi bir gözlenen bundled backend çocuğuyla açıldı. Sınırlı AX gözleminde 1 pencere/27 kontrol vardı; tanınan giriş/API-key düğmesi yakalanmadı. Bu nedenle GUI auth kabiliyeti bilinmiyor. CmdQ sonrası uygulama ve gözlenen 13 sahipli çocuk kapandı. Özel Electron profilinde Local State ve Default oluştu; normal core profiller yok kaldı. Yeni vendor state UID/inode/mode ve canonical yol ile bağlandı; hem o namespace hem kısa QA state normal çıkış sonrası kaldırıldı. Seçili dört paket girdisi ve native önce/sonra fingerprint'leri eşleşti.

3 yerel config/ortam/metadata gizlilik testi geçti. Native hazırlık 32 Python testinden 31'ini geçirdi, 1 atlandı. Ana ajan uygulama, auth/süreç ve kapsam kararlarını yürüttü. Aynı Luna xhigh iki salt okunur Asar incelemesini tamamlayıp durdu; statik işaretlerin normal profil çözümlemesini tümüyle açıklamadığı sınırı korundu. Ürün kaynağı ve mevcut macOS Desktop bağlantı engeli değiştirilmedi. Headroom'a ulaşan model turu gönderilmedi, loopback model isteği gözlenmedi; Desktop rota/sağlayıcı/savings kabulü çıkarılmaz. Sırada gerçek GUI isteğinin kontrollü yerel sağlayıcıya Headroom üzerinden ulaştığının doğrulanması var. [Makine kanıtı](../proofs/macos-desktop-preflight.json) · [Başarılı koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/43271658774).

Kaynak sınırı: [OpenAI gelişmiş yapılandırma](https://learn.chatgpt.com/docs/config-file/config-advanced) CODEX_HOME ve built-in provider endpoint'ini, [app-server protokolü](https://learn.chatgpt.com/docs/app-server) initialization/stdio/config okumayı açıklar. Bu belgeler sürümü sabitlenmiş gerçek GUI trafiğinin yerine geçmez.

H048-c7b'nin ilk native gözlemi geçti: sahipli loopback hedefindeki gerçek renderer `complete`, 5 root çocuğu, 4 görünür düğme ve “Sign in another way” kontrolünü gösterdi. Normal kapanış, özel state temizliği ve önce/sonra girdi eşitliği geçti. Model çağrısı yok; trafik kabulü açık. [Dar kanıt](../proofs/macos-desktop-renderer.json) · [Koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/43278256460).

Dış sürücü yalnız özel Electron profilinin DevToolsActivePort kaydını ve aynı sahipli app PID'sinin 127.0.0.1 dinleyicisini kabul eder; başka port/host, kimlik bilgili URL, farklı sayfa veya birden çok app sayfası reddedilir. Okuma yalnız sabit UI etiketleri, görünür kontrol sayıları ve hazır durumu döndürür; bilinmeyen metin/hesap/içerik kaydedilmez. Hazır durumu veya auth yolu doğrulanmadan istek gönderildi denmez. Tam rota kabulü gerçek GUI girdisinden yerel sağlayıcıya koşuya özel eşleme, korunmuş model/efor/sentetik auth, terminal öncesi görünür yanıt ve temiz kapanış ister. Test profilindeki sentetik API anahtarı yalnız yerel fixture içindir; gerçek hesaba veya ücretli servise giriş yapılmaz. [OpenAI auth belgesi](https://learn.chatgpt.com/docs/auth) yerel Desktop API-key yolunu tanımlar; gerçek sürüm davranışı ayrıca sınanır.

İlk tam GUI koşusunda gerçek paketli motor üzerinden kısmi/tam yanıt görüldü; test config dosyasının byte eşitliği durdu. Ayrı direct-fixture tanısı değişikliğin yalnız yeni `[desktop]` tablosu olduğunu, endpoint/model/efor/provider/dosya credential deposunun eşleştiğini gösterdi. Bu tanı Headroom rotasını kanıtlamaz. Yeni kontrollü aday yalnız başlangıçta bulunmayan Desktop tablosunun eklenmesini kabul eder; diğer bütün TOML alanları birebir aynı olmalı, mevcut Desktop tablosu değişikliği de reddedilir. No-account ön kontrolün byte eşitliği koşulu korunur. Tam dosyanın byte eşitliği yeni kontrollü koşuda iddia edilmez; önce/sonra config hash'leri ayrı kaydedilir. İlk başarısız sonuç [korunur](../proofs/macos-desktop-traffic-attempt.json). Kod/anahtar/içerik yerine yalnız alan adları ve eşleşme bayrakları alınır.

### H048-c7b kontrollü GUI akışı geçti

12 Kasım 2026. macOS 15.8.2 ARM64 üzerinde resmî Desktop 26.1106.51428 ve bundled CLI 0.167.0-alpha.4.1, normal renderer girdisiyle tek eşleşen kullanıcı isteğini paketli Headroom 0.41.0 üzerinden yerel WebSocket sağlayıcısına iletti. Test modeli `gpt-5.2-codex`, efor `medium` ve sentetik API auth değişmeden ulaştı. Nonce input içinde doğrulandı; promptta olmayan ayrı partial/final işaretleri gerçek renderer'da gözlendi. Terminal, partial görülene kadar fixture tarafından tutuldu. Diğer dört istek fixture tarafından reddedildi; bunların kullanıcı turu/yardımcı iş/alt ajan kimliği bu koşuda sınıflandırılmadı. Birden fazla modelin veya gerçek alt ajanın başarılı yanıt aldığı iddia edilmez.

Gerçek Headroom collector'ı içeriksiz SQLite olaylarına aynı modelden 2 eşleşme kaydetti; toplam 5 olay vardı. Bu sayılar tasarruf/benchmark sonucu değildir. Endpoint/provider/model/efor/dosya auth deposu ve Desktop dışındaki bütün TOML değerleri aynı kaldı. Yeni `[desktop]` tablosu nedeniyle config'in byte hash'i farklıdır; iki hash ayrı kaydedildi. Normal `.codex` ve Codex/ChatGPT core profilleri yok kaldı; özel profil ve sahipliği doğrulanan vendor state temizlendi. Tam uygulama state yalıtımı hâlâ `not_established`.

Codex normal CmdQ ile, Headroom kendi çıkış onayıyla kapandı. Gözlenen 11 Codex çocuğu ve gerçek Headroom motoru çıktı; Headroom portu ve iki sahiplik kilidi bırakıldı. Vendor paket ve Headroom bundle/runtime önce/sonra envanterleri, CI kaynak/araç/ortam fingerprint'leri eşleşti. 45 native Python testinden 44'ü geçti, 1'i atlandı; Windows'ta 15 ilgili testin PASS'i güncel girdi/araç eşleşmesiyle bağlandı, 1 POSIX test skipped idi. Son inceleme ve collector tanı ekinden sonra yalnız etkilenen kontroller yenilendi. QA ad-hoc/LTO kapalı/16 codegen profili üretim imzası veya performansı kanıtlamaz. [Makine kanıtı](../proofs/macos-desktop-traffic.json) · [Başarılı koşu](https://github.com/Protocol272/headroom-desktop/actions/runs/43284385567).

### H048-c7 ürün bağlantısı — etkin kabul

c7b'nin sabit sürüm kanıtı temelinde macOS config kontratı yalnız resmî, imzalı Desktop `26.1106.51428` için açılacak. Discovery'nin salt okunur ve taze seçim denetimi, mevcut endpoint yaması ve sahiplik kaydı kullanılacak. Model adı/efor/ajan sayısı bu sürüm sınırının koşulu değildir. Model, provider, reasoning ve auth deposu ile ilgisiz TOML alanları korunacak. Oturum veya hesap doğrulaması otomatik başlatılmayacak; kayıtlı config desteği gerçek canlı hesap/istek doğrulaması diye sunulmayacak.

Normal kabul gerçek vendor bundle keşfinden özel test config'iyle Preview → Apply → OwnershipStore/PlanManager reopen → Undo'dur. Apply sonrası eklenen Desktop tercihi Undo'dan sonra kalır, endpoint kalkar ve sentetik auth dosyasının baytları değişmez. Hata kabulleri preview sonrası dosya değişiminin Apply'i reddetmesi ve endpoint drift'inin Undo'yu dosya/sahiplik korunarak reddetmesidir. Bilinmeyen Desktop sürümlerinin macOS'ta kapalı kalması ayrıca kontrol edilir. Testler vendor uygulamasını veya gerçek kullanıcı istemcisini başlatmaz/yazmaz; önceki c7b GUI trafiği kendi fingerprint kapsamıyla ayrı kanıttır.

Ana ajan sürüm/rota ve uygulama kararlarını üstlenir; aynı Luna yalnız mevcut `config_io/macos_tests.rs` yardımcı testlerini yazar. Yerelde `plan_tests`, ilgili catalog/recovery kontrolleri ve fmt; native ortamda `macos-build.yml` Desktop seçeneğiyle yeni `native_desktop_manager_` testleri ve platform sürüm kontratı kullanılır. Tam yerel ana görev kapısı c8'de bir kez çalıştırılır. GitHub standart macOS koşusu ancak güncel dahil kota ve hesap Actions `$0` hard stop yeniden doğrulanırsa başlatılır.

İlk native adayda 13 test geçti; tek hata katalog snapshot kontrolüydü. Katalog, macOS'un derlemeye bağlı payload pin'ini Windows manifestiyle karşılaştırıyordu; ayrıca ilk kısa derlemede macOS pin'i hazırlanmamıştı. Bu başarısız koşu [korundu](../proofs/macos-desktop-connection-attempt.json). Düzeltme Mac katalog kullanılabilirliğini runtime'ın verdiği derleme pin'ine bağlar; eksik/bozuk pin hâlâ `unavailable` verir. Gerçek dosya/manifest/version bütünlüğünü motor başlatılmadan önce doğrulayan `MacRuntime` denetimleri korunur. Windows'un mevcut manifest/hash karşılaştırması aynı kalır. Pozitif native katalog kabulünde gerçek kilitli macOS payload'ı hazırlanır; engine lifecycle tekrar istenmedikçe yeni motor/model çağrısı başlatılmaz.

## Açık kabuller

- H048-c1–c6, c7a dar ön kontrol ve c7b kontrollü GUI akışı tamamlandı. H048-c7 ürünün macOS Desktop bağlantı engelinin kanıta göre değerlendirilmesi ve gerçek bağlantı planı; c8 son görev kabulü açık. Canlı sağlayıcı, gerçek hesap/alt ajan ve diğer sürüm/model kapsamı bu fixture'dan çıkarılmaz.
- Ana H048'in son adayında tam yerel kalite kapısı çalıştırılır. Alt dilimlerdeki odaklı sonuçlar ana görev kapanışı değildir.
- QA ad-hoc imzası üretim uygulaması/motor imzası, notarizasyon veya yayın kabulü değildir. Apple Silicon sonucu Intel, diğer macOS sürümleri, Windows 11 veya Linux'a aktarılmaz. Üretim yayını veya ücretli imzalama bu koşuların kapsamı dışındadır.
