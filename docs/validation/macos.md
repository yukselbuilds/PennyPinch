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

Bir sonraki motor koşusu, Python ve kilit dosyasına ek olarak bütün paket dosyalarının ve göreli bağlantılarının önce/sonra eşitliğini ölçecek. Sabitlenmiş Python dağıtımının paket içindeki dosya bağlantıları korunurken kök dışı bağlantı, dizin bağlantısı, özel dosya, isim çakışması ve sır dosyaları reddedilecek.

## Açık kabuller

- H048-b için platforma ve mimariye özel sabitlenmiş motor ve doğrulanabilir süreç/loopback sahipliği, atomik ayar işlemleri ve çatışmayı koruyan geri alma gerekir.
- H048-c için paketlenmiş uygulamanın gerçek açılışı, başlat/durdur ve temiz state kabulü ile paket ve motor imzasının ayrı doğrulanması gerekir. Apple Silicon sonucu Intel, Windows 11 veya Linux'a aktarılmaz. Üretim yayını veya ücretli imzalama bu koşuların kapsamı dışındadır.
