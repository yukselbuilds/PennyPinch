# H038 — Kaldırma ve veri tercihleri

9 Kasım 2026. H038 yerel kabulü tamamlandı, 36/51. Kullanıcı H037’nin dış kabulü açıkken bu göreve geçişi onayladı. [Plan026](../decisions/026-windows-uninstall.md), [kanıt](../proofs/uninstall.json).

Kurulu uygulama kaldırılmadan önce aynı kullanıcının açık Headroom örnekleri kontrol edilir. Ardından mevcut sahiplik kayıtları üzerinden istemci yönlendirmeleri geri alınır. Bir bağlantı geri alınamazsa kurulum korunur. Kullanıcının başka alanlara yaptığı düzenlemeler mevcut işlem sözleşmesiyle korunur.

Kaldırıcı yerel geçmişi ve tercihleri varsayılan olarak saklar. Silme kutusu seçilirse uygulamanın LocalAppData kökü silinir. Kilitli veride hata oluşabilir ve bu sırada silinmiş veriler geri getirilemez. Dışa aktarılan dosyalar ve istemci veri klasörleri kendi yerlerinde kalır.

Windows 10 build 19045 üzerinde dört odaklı Rust testi geçti. Kaynak ve araç girdilerinin önce/sonra değerleri eşleşti. Açık uygulamalar ile kaldırıcı birbirini dışladı. Mevcut kilit dosyasındaki yabancı veri korundu. Kilitli yerel veri için silme hatası ve yeniden deneme doğrulandı. Üç istemcide kullanıcı düzenlemeleri korundu ve kilitli istemcinin geri alması yeniden denemede tamamlandı. [Odaklı test kaydı](../../work/proofs/windows-uninstall/focus-655bfb8a4e9e46a78d2c62c13ac24825/result.json).

Kurulu adayın uygulama kimliği `dev.headroom.desktop.h038-qa` ve adı Headroom H038 QA. İstemci fixture’ları `%LOCALAPPDATA%\HeadroomDesktop-QA-H038` altında ayrı bir koşu dizininde tutulur. Normal kullanıcı ayarları fixture olarak kullanılmaz. [Derleme kaydı](../../work/proofs/windows-uninstall/qa-build.json).

Windows 10’da gerçek kurulu uygulama fareyle kontrol edildi. Motor başlatıldı, Codex fixture’ı bağlandı, ürün içi kaldırma ile kayıt released oldu ve tekrar bağlandı. Normal çıkış kodu 0. Gerçek kaldırıcı açık uygulamada, kilitli ayar dosyasında ve kullanıcı endpoint çatışmasında kod 2 ile durdu. Geçmişi saklayarak kaldırma kod 0 ile tamamlandı. Kullanıcının fixture model/reasoning/yorum değişiklikleri korundu. Geçmiş ve tercih dosyaları yerinde kaldı. [Koşu kayıtları](../../work/h038-test/7b843249/keep.json).

Altı komutlu son yerel kapı başarılı. Python supervisor/paket testleri, 173 arayüz testi, TypeScript/Vite, Rust fmt, 392 Rust testi ve custom-protocol derlemesi geçti. Dokuz Rust opt-in testi ignored. Kaynak ve araç parmak izleri önce/sonra eşleşti. [Yerel kapı](../../work/proofs/windows-uninstall/local-quality.json).

İkinci kurulumda motor ve Codex bağlantısı arayüzden yeniden başlatıldı, uygulama normal yolla kapatıldı. Gerçek kaldırıcıdaki silme kutusu varsayılan olarak boştu. Kutuyu işaretleyip geçmiş dosyası kilitliyken kaldırma denendi. İstemci yönlendirmesi geri alındı, silme hata verdi ve kurulum korundu. Kilit bırakıldıktan sonra aynı kaldırıcıyla tekrar silme seçildi. Arayüz başarı gösterdi ve çıkış kodu 0 oldu. QA kurulum ve veri kökü kaldırıldı. Üç dış istemci config’inin önce/sonra hash’leri eşleşti. [Kilit kontrolü](../../work/h038-test/7b843249/delete-locked-postcheck.json), [silme kontrolü](../../work/h038-test/7b843249/delete-postcheck.json).

Kabul betiği 40 kontrolü aynı aday kimliğine bağladı. 212 ürün/araç girdisi, derleme ve test kayıtları, iki normal çıkış ve installer kimliği eşleşti. Tanı portları ve test pencereleri kapalı. Tam adayın kabulü için başarılı testler tekrar çalıştırılmadı. [Kabul kaydı](../../work/proofs/windows-uninstall/acceptance.json).

Bu kayıt Windows 10 build 19045’te ayrı QA uygulama kimliğine aittir. Üç adaptörün config/rollback davranışı Rust fixture’larıyla, kurulu uygulamanın bağlantı yolu Codex fixture’ıyla sınandı. Temiz Windows, Windows 11 ve üretim kimliğiyle kurulum H037’nin dış kabulünde, `/UPDATE` yolu H042’de kalır. Silme güvenli adli imha iddiası taşımaz. Sıfır baytlık kardeş `.desktop-use` eşzamanlılık dosyası korunur.
