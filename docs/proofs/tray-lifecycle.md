# H030 — sistem tepsisi ve kapanış kabulü

7 Kasım2026; **aktif,28/51 aynı**. Plan [018](../decisions/018-tray-lifecycle.md), inceleme [H030](../reviews/H030.md). Yerel ortam Windows10 x64 build19045. Ücretli Windows11 koşusu önceki H047/karar006 istisnasıyla skipped/not_tested; bu sayfa Windows11/macOS/Linux/installer veya canlı sağlayıcı desteğini kanıtlamaz.

## Uygulanan davranış

- Tepsi kurulmuşsa X/Alt+F4 pencereyi gizler; motor ve sabit ingress yaşamaya devam eder. Tepsi yoksa gizlemek yerine çıkış onayı istenir.
- Tepsi menüsü Aç, gerçek motor durumu, Başlat/Duraklat/Devam et ve Tamamen çık içerir. Durum polling'i yalnız bellek snapshot'ını okur; ölçüm veya model isteği üretmez. Pause/Resume generation/revision CAS ile gerçek ack bekler.
- Tamamen çıkış, yönlendirme ayarlarının geri alınmayacağını ve bağlı istemci/süren yanıt etkisini açıklar. Default Vazgeç, ESC ve focus geri dönüşü korunur. Client-config işlemi sürüyorsa çıkış busy döner; stale token yeni writer/shutdown başlatmaz.
- Onay, yeni config işlerini terminal kapatır; mevcut owned kapanış zinciri yalnız bir kez çalışır. H029 stop/drain/no replay bütçeleri değişmez. Sonuç temiz değilse exit1; crash/force-kill için kesintisiz bypass veya otomatik config undo vaat edilmez. H031 autostart yoktur.

## Doğrulama kayıtları

Kanıtlar `work/proofs/tray/` altında; ham auth/prompt/HTTP gövdesi tutulmaz. Kaynak/config, araç/ortam, komut ve önce/sonra fingerprint'leri doğrulama adımlarında programatik kaydedildi.

| Senaryo | Sonuç | Kanıt / sınır |
| --- | --- | --- |
| Nonce/cancel/stale/busy, tray yokken hide reddi, ack projection ve gerçek FS exit hold | PASS | Son aday `final-1.json`:146 frontend,356 Rust PASS;7 opt-in ignored. Client ayar baytları korunur. |
| Son aday TypeScript/Vite/native build ve fmt | PASS | `build-2.json` frontend; sonraki yalnız native değişiklikler `native-build-4.json`; tam son Rust/fmt `final-1.json`. Eşleşen kaynak kanıtları tekrar çalıştırılmadı. |
| Component TR/EN/açık/koyu/1280,819,683,507; ESC/focus/Tab/parent busy/duplicate pending/error ve gizli sidebar | PASS, izole fixture | `ui-d655c49c-795d-44f3-9822-8dcb4f76ca8f/result.json`;6 davranış grubu, tek StrictMode aboneliği. `tr-dark-507.png` gözle incelendi. Native IPC yerine açık mock kullanır. |
| Native yeni event izni ve ilk tray state okuması | PASS | Gerçek `get_desktop_state` ve erişilebilirlik ağacı, tepsi kullanılabilir/idle metnini doğruladı. İlk izinsiz aday kabul edilmedi. |
| Gerçek native açık SSE sırasında Alt+F4 | PASS | `native-runs/fff9de67-e194-46bf-88f7-3de65bb997ee/sky-active-hide.json`: Sky envanterinde0 Headroom penceresi; process/engine generation/endpoint aynı, client ilk baytı almış, terminal gate kapalı, yalnız1 upstream POST; yabancı fixture listener canlı. İlk envanter pencere kapanırken okunmuştu; settled envanter ayrı doğrulandı. |
| Gerçek native exit request/cancel/stale ve normal aktif akışta owned kapanış | PASS, tek istek/tek POST, terminal tam, exit0 | `native-runs/610ebbd4-091f-4135-9c02-e8baf91508fc/result.json`; `drain-before-release.json`: yeni stable-ingress isteği reddedilirken eski SSE açık; ancak sonra upstream gate açıldı. Cancel/stale aynı generation/endpoint'i korudu. Önceki kurulum denemeleri bu final koşunun kanıtı değildir. |
| Tamamlanmayan aktif yanıtta açık çıkış → exit1/no replay/owned cleanup | PASS | `native-runs/fff9de67-e194-46bf-88f7-3de65bb997ee/result.json`: başlık/ilk frame geldi, terminal yok; exit1, yalnız1 POST; test aracının zorlaması yok, normal state geri yüklendi. |
| Motor kaybı, görünür hata, native ESC/Tab/Enter ve çıkış | PASS, exit0 | `native-runs/de1769d7-43c7-4cc4-be19-9337a446a6b0/result.json`, `crash-settled.json`, `native-keyboard.json`: pinned engine executable kimliği, üçlü parent zinciri ve test job sahipliği doğrulanıp yalnız leaf engine sonlandırıldı. UI failed/engine_not_ready gösterdi; ESC onayı iptal etti; Tab gerçek onay düğmesini odakladı, Enter owned kapanışı tamamladı. Native tray menüsündeki render hâlâ aşağıdaki etkileşim kontrolüne bağlıdır. |
| Native tepsi menüsü Aç/Duraklat/Devam et | Bekleniyor | Sky screenshot: `FrameArrived timed out`, yeniden seçimden sonra `window capture timed out`; click: `coordinate input geometry is unavailable`. Erişilebilirlik ve Alt+F4 çalışır; Explorer/tepsi hedefi envanterde sunulmuyor. Kullanıcıdan kısa gerçek menü kontrolü istendi; cevap gelmeden PASS olmaz. |

Tüm native koşular yalnız loopback sahte upstream ve test job'unu kullanır; gerçek provider/model çağrısı yoktur. Projeye ait development runtime/metrics dizinleri doğrulanmış proje içi yollara geri alınabilir biçimde taşınıp koşudan sonra byte fingerprint'leriyle geri yüklenir; auth, kullanıcı client config'i, ownership, tercih ve diğer projeler değiştirilmez. Token/savings değeri bu testten gerçek tasarruf diye sunulmaz. Kısa koşu uzun süreli kararlılık değildir.

`acceptance-1.json`,43 programatik bağlama kontrolüyle tüm ilgili güncel kaynak/config, son ürün binary/dist, original verifier ve UI kanıtlarının eşleştiğini doğruladı. Başarılı kapılar tekrar çalıştırılmadı. İlk crash-fixture süreç seçimi Windows `\\?\` prefix'ini düz metin yoluyla karşılaştırdığı için güvenli guard'da durdu; süreç sonlandırılmadan job kapatıldı ve state geri yüklendi. Son harness executable file kimliğini `samefile`, açık handle, parent zinciri ve job üyeliğiyle kontrol eder; başarısız hazırlık koşuları PASS sayılmadı. Normal/timeout/crash final koşuları ayrı ayrı source/artefact before=after kaydeder.

Otomatik koşular tamamlandı ve geçici state geri yüklendi. Son kullanıcı menü kontrolü için normal development uygulaması açık, motor başlangıçta Durduruldu; Başlat sonrasında tepside Aç/Duraklat/Devam et denenebilir. Bu son pencere izole sahte-trafik koşusunun parçası değildir ve arka planda test otomasyonu çalışmaz.

H030 henüz kapatılmadı; H031 başlamadı.
