# H034 — hata ve toparlama akışları

8 Kasım 2026 tamamlandı; 33/51. Ana ajan runtime, dosya/ayar sahipliği, teşhis semantiği, backend/entegrasyon ve kabul sahibi. Salt okunur strict istemci/arayüz dilimi aynı tek Luna xhigh tarafından tamamlandı; root tek düzeltme paketi sonrası devraldı. Ponytail full, Emil ve DESIGN.md uygulandı. H035 başlamadı.

## Amaç ve sınırlar

Tek “Bağlantıyı denetle” eylemi motoru, kalıcı girişi, her uygulamanın kayıtlı config kapsamını ve son kaydedilmiş trafik işaretini ayrı gösterir. Sonuç bir yerel anlık görüntüdür; sağlayıcı/auth/ana-alt ajan başarısı veya tüm modellerin çalıştığı anlamına gelmez. Teşhis istemci çalıştırmaz, sağlayıcı isteği göndermez, config/auth/istemci tercihi yazmaz, otomatik onarım/bypass/yeniden başlatma yapmaz. Başlatma mevcut sahiplikli lifecycle yolunda; config onarımı mevcut açık Bağlan → önizleme → uygula yolunda kalır.

Bir kullanıcı eyleminde en fazla bir teşhis vardır; pending ve native single-flight yeni isteği reddeder. Başarısızlığı kör döngüyle tekrar deneme yoktur. Config/auth hatası motoru veya başka süreçleri kapatmaz. Eksik runtime ve kayıp sahiplikte güvenli sonraki adım gösterilir. Gözlenemeyen hata kaynağı unknown kalır; 401/403 veya giriş hatasını istemcide doğrulama/giriş yapma yönlendirmesi sunulur, auth dosyası okunmaz.

Keşifteki dosya kimliği/boyutu/son yazma zamanı yeniden gözlenir. Değişmiş veya kayıp dosya eski sürüm kanıtıyla current sayılmaz; yeniden tara ve yeni config/protokol doğrulaması önerilir. Current yalnız keşif anındaki dosyayla metadata eşitliğidir, sağlayıcı/özellik uyumluluğu kanıtı değildir. Yeniden tarama eski görünür raporu stale yapar. Normal akış binary seçimi istemez.

## IPC sözleşmesi

`get_recovery_snapshot({request:{schema_version:1,request_id:UUID}})`. Request deny_unknown_fields; UI herhangi bir dosya yolu, hata metni, token veya istemci seçimi göndermez.

Başarılı zarfın tam alanları: `{schema_version:1,request_id,status:"snapshot",observed_at_utc,runtime,traffic,clients}`.

- runtime: `{engine_status,ingress_status,error_code}`. Engine unavailable/stopped/starting/ready/draining/failed/unknown; ingress unavailable/starting/ready/unknown; error_code mevcut runtime public error allowlist veya null.
- traffic: `{status,period:"seven_days",last_observed_utc,outcome,error_layer:"unknown"}`. Status observed/none/unavailable; outcome unknown/no_error_reported/error_reported. Observed yalnız mevcut yerel yedi günlük kayıtlar; timestamp nullable ise none/unavailable. Rota/hesap/sağlayıcı doğrulaması değildir; hata bildirilmedi başarı değildir. Ham error/exit code uydurulmaz veya maskelenmez.
- clients: tam üç satır, codex/claude_code/opencode sırasıyla. Her biri `{client_kind,detected_count,installation_status,compatibility,config_state,config_scope,endpoint_binding}`. Count 0–512. Installation current/stale/not_found/unknown; compatibility config_supported/not_verified (yalnız mevcut yerel config adaptörü, canlı trafik değil). Config matches_owned_endpoint/conflict/not_owned/unavailable/busy/not_rechecked; scope user_config/project_local_config/unknown; binding same/different/not_owned/unknown. Başka alan, kişisel yol, model/provider etiketi, auth, config içeriği veya owner/generation kimliği çıkmaz.

Hata zarfı: `{schema_version:1,request_id,status:"error",error:{code,retryable,message_key,diagnostic_id:null}}`. Kodlar invalid_input (false), busy (true), unavailable (false); anahtar `recovery.<code>`. UI ayrıca invalid_response/request_failed/desktop_unavailable gösterir. Korelasyon ve kesin alan kontrolü uygulanır.

## Kabul

1. Normal yerel akış: gerçek backend raporu → tek eylemle iki dil/tema/dar görünümde dört ayrı kanıt alanı; boş trafik kaçak veya yüzde sıfır olarak sunulmaz. Config eşleşmesi canlı başarı etiketi almaz. Teşhis önce/sonra istemci/auth/ownership baytları aynı.
2. Motor çökmesi/eksik runtime, config değişimi/yanlış kapsam, auth veya kaynağı bilinmeyen istemci hatası, busy/bozuk cevap: doğru sonraki adım; bir istekte tek deneme, sahte başarı ve yabancı süreç sonlandırma yok. Onarım mevcut lifecycle veya önizleme yoluna gider. Raw error, içerik ve sırlar rapora çıkmaz.
3. Güncelleme veya tarama değişimi: eski rapor stale; metadata değişimi eski current/compatibility kabulünü geçersiz kılar, yeniden tarama ve kontrollü doğrulama önerilir. Son aday yerel kalite kapısı ve gerçek native tıklama; kaynak/test/config/araç/ortam önce-sonra fingerprint'leri tutulur. Windows 11/diğer OS mevcut kanıt sınırlarını aşmaz.

Odaklı kapı: yeni recovery Rust testleri ve strict client testleri; UI fixture gerçek sağlayıcı kanıtı değildir. Son adayda WORKFLOW.md tam kapısı bir kez. Kanıt `work/proofs/recovery/`, kabul `docs/proofs/recovery.md`, inceleme `docs/reviews/H034.md`. H034 kapanınca kullanıcıya bildirilir ve durulur.

## Sonuç ve kanıt sınırı

Son aday 168 frontend/379 Rust PASS, 8 opt-in ignored; fmt, TypeScript/Vite ve embedded native build PASS. 20 UI birleşimi/8 davranış grubu, gerçek native stopped → start → ready denetimi, eski raporun geçersizliği ve üç native yanıtın strict decoder kabulü tamamlandı. 18 toplu kontrol 187 güncel kaynak/config, son binary/dist ve araç/OS fingerprint'lerini bağlar. Yeni sağlayıcı çağrısı yapılmadı; normal exit0, owned portların kapanması ve orijinal state/Run korunması kaydedildi. [Kabul raporu](../proofs/recovery.md).

Config onarımı için yeni writer eklenmedi. Pure undo'nun ürettiği baytlar atılır; mevcut kaydı doğrulama dışında kullanılmaz. Sadece metadata eşitliği binary bütünlüğü/özellik uyumluluğu sayılmaz. Arıza kabulü gerçek manager public-state enjeksiyonu ve gerçek Windows dosya fixture'larıyla sınırlıdır; canlı auth arızası ve gerçek motoru zorla çökertme yapılmadı. Bilinmeyen katman unknown kalır. Windows10 yerel kanıt; Windows11, diğer OS, installer ve uzun süreli yük sonucu çıkarılmaz. H035 için kullanıcı devam mesajı beklenir.
