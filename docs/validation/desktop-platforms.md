# 19 Kasım 2026 — Masaüstü platform testleri

Ürün adayı 10f32e3. İlk kullanım önceliği Windows 10 22H2 x64. macOS ve Linux için CircleCI Free kullanıldı. Windows 11 x64 doğrulaması daha sonra yapılacak.

| Ortam | Doğrulanan kapsam | Açık kabul |
| --- | --- | --- |
| Windows 10 22H2 x64, build 19045 | Güncel NSIS paketi kuruldu. 19.484 kaynak ve makbuz eşleşti. Native pencere açıldı, 0.44.0 motoru bir yerel HTTP isteğini yanıtladı. Uygulama normal kapandı ve test kurulumu temizlendi. | WebView2 bulunmayan temiz bir Windows kurulumu, kurulu güncel adayla gerçek istemci trafiği ve yayın adresinden indirme ayrı doğrulanır. |
| macOS 15 ARM64, CircleCI M4 Pro | 0.44.0 motoru yerel trafik testini geçti. ARM64 uygulama paketi üretildi, ad hoc imza ve paket envanteri doğrulandı. | Uygulama süreci başladı. AppleScript, pencereyi inceleme adımında iki koşuda −1712 zaman aşımına uğradı. Native pencere ve motor yaşam döngüsü kabulü açık. [Koşu 5](https://app.circleci.com/pipelines/circleci/KoDRDU6JmR6wiWsrCamPDH/b460f300-344a-4d50-8849-edc9e66c82b8/5/details) |
| Ubuntu 24.04 x64, CircleCI | TypeScript/Vite ve Rust derlemeleri geçti. ELF uygulaması Xvfb üzerinde görünür pencere açtı. Rust testlerinde 220 başarılı, 81 başarısız ve 2 atlanan test var. | İstemci yapılandırması, dosya sahipliği ve motor portu H049’da ele alınır. Installer ve motor yaşam döngüsü doğrulaması açık. [Koşu 6](https://app.circleci.com/pipelines/circleci/KoDRDU6JmR6wiWsrCamPDH/b460f300-344a-4d50-8849-edc9e66c82b8/6/details) |

## Windows 10 kurulumu

Güncel kaynak 234 Windows build girdisiyle doğrulandı. Önceki staging makbuzu eski overlay dosyalarına aitti. Payload’a sonradan eklenen iki Python bytecode dosyası arşivlendi ve sabit envanterle yeniden sahnelendi. 100 UTF-16 birimini aşan hedef reddedildi. Ayrı, geçerli hedefte kurulum ve normal çıkış geçti.

Kurulum dosyası 123.653.119 bayt. SHA-256 değeri 9725ecfd72005185542119684558f4fd27733a90acd746d7d05e291219cd64eb. 299 eşleşen girdiyle 227 frontend ve 478 Rust testi geçti. 24 opt-in Rust testi atlandı. [Derleme makbuzu](../../work/proofs/windows-install/runs/10dc9e1a-1db8-4450-9326-305c5d3a8181/recovered-build.json) · [Yerel kalite kapısı](../../work/proofs/windows-install/runs/10dc9e1a-1db8-4450-9326-305c5d3a8181/quality-gate.json) · [Kurulum sonucu](../../work/proofs/windows-install/runs/9bf2f318-01b9-4348-b810-b9c084d32453/result.json) · [Temizlik](../../work/proofs/windows-install/runs/9bf2f318-01b9-4348-b810-b9c084d32453/cleanup.json)

## CircleCI

GitHub App yalnızca Protocol272/headroom-desktop özel deposuna kuruldu. Başlangıçta Free plan 30.000 kullanılabilir kredi gösterdi. Pipeline elle tetiklenir. Oluşturulan tag tetikleyicisi kaldırıldı ve varsayılan platform değeri iş başlatmaz. İlk hatalar macOS’ta eksik Rust aracı ve Linux’ta şablon ayrıştırmasıydı. Sabit araçla ve düzeltilen ayrıştırmayla iki ortam da ürün adımlarına ulaştı. [Pipeline yapılandırması](../../.circleci/config.yml)
