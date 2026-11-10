# macOS doğrulaması

## H048-a — Apple Silicon kaynak ve derleme

İlk hedef GitHub'ın standart macos-15 ARM64 runner'ında Apple Silicon için gömülü frontend içeren bir Tauri ikilisi. Pencere sahipliği isteyen Win32 dosya seçicileri Windows yolunu korur. macOS'ta güvenli karşılığı olmayan ayar işlemleri desteklenmiyor sonucu verir. Motor veya istemci yetenekleri derleme sonucundan çıkarılmaz.

| Senaryo | Beklenen |
| --- | --- |
| Dahil kullanım ve hesap harcama durdurucusu doğrulanmış, elle dispatch edilmiş aday | Kilitli frontend bağımlılıklarıyla typecheck/build ve kilitli Tauri derlemesi geçer. İkili yalnız ARM64 Mach-O'dur. |
| Onay alanı false veya doğrulama eksik | İş başlamaz. Derleme veya destek durumu geçer sayılmaz. |
| Rust/macOS hedefi farklı, girdiler değişmiş ya da derleme başarısız | İş başarısız olur. Önce/sonra girdi parmak izi ve kısa derleyici tanısı saklanır. |

Workflow pinned checkout/setup-node, .node-version, pnpm 11.25.0 ve Rust 1.92.0 kullanır. 20 dakikalık tek standart job sınırı, cache ve artefakt yükleme olmadan uygulanır. Her dispatch öncesi [free-only kararına](../decisions/006-free-ci.md) göre güncel dahil kullanım ve hesap düzeyinde Actions 0 dolar durdurucusu kontrol edilir. [Workflow](../../.github/workflows/macos-build.yml).

## Açık kabuller

- H048-b için platforma ve mimariye özel sabitlenmiş motor ve doğrulanabilir süreç/loopback sahipliği, atomik ayar işlemleri ve çatışmayı koruyan geri alma gerekir.
- H048-c için paketlenmiş uygulamanın gerçek açılışı, başlat/durdur ve temiz state kabulü ile paket ve motor imzasının ayrı doğrulanması gerekir. Apple Silicon sonucu Intel, Windows 11 veya Linux'a aktarılmaz. Üretim yayını veya ücretli imzalama bu koşuların kapsamı dışındadır.
