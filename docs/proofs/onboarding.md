# H022 — İlk açılış ve hatırlanan bağlantılar

27 Ekim 2026. **Kabul edildi.** [Görev](../../TASKS.md), [root incelemesi](../reviews/H022.md). Kapsam Windows 10 22H2 x64 üzerinde embedded debug ürünüdür; installer, canlı sağlayıcı ve diğer işletim sistemi kabulü değildir.

| Senaryo | Gözlenen sonuç | Kanıt |
| --- | --- | --- |
| Açılış → salt okunur tarama → Genel bakıştan Bağlan erişimi | Motor Stopped, Claude bulundu; seçim dosyası, proje `.claude` ve genel kullanıcı ayarı oluşmadı. Applications açıldı; ayrıntılar kapalıydı. | Native `initial-pass.json` |
| İlk seçici ve iptal | Başlangıç izole home; iptal scope/tercihi değiştirmedi. İzole profilde eksik Desktop klasörü nedeniyle açılan Windows uyarısı test fixture'ında giderildi. | `picker-cancel-pass.json` |
| Unicode proje seçimi | Doğru `settings.local.json` hedefi gösterildi; yalnız proje metadata'sı kaydedildi. Tek adayın ön seçimi kurulum tercihi yazmadı; `.claude` oluşmadı. | `project-selected-pass.json` |
| Tek açık Bağlan | Önceden Stopped motor Ready oldu; fresh Preview/Apply proje `.claude/settings.local.json` dosyasını ve scoped active ownership kaydını oluşturdu. Kurulum açık eylemle hatırlandı; genel kullanıcı ayarı oluşmadı. | `connect-pass.json`; UI → manager → runtime/transaction kaynak yolu |
| Normal kapat/aç | Yeni gerçek süreçte aynı Unicode proje kapsamı ve Claude kurulumu döndü. Config byte'ları aynı; motor Stopped, ücretli doğrulama/otomatik Başlat yok. | `reopen-pass.json`, `closed-1.json` |
| Hatırlanan seçici ve iptal | Başlangıç aynı Unicode proje; iptal seçim byte'larını korudu. | `remembered-picker-cancel-pass.json` |
| Yanlış projede Geri al | UI yapılandırma çatışmasını gösterdi; asıl config ve ownership byte'ları değişmedi, yanlış projede `.claude` oluşmadı. | `wrong-project-undo-pass.json` |
| Doğru projede Geri al | Yalnız sahip olunan `ANTHROPIC_BASE_URL` kaldırıldı. Sentetik kullanıcı alanı ve not dosyası korundu, ownership Released oldu. | `undo-pass.json` |
| Yeni istemci → tekrar tarama | Çalıştırılmayan boş `opencode.cmd` manuel adayı taramadan sonra kaldı. Yeni istemci kendiliğinden bağlanmadı; OpenCode seçimi veya istemci config'i yazılmadı. | `new-client-rescan-pass.json`; bu, manuel keşfin restart kalıcılığı iddiası değildir. |
| Son kapanış/temizlik | Üç GUI normal exit0. Tercih hash'i geri döndü; başlangıçta olmayan seçim/fixture settings dosyaları kaldırıldı. Tüm ownership Released, owned süreç/listener0; farklı kurulu Headroom'un iki süreci yönetilmedi. | `closed-{1,2,3}.json`, `cleanup.json`, `metadata-restore-audit.json`, `process-cleanup-audit.json` |
| Durum ve sade UI | Bulunma, ayar kaydı ve trafik doğrulaması ayrı. Ready engine yanında Connection Not verified kaldı. Normal ekranda kısa açıklama ve ana eylemler; teknik ayrıntılar kapalı Gelişmiş bölümlerinde. | Gerçek OCU görüntü/AX gözlemi; C1/C2 kaynak incelemesi |
| Son tam yerel kapı | 97 Node PASS; 273 Rust PASS/3 ignored. fmt, frontend ve custom-protocol build PASS. QA sonrasında aynı 77 kaynak, 3 dist ve binary hash'i eşleşti. | `frontend-quality.json`, `final-quality.json`, `acceptance.json` |

Native dosyalar `work/proofs/onboarding/ui-runs/c5c8cfa4-3ad7-409d-8bc0-725a1d0cec2e/` altındadır. Toplu kabul ve hash'ler `work/proofs/onboarding/acceptance.json` içinde; ham hata/sağlayıcı içeriği saklanmadı. Araçlar Node25.2.1, pnpm11.25.0, Cargo/rustc1.92.0; offline Cargo cache, tek Rust test thread'i kullanıldı. Build warning sayısı6; hata0. Ürün gate'i yalnız son adayda bir kez koşuldu; belge kapanışı için tekrar edilmedi.

A1/A3/B1/B2/C1/C2 ara kayıtları ayrı fingerprint'leriyle aynı proof kökünde korunur. A1 eksik ebeveynle pasif Preview; A2 kullanılmayan dizin silme API'si çıkarılmadan önceki tarihsel primitive; A3 gerçek TxF Apply/abort/Drop/Undo; B1 boundary/reparse düzeltmeleri; B2 seçimlerin keşif/context/IPC/UI bağıdır. Son kapı bu ara sonuçların son kaynaklarını kapsar. CodeGraph sync/dar sorgusu `codegraph-final.json` içindedir; çalışma zamanı veya tasarruf kanıtı sayılmaz.

Geri alma alan sahipliği sözleşmesini sürdürür: Headroom yönlendirmesi kaldırılır, kullanıcı ekleri korunur. Boş ayar dosyası ve `.claude` kalabilir. Abort/commit öncesi hata ise yeni yolu transaction ile geri çevirir. Native SetFolder, boundary/reparse/identity, seçim/depo hataları ve conflict yolları kaynak+odaklı testlerle incelendi.

Runtime hata metinleri temel paket, disk/yazma izni, durumu yenile ve Başlat yolunu açıklar; gerçek indirme/devam/onarım H037/H042'dedir. Hata üretmek için kullanılan workspace/settings.json bu doğrulama yolunun girdisi değildi: motor Ready oldu, dosya geri yüklendi ve **bu deneme kurtarma PASS sayılmadı** (`recovery-probe-not-counted.json`). Disk dolması veya indirme kesintisi denenmedi.

Bu akışta hesap girişi, CLI/model isteği, ücretli doğrulama veya model/efor/ajan değişimi başlatılmadı. Yerel sink POST0 gözlemi ağ izolasyonu veya sağlayıcı trafiği kanıtı değildir; workspace base-url override'ı kullanılmamıştır. Canlı auth/model/subagent ve Win11/macOS/Linux doğrulaması yapılmadı. Kapsam metni desteklenen aynı rotadaki model/ana-alt ajanları açıklar; actual session adoption iddia etmez. Geliştirme token tasarrufu ölçülmedi. H019 ek CLI hataları H008'de açık kalır.
