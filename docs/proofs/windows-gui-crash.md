# H006-c — gerçek paketli GUI sahibinin beklenmedik kaybı

15 Kasım 2026. **Yerel dilim kabul edildi, ana H006 açık.** Windows 10 22H2 x64 build 19045 üzerinde gerçek Tauri release uygulaması kullanıldı. [Önceden belirlenen plan](../decisions/002-runtime.md#h006-c--gerçek-gui-sahibi-kaybı-için-yerel-kabul-planı) süreç ve portlar için 5000ms sınırı koyar. Ana ajan süreç/ayar sahipliği ve kabulü üstlendi.

| Senaryo | Gözlenen sonuç |
| --- | --- |
| Native pencere → Başlat → manager → paketli motor | İki gerçek GUI süreci de Stopped başladı, UI eylemiyle Headroom 0.41.0 Ready oldu. Sahipli üç Python process handle'ı, motor listener'ı, doğrudan `/readyz` sürümü ve GUI'nin sabit ingress sahipliği doğrulandı. |
| Yalnız GUI sahibinin zorla kapanması | Elde tutulan Popen Child handle'ına TerminateProcess uygulandı, GUI exit 1. Üç motor süreci 57,429ms, motor ve sabit ingress portları 58,299ms içinde kapandı. |
| Yanlış başarıyı önleyen kontrol | Gözlemci dış Job'ının handle'ı kapanış ölçümü boyunca açıktı. Başarıdan önce dış Job sonlandırması veya cleanup yoktu. Job boşaldıktan sonra kapatıldı. Yabancı listener doğru tanık baytlarını aldı. |
| Aynı izole profilde yeniden açılış | Stopped açıldı, endpoint kimliği ve portu değişmedi. Yeni runtime generation ile Başlat → Ready → Durdur → normal Çıkış geçti, GUI exit 0. |
| Son durum | İki kolda sahipli süreç 0, motor/sabit ingress/WebView debug portları kapalı. Test motor ayarı değişmedi, fixture model isteği 0. |

Bu deney H006-b'deki console-less fixture sahibinden ayrı olarak gerçek paketli GUI'yi kapsar. Native WebView sürücüsü görünen Başlat/Durdur/Çıkış kontrollerini kullanır, Ready sonrasında yalnız mevcut snapshot IPC'sini okur. Tauri ürünü veya runtime'a test hook'u eklenmedi. Dış Job, test ailesini sınırlar ve yalnız hata temizliği için kullanılabilir. GUI kaybında EOF ve üretim Job etkisi bu deneyde ayrıştırılmaz. Job mekanizmasını ayıran kontrol borusu/ek handle kolları [H006-b](windows-engine.md#h006-b--ani-sahip-süreç-kaybı-14-kasım-2026) kapsamındadır.

Uygulama mevcut kaynak ve staged Windows runtime ile ayrı `dev.headroom.desktop.gui-crash-a8e23e494d` kimliğinde `cargo build --release --locked --offline --features custom-protocol` yolundan üretildi. Release profili değiştirilmedi. Paketli runtime 22.332 dosya içerir, kaynak stage'in package receipt'i ayrıca kayıtlıdır. Bu sayı yalnız runtime ağacıdır. GUI PATH'i System32, motorunki paketli runtime ve System32 ile sınırlıydı. Testin gözlemci Python/Node araçları geliştirme ortamındadır, bu makine temiz Windows değildir.

Başarılı native koşuda 22.689 kaynak/test/config/payload/artefakt girdisi ve araç/OS önce/sonra eşleşti. Değişmeyen 344 ürün girdisi ve aynı araç/ortam için H009'un son 222 Node/462 Rust, 19 ignored, fmt/frontend/embedded build kapısı yeniden kullanıldı. Yeni benchmark'ın native kabulü ayrı kayıttır. Ürün kodu değişmediği için aynı ürün testleri tekrar edilmedi. [Programatik sonuç](windows-gui-crash.json), [inceleme](../reviews/H006-c.md).

İlk native denemede test sürücüsünün TCP tablo sınıfı yanlıştı. Gerçek GUI/motor Ready oldu, fakat port okuması TerminateProcess aşamasından önce hata verdi. Dış Job yalnız bu başarısız denemenin temizliğinde kullanıldı, süreç kalmadığı kontrol edildi. Koşu `native-7928c92f86` ve yalnız bu denemeye ait state, aynı yerel uygulama state kökünde byte-exact arşivlenerek korundu. Doğru sınıf pinned runtime'ın `TCP_TABLE_OWNER_PID_LISTENER` sabitinden alındı. Fazladan paket okumaları başlangıç/son tek taramaya indirildi. Derleme sürücüsünün eski baytları ve hash'i korundu; değişen gözlemci sürücüsü yeni native fingerprint'ine bağlandı. Ürün, payload, UI sürücüsü ve derleme komutu aynı kaldı. Nihai koşu `native-bb9c843392/result.json` geçti.

Tekrar üretim için mevcut pinned runtime stage'i ve son kalite kaydı gerekir. Proje kökünde yeni bir test kimliği oluştur:

```powershell
& work/proofs/packaging/artifacts/headroom-engine/runtime/python.exe -I -B bench/windows/gui_crash.py --prepare
# Yukarıdaki komutun bastığı candidate yolunu kullan.
& work/proofs/packaging/artifacts/headroom-engine/runtime/python.exe -I -B bench/windows/gui_crash.py --candidate work/proofs/windows-gui-crash/candidate-<id>
```

Mevcut başarılı kontrol ilgili girdiler değişmedikçe yenilenmez. Native kanıt, exe ve boş istemci/WebView profilleri `work/proofs/windows-gui-crash/` altındadır. Uygulamaya ait izole test state'i `%LOCALAPPDATA%/dev.headroom.desktop.gui-crash-<id>` altında korunur. Gerçek istemci hesabı/ayarına bağlanılmadı, model veya alt ajan seçimi değiştirilmedi, Actions/ücretli kaynak/yayın kullanılmadı.

Temiz Windows 10, ayrı Windows 11, Job ataması öncesindeki başlangıç yarışı, aktif stream/CCR verisi ve uzun süreli kararlılık bu deneyde doğrulanmadı. İki tekil süre gözlemi performans garantisi değildir. Token/fatura veya geliştirme sohbeti tasarrufu ölçülmedi. **Toplam 45/54 değişmedi.**
