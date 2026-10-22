# H018 işlem bağı tasarımı

22 Ekim2026. **C1–C3 uygulanmış, root incelemesi/statik UI ve D native kabulüyle doğrulanmış bağ.** Gerçek komut/DTO özeti [sözleşmede](contracts.md), son kaynak fingerprint'leri [yerel gate](../work/proofs/config-transaction/final-quality.json) içindedir. [Karar005](decisions/005-config-transaction.md), [kabul matrisi](validation/config-transaction-matrix.md).

## Yetki ve güncellik

Apply isteği yalnız schema/request UUID, plan UUID ve expected config fingerprint taşır. Renderer dosya yolu, serbest URL, model, config gövdesi veya bütün planı geri yollamaz. Server kendi64 plan/300s lease deposunu kullanır. Preview/apply/undo aynı işlem guard'ını paylaşır; preview kapasite hatası gerçek yazım başarısı sayılmaz. Lease tek kullanımlıdır; blocked/stale/replay/expired plan dosyaya yazmaz.

Apply önce güncel seçili kurulum/tarama ve stable endpoint kimliği/portunu doğrular. Plan hedefi environment köklerinden backend'de türetilir. TxF içindeki held snapshot ile kaynak/presence/content/Windows file ID/canonical target dahil H017 fingerprint'i kurulur. Eski preview'nin identity kontrolü atlanmaz. JSON/JSONC alternatif slotunun yokluğu aynı transaction'da rezerve edilir. Config'in kendi üst dizini zaten var olmalıdır; yalnız uygulamaya ait ownership dizini lazy oluşturulabilir.

H017 present fingerprint'i Windows canonical target path'i (extended prefix dahil), missing fingerprint'i özgün lexical target path'i içerir. Aynı fingerprint sözleşmesi C'de korunur; missing config için yeni TxF file ID önceden varmış gibi hash'e katılmaz. Ownership path binding ise kalıcı config yolunu inode'dan bağımsız bağlar.

Store constructor var olan güvenli boundary ister; kurulumun ilk açılışında uygulama alt dizininin var olduğu varsayılmaz. Debug'da mevcut proje kökü + bounded `work/app-state/ownership` relative bileşenleri; release'te mevcut LocalAppData parent + uygulama identifier/ownership bileşenleriyle hedef `app_local_data_dir()/ownership` sağlanabilir. Setup dosya yaratmaz; bütün app-owned relative bileşenler lazy create sırasında pinli oluşturulur. Kök hata verirse preview çalışabilir, mutation sabit hata döner; app setup paniklemez.

`config_io` altında child action modülü, parent'ın private lease/guard/roots yardımcılarını kullanabilir. Bağımsız ikinci runtime/discovery/plan yöneticisi veya renderer'a geniş path API'si gerekmez. Held transacted writer açıkken ordinary read helper çağırmak paylaşım hatasına yol açabileceğinden karşılaştırma doğrudan transaction snapshot'ından yapılır.

## Sahiplik ve geri alma

Bellekte patch sonucu aynı endpoint/no-op ise sahiplik yaratılmaz. Mevcut uygulama kaydı varsa yeni kayıtla sessizce ezilmez; root/source/owner/revision/fingerprint tutarlılığı doğrulanır. Config ve record aynı kernel transaction'da commit edilir. Commit cevabı belirsizse 'dosya değişmedi' sonucu uydurulmaz; sabit belirsiz sonuç ve yeniden oku akışı gerekir. Otomatik write retry yok.

Undo isteği client enum, ownership UUID, expected revision ve expected record fingerprint ile sınırlıdır. Yol/önceki config/URL göndermez. Güncel config source/root binding ve record doğrulanır; endpoint hâlâ yönetilen değer ise yalnız bu alan kaldırılır. Önceki after-file ID veya bütün config hash'i eşitliği undo için mutlak şart değildir; normal editörün atomic save'i ve ilgisiz kullanıcı düzenlemeleri desteklenmelidir. Yönetilen alan değişmiş/silinmişse yeni kullanıcı değeri korunur; conflict veya açık no-op sonucu gerekir. Config dosyası bütün olarak geri yüklenmez/silinmez.

Undo geçmiş sahipliği kaldırdığı için yeni plan, aktif model motoru, yeni istemci sürüm allowlist'i veya halen kurulu executable gerektirmez. Kalıcı kayıt yeniden açılışta salt okunur bulunabilir; environment/root değişmişse kayıt başka hedefe uygulanmaz. UI kurulum bulamasa da sahipliği olan kullanıcıya Undo erişimi sunmalıdır.

## Typed IPC ve Graphite

Üç dar eylem: apply, undo, client başına ownership snapshot. Yanıtlar schema/request/client korelasyonu, canonical UUID, safe integer, fixed enum, doğru retryable/message key ve kesin alan listesiyle TS tarafında yeniden oluşturulur. Raw serde/OS error, path, auth, config, hook veya eski URL gönderilmez. 'Applied/owned' kaydı config'in veya trafiğin halen etkin olduğunun kanıtı değildir.

Uygula yalnız güncel, desteklenen ve süresi dolmamış native preview'de etkinleşir. Yazım sürerken modal Close/Escape veya parent navigasyonunun sonucu sessizce yetim bırakması engellenir. Tamamlanınca kaydedilmiş yönlendirme, istemcinin yeniden başlaması ve canlı bağlantının doğrulanmamış olması sade dille görünür. Headroom kapalıyken kalıcı yönlendirmeyi taşıyan portun hizmet veremeyeceği, kullanıcının kararına yardımcı olacak kısa notla belirtilir; otomatik whole-file undo yapılmaz.

Undo yalnız uygulamanın sahiplik kaydı olan kapsam için sunulur; aynı endpoint'i kullanıcının kendisi eklemişse Undo yetkisi üretilmez. Çakışmada force overwrite yok; kullanıcıya güncel durumu yeniden okumayı sağlayan eylem sunulur. Browser-only preview gerçek mutation taklit etmez. TR/EN, koyu/açık tema,760×620 minimum ölçü ve focus/Escape geri dönüşü korunur.

Mevcut connection dialog'u yeniden kullanılır: native kullanıcı, kurulum bulunmasa da bağlantı yönetimini açıp ownership snapshot'ını okuyabilir; yalnız yeni Preview/Uygula seçili kurulum ister. Ownership paneli source/yerel port/durum ve Undo/Yeniden oku sağlar. Modal yazım sırasında Close/Escape'i engeller; dar ekranda scroll body/fixed footer korunur. TR/EN koyu/açık,760×620/1280×800 statik kontrolleri ve gerçek native eylemler kabul edildi.

Son kabul gerçek UI eylemi → Tauri → aynı manager → transaction/config/ownership zincirini disposable native köklerde izler. Sentetik npm manifestleri version-gated kod yolunu açabilir; bu gerçek istemci/auth/model/alt ajan uyumluluğu değildir. Bunlar H019–H021'de ayrı doğrulanır.
