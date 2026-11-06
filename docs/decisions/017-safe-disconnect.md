# H029 — güvenli bağlantı kaldırma

6 Kasım2026. Durum: yerel kabul tamamlandı; H030 başlamaz. [Kanıt](../proofs/safe-disconnect.md), [inceleme](../reviews/H029.md).

## Sınır ve sahiplik

Ana ajan bağlantı/ayar sahipliği ve aktif stream kabulünü üstlenir. Tek Luna xhigh çalışanı yalnız kesinleşmiş onay arayüzünü uygular. H022'nin mevcut `undo_connection` → `PlanManager` → `FileTransaction`/ownership yolunu kullanırız; ikinci bir config yazıcısı veya yeni undo protokolü eklenmez. Model/auth/reasoning ve diğer istemci ayarları değişmez. Yeni hesap, ücretli sağlayıcı, yayın veya global ayar bu görevin kapsamında değildir.

Bağlantıyı kaldırma yalnız seçili istemci/kapsamın Headroom tarafından eklenen endpoint alanını geri alır. Motoru durdurmaz; başka istemciler ve başlamış istekler aynı motor nesliyle devam edebilir. Açık istemci süreçleri eski ayarı bellekte tutabilir; yeni isteklerin ayarı kullanması için istemciyi yeniden açmak gerekebilir. Doğrudan bağlantıya otomatik geçiş veya açık sohbet benimsenmesi vaat edilmez. Motoru durdurmak ayrı, bütün o motora bağlı istekleri etkileyen mevcut eylemdir.

## Korunan sözleşmeler

- Onaydan önce yazım yok; iptal/ESC klavye odağını geri getirir. Bekleyen mutasyonda ikinci komut, kapanış veya olumlu sonuç gösterilmez.
- Canonical scope/path ve owner/revision/fingerprint karşılaştırması, atomik config+ownership commit ve çatışmayı koruyan geri alma aynen korunur. İstemcinin sonradan değiştirdiği endpoint korunur; başarısız/belirsiz sonuçta kayıt yeniden okunmadan yeniden kaldırma yapılmaz.
- Önce ingress yeni route kabulünü kapatır; mevcut socketler generation'a bağlıdır. Stop, pinned engine graceful shutdown ve bounded drain sonucuna dayanır; zorla kapanış başarı/graceful gibi sunulmaz. Tek request yeniden gönderilmez.

## Kabul senaryoları

1. Üç adaptörde sahipli bağlantı → onay/undo: yalnız sahipli alan kalkar, kullanıcı ekleri ve diğer istemci dosyaları korunur; endpoint/sahiplik çatışmasında byte'lar ve active kayıt değişmez. Yeniden açılmış manager eski sahipliği doğru yorumlar. Native arayüz onay/iptal/bekleme/başarı/hata, TR/EN, klavye ve dar/ölçekli görünümü kapsar.
2. Gerçek pinned motor + izole loopback upstream'de gated aktif SSE → bağlantı kaldırılır: motor/nesil ve diğer istemci korunur, aynı POST tek kez tamamlanır. Ayrı aktif SSE → Stop → gate bütçe içinde açılır: terminal frame tam gelir, stop/graceful ve owned cleanup doğrulanır. İzole motor settings dosyası custom loopback upstream'i sağlar; kullanıcı ayarı veya vendor kaynak değiştirilmez.
3. Aynı yolda gate açık bırakılır → mevcut shutdown bütçeleri dolunca eksik/timeout sonucu görünür; terminal başarı ve replay yok, owned süreç/socket cleanup ve bağımsız listener'ın canlılığı doğrulanır. Önceden belirlenen sınırlar: normal gate stop başladıktan sonra en çok2s içinde açılır; stuck gate en çok60s bekler. Mevcut20s engine graceful/30s controller/90s actor bütçeleri kabulü gizlemek için değiştirilmez. Bu bir performans veya uzun süreli yük kabulü değildir.

## Kaynak ve doğrulama

Uygulama sırasında eksik tiktoken vocab yüklemesinin soğuk ilk isteği bloke ettiği ve pinned uvicorn'ın iptal edilmiş stream sonrasında exit0 verebildiği görüldü. Altı checksum'lu yerel veri dosyası/yedi offline kodlama hazırlığı ve helper'ın içeriksiz request-drain-timeout flag'i eklendi. Süreler/model/auth/reasoning/pause/CCR politikası değişmedi; [kaynak ve hazırlık](../development/tokenizers.md). Normal gate için2s fixture sınırı korunur; gerçek stop sınırı stable ingress'te yeni route'un kapanmasıdır. Private engine listener'ın2s'de kapanacağı bir SLA eklenmez.

`src/features/integrations/ConnectDialog.tsx`, yeni `DisconnectDialog.tsx`; native `main.rs` undo handler; `integrations/connection_actions.rs`, `transaction.rs` ve testleri; `runtime/manager.rs`, `ingress_relay.rs`; pinned `runtime/desktop_{controller,helper}.py` ve engine `settings_store.py`/`proxy/server.py`.

Önce odaklı üç-adaptör ownership testleri ve opt-in pinned-engine SSE testi, sonra native UI incelemesi. Kaynak ve ortam fingerprint'leri aynı koşuda önce/sonra kaydedilir; mevcut proof helper'ı uyarlanır. Son incelenmiş adayda WORKFLOW.md'nin beş komutlu yerel kalite kapısı bir kez çalıştırılır. Kanıt `work/proofs/disconnect/`, kısa sonuç `docs/proofs/safe-disconnect.md`, inceleme `docs/reviews/H029.md`. Başarılı eski testler somut girdi değişimi olmadan tekrarlanmaz. Windows10 yerel kanıtı diğer OS/gerçek sağlayıcı/installer kabulü değildir. Geliştirme tasarrufu ölçülmezse oran yazılmaz.
