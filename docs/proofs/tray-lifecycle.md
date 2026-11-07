# H030 — sistem tepsisi ve kapanış kabulü

7 Kasım2026; **H030 tamamlandı,29/51**. Plan [018](../decisions/018-tray-lifecycle.md), inceleme [H030](../reviews/H030.md). Yerel ortam Windows10 x64 build19045. Ücretli Windows11 koşusu önceki H047/karar006 istisnasıyla skipped/not_tested; bu sayfa Windows11/macOS/Linux/installer veya canlı sağlayıcı desteğini kanıtlamaz.

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
| Motor kaybı, görünür hata, native ESC/Tab/Enter ve çıkış | PASS, exit0 | `native-runs/de1769d7-43c7-4cc4-be19-9337a446a6b0/result.json`, `crash-settled.json`, `native-keyboard.json`: pinned engine executable kimliği, üçlü parent zinciri ve test job sahipliği doğrulanıp yalnız leaf engine sonlandırıldı. UI failed/engine_not_ready gösterdi; ESC onayı iptal etti; Tab gerçek onay düğmesini odakladı, Enter owned kapanışı tamamladı. |
| Native tepsi menüsü Aç/Duraklat/Devam et | PASS, kullanıcı denemesi ve bağımsız son durum | Kullanıcı yönergedeki Başlat → X → tepsiden Aç → Duraklat → Devam et adımlarının tümünü yaptığını bildirdi. Ana ajan açık native pencerede motor Açık, etkin Duraklat ve tepsi metnini okudu; çıkış modalı yok. `manual-final.json` ve `closeout.json` kaynak/binary/dist ve araç/OS eşleşmesini bağlar. Menü tıklamaları kullanıcı tarafından yapıldı; ajan bunları doğrudan gözlemlemiş sayılmaz. |

Otomatik native koşular yalnız loopback sahte upstream ve test job'unu kullanır; bu koşularda gerçek provider/model çağrısı yoktur. Projeye ait development runtime/metrics dizinleri doğrulanmış proje içi yollara geri alınabilir biçimde taşınıp koşudan sonra byte fingerprint'leriyle geri yüklenir; auth, kullanıcı client config'i, ownership, tercih ve diğer projeler değiştirilmez. Token/savings değeri bu testten gerçek tasarruf diye sunulmaz. Kısa koşu uzun süreli kararlılık değildir.

`acceptance-1.json`,43 programatik bağlama kontrolüyle tüm ilgili güncel kaynak/config, son ürün binary/dist, original verifier ve UI kanıtlarının eşleştiğini doğruladı. Başarılı kapılar tekrar çalıştırılmadı. İlk crash-fixture süreç seçimi Windows `\\?\` prefix'ini düz metin yoluyla karşılaştırdığı için güvenli guard'da durdu; süreç sonlandırılmadan job kapatıldı ve state geri yüklendi. Son harness executable file kimliğini `samefile`, açık handle, parent zinciri ve job üyeliğiyle kontrol eder; başarısız hazırlık koşuları PASS sayılmadı. Normal/timeout/crash final koşuları ayrı ayrı source/artefact before=after kaydeder.

Otomatik koşular tamamlandı ve geçici state geri yüklendi. Son kullanıcı menü kontrolü normal development uygulamasında yapıldı; kullanıcı başlangıçta Durduruldu olan motoru Başlatıp adımları tamamladı. Pencere Açık/Duraklat durumunda bırakıldı. Bu son pencere izole sahte-trafik koşusunun parçası değildir; arka planda test otomasyonu çalışmaz ve menü kontrolü canlı sağlayıcı doğrulaması sayılmaz. Sky screenshot timeout/click geometry hatası nedeniyle gerçek menü adımları kullanıcıya bırakılmıştı; erişilebilirlik ağacından yapılan son bağımsız okuma `manual-final.json` içinde yalnız gereken metadata ile kaydedildi.

`closeout.json` ile güncel kaynak/binary/dist, aynı araç zinciri, Windows build ve doğrulayıcı Python ortamı önceki PASS ile eşleşti. Ürün kodu değişmedi;146 frontend/356 Rust ve derleme kontrolleri tekrar çalıştırılmadı. Son kabul ve belge kapanışı ana ajan tarafından yapıldı; yeni devir veya geliştirme tasarruf ölçümü yok.

H030 kapatıldı; H031 başlamadı. Ana görev sonunda duruldu.
