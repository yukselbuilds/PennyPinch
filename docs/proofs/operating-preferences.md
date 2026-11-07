# H031 — çalışma tercihleri kabulü

7 Kasım2026; **tamamlandı,30/51**. [Plan019](../decisions/019-operating-preferences.md), [inceleme](../reviews/H031.md). Ana ajan kalıcı/OS sahipliği, retention, motor bağlantısı ve kabul sahibi; tek Luna frontend/client testini tamamladı, root devraldı. H032 başlamadı.

## Son davranış

- Varsayılan X davranışı tepside çalışmadır. Tercih kapalıysa X çıkış onayı ister; ESC motoru durdurmaz. Footer ve çıkış penceresi aynı etkin tercihi açıklar.
- Ölçüm metadata'sı7/30/90 gün, varsayılan30. Daha kısa süre için kalıcı temizleme açıklaması ve onay vardır. CCR7200 saniye ayrıdır. SQLite schema1→2 mevcut ölçümleri korur; tek kalıcı tarih sınırı eski tarihli replay'in süre uzatılınca geri yazılmasını engeller. Zamanı bilinmeyen yeni yakalamalar önceki unknown/first_seen semantiğini korur; mutlak yaş iddiası yoktur.
- Gelişmiş korumalı araç listesi boş varsayılanla, en fazla32 doğrulanmış tam ad kabul eder. Sonraki elle Başlat'ta uygulanır; kayıtlı ve etkin liste farkı görünür. Aynı Headroom motorundan geçen desteklenen istemci/model/ana-alt ajan trafiği kapsamındadır; araç adı taşınmayan veya rota dışı çağrılar için garanti değildir. Sentetik deney adları üretim varsayılanı değildir.
- Windows başlangıcı ayrı kullanıcı eylemidir. Yalnız kendi exe komutuna ait sabit HKCU Run değeri yönetilir; stale/farklı değer korunur. Başlangıç pencereyi açar, motoru başlatmaz; Windows başlangıç politikası geçerlidir. Kabul testleri gerçek kullanıcı Run değerini değiştirmedi.
- Çalışma tercihleri dil/tema ile aynı atomik revision/fingerprint kaydındadır. Eski v1 kayıt okumada yazılmaz; eski save çağrısı yeni alanları sıfırlamaz. Bozuk/busy/çatışma sessiz başarı değildir.

## Son adayın kanıtı

Kayıtlar `work/proofs/settings/` altında. `acceptance.json`,26 kontrolle171 güncel kaynak/config girdisini, son binary/dist'i, UI bundle'ını ve araç/OS ortamını bağlar. Sonuç ile önce/sonra fingerprint'ler aynı adımlarda programatik kaydedildi; ham auth/prompt/HTTP hata dökümleri tutulmadı.

| Senaryo | Gözlenen | Kanıt / sınır |
| --- | --- | --- |
| Bütün yerel regresyon kapısı |153 frontend,362 Rust PASS;8 opt-in ignored; fmt PASS | `final-1.json`; son adayda bir tam kapı, worker'ın6 client testi bu toplamda. |
| TS/Vite/embedded native build; Python sınırları | PASS | `build-3.json`, `python-1.json`; son modal metni dâhil. Eksik etkin koruma veya açık beacon hazır sayılmaz. |
| Eski kayıt/CAS/araç sınırları, SQLite migration/7–30–90/Overview/Activity/reopen | PASS | `operating_*` gerçek FS/SQLite testleri; eski tarihli replay süre uzatılınca dönmedi. |
| Startup aç/kapat/reopen/stale/farklı/komşu değer | PASS | `autostart_*`: gerçek Windows API'si, UUID'li izole HKCU test anahtarı. Gerçek Run veya login/reboot testi değildir. |
| İki gerçek pinned0.41.0 generation ve araç listesi | İki58.800 bayt sayısal çıktı tam,2 POST; aynı endpoint/yeni generation | `live-2.json`, `live-runs/7f2ed1ab-b71d-42db-ae94-bb3e0e8b19f9/result.json`. Kayıt değişince eski nesil aynı kaldı; sonraki Başlat yeni listeyi doğruladı. Model/reasoning/call ID ve stats açık/beacon kapalı/TTL7200 korundu. Loopback fixture; canlı sağlayıcı değildir. |
| Native gerçek mouse ile iki yönde save/reopen, X onayı/ESC/same generation, tepsiye gizle | PASS, iki owned exit0 ve port cleanup | `native-runs/d771efba-25ef-4b6c-9cd8-61e008c33b1d/result.json`, `sky-ui.json`. Sky görüntü/tıklama bu kez çalıştı. Settled envanter0 görünür pencere gösterdi; runtime/metrics/preferences/protected state geri yüklendi, Run önce=sonra. |
| Busy/retry | Güvenli error ve Yeniden oku ile aynı kayıt | Eşzamanlı okumada store busy görüldü; UI varsayılanı kaydetmedi, kontroller kapalı kaldı. Sky Yeniden oku sonrası kayıtlı close=false/tema/dil geldi (`phase-1-reopened-ui-ready.json`). Meşguliyetin kök nedeni için performans iddiası yapılmaz. |
| TR/EN/açık-koyu/1280,819,683,507; pending/parser/conflict ve kayıtlı↔etkin ayrımı |8 UI davranış grubu PASS; taşma yok | `ui-d61e2bec-b88f-4989-a2bd-c929497a4395/result.json`; gerçek typed client + izole native transport. Native125/150 OS DPI iddiası yok; dar viewport kontrolüdür. Önceki örnek görüntüler ve son native ekranlar gözle incelendi. |

Native UI testinde retention/OS startup değeri değiştirilmedi; bunlar gerçek izole backend/API ve mock UI ile doğrulandı. Kullanıcı state'i geri yüklendi, auth/model/reasoning ve başka süreçler değişmedi. İlk native adayda bulunan tepsi metni düzeltildi; son native/UI koşusu ve tam kapı bu düzeltmeyi içerir. Değişmeyen başarılı kontroller tekrarlanmadı.

Yerel ortam Windows10 x64 build19045. Windows11 ücretli koşusu önceki H047/karar006 ücretsiz istisnasıyla skipped/not_tested; bu sonuç onun kanıtı değildir. Gerçek login/reboot, installer/upgrade, diğer OS, uzun yük ve canlı API-key/provider kabulü ilgili görevlerde açıktır. Geliştirme tasarrufu ölçülmedi, yüzde verilmedi.

H031 kapatıldı; H032 başlamadı. Ana görev sonunda duruldu.
