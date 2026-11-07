# H030 — pencere, tepsi ve açık çıkış

7 Kasım2026. Durum: H030 tamamlandı; otomatik native kabul, kullanıcının gerçek tepsi menü denemesi ve ana ajanın son native durum/fingerprint gözlemi bağlandı.29/51; H031 başlamaz. Ana ajan süreç/ayar sahipliği ve kapanış kabulünü üstlendi. Tek Luna xhigh sabit sözleşmeli frontend dilimini tamamladı; root inceleme/düzeltme/native kabulü devraldı. Kanıt [tray-lifecycle](../proofs/tray-lifecycle.md).

## Amaç ve kapsam

X, native tepsi kaydı hazırsa yalnız ana pencereyi gizler; motor, sabit ingress ve kolektör çalışır. Tepsiden Aç/sol tık pencereyi yeniden gösterir. Tepsi kurulamazsa pencere gizlenmez, X açık çıkış onayını ister. Tepsi durumu engine ack'inden türetilir; bilinmiyor hazır sayılmaz. Menü temel eylemleri Aç, Başlat/Duraklat/Devam et ve Çıkış'tır. Tray Stop eklenmez; UI'daki uyarılı motor Stop ayrı kalır. Windows başlangıcı/tercihi, otomatik bypass/onarım, installer ve H031+ işleri bu göreve eklenmez.

Tamamen çıkış her zaman kullanıcıya motor/tepsinin kapanacağını, kayıtlı yönlendirmelerin geri alınmayacağını, bağlı istemcilerin hata alabileceğini ve süren yanıtların tamamlanamayabileceğini açıklar. İlk odak Vazgeç'tir. İptal hiçbir motor/config değişimi yapmaz. Onayda owner/revision korumalı mevcut config işlemi sürüyorsa busy döner; çıkış başlatılmaz. Onay başarıyla alınınca PlanManager yeni işlemlere terminal olarak kapatılır; ardından mevcut ExitGate ve verifier → collector → RuntimeManager shutdown zinciri yalnız bir kez çalışır. Sonuç temiz değilse exit1; graceful diye sunulmaz. H029 drain/no-replay/owned cleanup sözleşmesi ve bütçeleri değişmez.

Pencere kapanışı, duraklatma, bağlantıyı kaldırma ve process çıkışı farklı eylemlerdir. Force-kill/OS kapanışı/crash kesintisiz bağlantı veya otomatik direct-upstream dönüş garantisi değildir. Yabancı listener veya süreç kapatılmaz. İlk sürüm tepsi kullanımı sabittir; kullanıcıdan autostart/hesap/global ayar izni istenmez veya böyle bir ayar yazılmaz.

## IPC ve UI sözleşmesi

`get_desktop_state(requestId)`, `request_desktop_exit(requestId)`, `cancel_desktop_exit(requestId, exitToken)`, `confirm_desktop_exit(requestId, exitToken)`; aynı Tauri camelCase argument yöntemi. UUID'ler canonical lowercase; nonce yalnız process belleğindedir.

Snapshot tam alanları: `schema_version:1`, `request_id`, `status:"snapshot"`, `tray_available:boolean`, `exit_state:"idle"|"confirmation_required"|"shutting_down"`, `exit_token: UUID|null`. Token yalnız confirmation_required durumunda doludur. Error tam alanları: `schema_version:1`, `request_id`, `status:"error"`, `error:{code,retryable,message_key,diagnostic_id:null}`. Kodlar `validation_failed`(false), `state_conflict`(true), `busy`(true); message_key `desktop.<code>` olur. Bozuk correlation için diğer protokollerdeki zero UUID kullanılır.

`headroom:desktop-change` içeriksiz değişim bildirimidir. Frontend önce dinler, sonra snapshot okur; yeniden açılış/kaçan olay snapshot'tan toparlanır. Confirmation token tam eşleşmeden cancel/confirm olmaz; stale/tekrarlı istek process'i kapatmaz. Pending/başarısız IPC'de iyimser başarı yok. Üstteki çıkış modalı açıkken alttaki dialog odak tuzağına müdahale etmez. TR/EN ve mevcut DESIGN.md uygulanır.

## Kabul ve doğrulama

1. Gerçek native X/gizle → tepsi Aç/sol tık → aynı process, engine generation, endpoint ve açık gated SSE; terminal frame tam, tek POST. Tepsi state/primary ack ve bir ikinci headroom-dışı fixture listener'ın canlılığı doğrulanır. Pencere kapatmak runtime shutdown başlatmaz.
2. Çıkış isteği/iptal/eski nonce/config busy → ayar ve süreçler korunur. Onay → yalnız bir shutdown; normal tamamlanan akışta exit0, stuck akışta eksik tamamlanma/exit1/no replay/owned cleanup. Motor hatası tepside hazır görünmez; pencere geri açılabilir. Tepsi kurulamayan koşul pencereyi gizlemez.
3. TR/EN, klavye/ESC/odak, dar görünüm/WebView125/150; gerçek tray menüsü ve pencere akışı gözlemlenir. Önce odaklı frontend/Rust testleri, ana ajan bağlı incelemesinden sonra son aday WORKFLOW.md yerel kapısı bir kez; önce/sonra kaynak/araç/artifact fingerprint'leri aynı adımda. Kanıt `work/proofs/tray/`, rapor `docs/proofs/tray-lifecycle.md` ve `docs/reviews/H030.md`.

Windows10 x64 build19045 yerel ortamdır. H047/[karar006](006-free-ci.md) içindeki önceden kabul edilmiş ücretsiz çalışma istisnası korunur: ücretli Windows11 doğrulaması skipped/not_tested, başarı sayılmaz. H030'un genel iki-Windows satırı bu istisnayla okunur; diğer OS/temiz makine/yayın kabulü ayrı kalır. Yeni ücretli kaynak veya kullanıcı PC'sine VM kurulmaz. Geliştirme token tasarrufu ölçülmediği sürece yüzde yazılmaz. Ana görev tamamlanınca bildirilir ve H031 için kullanıcı beklenir.
