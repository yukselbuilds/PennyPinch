# Yerel uygulama sözleşmeleri

**Tarih:** 19 Ekim 2026. **Durum:** H012-a temel sözleşme kararı; H012'nin kalan kabulü ve Rust/TypeScript uygulaması açık. Teknik temel [002 runtime kararında](decisions/002-runtime.md); uyumluluk kayıtları [`compatibility/clients.json`](../compatibility/clients.json) içindedir.

## 1. Ortak yanıt ve sınırlar

Her Tauri DTO `schema_version=1` taşır. Bu sürüm, helper'ın stdio JSONL `protocol_version=1` sürümünden ayrıdır. Her istek/yanıt aynı opak `request_id` taşır; motorla ilgili yanıt ayrıca `runtime_generation` ile o nesle bağlanır. Helper'dan beklenmeyen sürüm veya mesaj türü gelirse `ready`/başarı üretilmez.

Hata DTO'su sabittir: `{schema_version, request_id, status:"error", error:{code, retryable, message_key, diagnostic_id}}`. `diagnostic_id` yoksa `null` olur. Başlangıçta kullanılacak sınırlı kodlar `validation_failed`, `unsupported_schema`, `unsupported_protocol`, `not_implemented`, `environment_blocked`, `busy`, `port_conflict`, `engine_not_ready`, `ownership_lost`, `timeout`, `cleanup_failed` ve `config_conflict` değerleridir. Tekrar denenebilirlik her yanıt için açıkça verilir; UI bunu hata metninden çıkarsamaz. Ham exception, subprocess stdout/stderr, URL/header, kullanıcı yolu, auth veya motor response gövdesi UI'ya taşınmaz. Bilinmeyen enum/sürüm, başarı ya da uyumluluk sayılmaz.

UI komutları dar ve tipli olur; runtime executable yolu, keyfi argüman, upstream URL, shell kodu, rastgele port veya okunacak dosya içeriği kabul etmez. Dosya seçimi yalnız ilgili istemci konumunu belirleme akışında doğrulanmış backend seçiciyle yapılır. Başlangıçta tek pencere/tek yönetici aynı runtime neslini sahiplenir.

**Model/ajan kapsamı varsayılanı (20 Ekim2026):** Bağlantı, desteklenen bütün modelleri ve aynı doğrulanmış rotaları kullanan ana/alt ajanları birlikte kapsar; kullanıcı model veya alt ajan başına kurulum yapmaz. Model/efor/ajan sayısı değişmez. Ayrı provider/protokol/süreç/bulut yolu plan ve kanıt gerektirir. Kapsama, role attribution ve gerçek optimizasyon sonucu farklı alanlardır: ajan kimliği bilinmeyen istek de desteklenen yoldaysa optimize edilebilir; bu kimlik yokluğunu “tüm alt ajanlar doğrulandı” iddiasına çeviremez. Mevcut uyumluluk kataloğundaki not_tested/unknown kayıtları bu kararla yükseltilmez.

Güncel kaynak ayrımı: [OpenAI'nin subagent belgesi](https://learn.chatgpt.com/docs/agent-configuration/subagents) custom agent dosyasında model/eforun özelleşebildiğini, model_provider gibi diğer oturum ayarlarının parent config'i override etmediğini açıklar. [Headroom proxy belgesi](https://docs.headroomlabs.ai/docs/proxy) protokol/endpoint ve auth yollarını ayrı tanımlar. Bunlar tasarım girdisidir; pinned0.41.0/kurulu istemci/native subagent için yerel doğrulama yerine geçmez.

| Komut ailesi | Girdi | Sonuç / uygulama kapısı |
| --- | --- | --- |
| Runtime snapshot | Yok | Aşağıdaki durum DTO'su; salt okunur. |
| Start engine | Yok; ayarlar backend state'inden | Idempotent başlatma isteği + güncel snapshot; H014. `ready` yalnız gerçek kanıtla. |
| Stop engine | Beklenen runtime nesli | Aktif trafik/bağlantı etkisi incelenmiş bounded shutdown; H029/H030 semantiği. |
| Pause/resume compression | Beklenen nesil ve hedef | H011/H028 geçmeden etkin bir UI eylemi yok; proxy restart veya config rollback yerine geçmez. |
| Discover clients | Yeniden tara isteği | İçeriksiz kurulum kimlikleri, sürüm/yüzey ve durum; H016. Ayar değiştirmez. |
| Plan connection/disconnect | Keşfedilmiş kurulum ve kapsam kimliği | Süreli plan kimliği, uygulanacak alanların sade özeti, yeniden başlatma ihtiyacı; H017/H029. |
| Apply plan | Plan kimliği, beklenen fingerprint | Atomik işlem/sahiplik sonucu; drift varsa conflict; H018. |
| Verify connection | Bağlantı kimliği | Gerçek rota kanıtı veya bilinmiyor; yalnız config yazılmış olması yeterli değil; H019–H021. |
| Query metrics | İzinli dönem/filtre | Aynı dönemden miktar/yüzde/kapsam DTO'su; H023–H025. |
| Save preferences | Dar tema/dil/retention vb. alanlar | Validasyonlu, sürümlü dayanıklı state; H015/H031/H032. |
| Export | Mevcut sorgu kimliği + JSON/CSV | Aynı metrik kapsamı; backend seçiciyle hedef; içeriksiz; H033. |

Bu adlar komut aileleridir; H013 yalnız uygulanmış dar snapshot/sağlık sınırını açar. Sonraki ailelerin isimleri ile request/response tipleri ilgili uygulama diliminde sabitlenir; boş stub başarı döndürmez.

## 2. Ayrı durum makineleri

**Motor:** `unavailable | stopped | starting | ready | stopping | failed | unknown`. Snapshot en az `schema_version`, `snapshot_id`, `observed_at_utc`, `runtime_generation`, `engine_version`, durum ve sabit hata kodunu taşır. `unknown`, gözlem yok/eskiyse kullanılır; `ready` veya `stopped` varsayılamaz. PID/loopback adresi iç tanı metadata'sıdır; UI'nın kendi yönetim hedefi seçmesine izin vermez. Önceki nesilden gelen geç yanıt yeni nesli `ready` yapmaz.

**Sıkıştırma:** `unavailable | not_implemented | enabled | pausing | paused | failed | unknown`. `ready` motor, `enabled` sıkıştırma anlamına gelmez. H011/H028 pause sözleşmesini kanıtlayana kadar UI'da pause eylemi yok ve durum `not_implemented` olur; `paused` uydurulmaz. Aktif stream sayısı `null`/unknown olabilir ve hiçbir zaman sessizce sıfıra çevrilmez.

**Keşif:** `not_scanned | detected | not_detected | error`; istemci sürümü bilinmiyorsa `null`. `not_detected`, protokol uyumsuzluğu değildir.

H016 Applications kaynağı ayrı `get_discovery_snapshot` / `scan_discovery` komut ailesidir; H014 genel runtime snapshot'ındaki discovery placeholder'ı bu ekranda kullanılmaz. Schema1/UUID request correlation ve artan `scan_generation` ile ilk durum `not_scanned`, bounded tarama sonucu `partial` olur. `installations` yalnız algılanan adayları taşır; her aday opaque UUID, istemci/yüzey, doğrulanabilmiş sürüm veya null, sabit kaynak/konum etiketi ve `connection_status:not_verified` içerir. Ham kurulum yolları backend'de kalır. Listede olmayan istemci yalnız taranan kapsamda bulunamamıştır; desteklenmiyor veya bağlantısı kurulmuş sayılmaz. Tarama program çalıştırmaz, config/auth/sohbet verisi okumaz ya da değiştirmez. Sabit hata allowlist'i `validation_failed | busy | environment_blocked`; ayrıntılar `docs/proofs/discovery-backend.md` içinde.

**Bağlantı:** `disconnected | planned | applying | needs_restart | verifying | verified | config_conflict | failed`. `verified`, hangi yüzey/sürüm/auth/protokol/transport için ve ne zaman doğrulandığını taşır. Runtime durması config sahiplik kaydını kendiliğinden silmez; UI bağlantı kaydı ile canlı rota durumunu ayrı gösterir.

**H017 salt okunur önizleme (21 Ekim2026):** `preview_connection_plan({request})`, deny-unknown-fields schema1 istek nesnesinde yalnız `schema_version`, opak `request_id`/`installation_id`, pozitif JS-safe `scan_generation` ve `scope:"user_config"` kabul eder. Config yolu, model, auth, provider URL veya port renderer'dan gelmez. Kurulum güncel keşif kaydından, adres taze H040 accessor'ından alınır; durdurulmuş motorla bağlı ingress de kullanılabilir. Başlangıç/son keşif, endpoint ve dosya fingerprint'i uyuşmazsa plan verilmez. Tek dosya okuma işlemi aynı anda yürür; en çok64 redakte plan lease'i beş dakika bellekte tutulur.

Başarı zarfı `{schema_version,request_id,status:"preview",plan}` biçimindedir. İç planın `status:"preview"|"blocked"` alanı draft'ın engellerini belirtir; bağlantı doğrulanması değildir. Kurulum/nesil/kapsam, süreli plan UUID'si, source/presence, sırsız fingerprint, yalnız loopback hedefi ve gerekli endpoint alanı, model/provider/auth/reasoning/ajan sayısı korunumu, config bildirim durumları, hook/shell özeti, yeni süreç/oturum gereği ve sabit blocker'lar taşınır. `runtime_effective:"unknown"`; config bildirimi açık istemcinin etkin değeri sayılmaz. Eksik config yeni dosya gereğini belirtir ama dosya/dizin oluşturmaz. Bilinmeyen sürüm/yüzey veya rota parse edilebilir engelli draft olabilir; malformede hata verilir. Kapsama ayrı provider/bulut yolları veya bütün açık oturumlar kendiliğinden dahil değildir.

`config_fingerprint` ham dosya SHA'sı değildir: sürümlü domain, client/scope/source/presence, length-framed içerik SHA-256, backend hedef kimliği ve Windows file identity bağını taşır. Yolun kendisi DTO/log'a çıkmaz. Aynı baytlarla farklı dosya nesnesi veya JSON→JSONC slot değişikliği drift sayılır. H018 bu binding'i ve kaydın süresini yeniden kontrol etmek zorundadır; H017 yazım/rollback komutu açmaz.

Hata zarfı ortak biçimdedir; dar kodlar `validation_failed`, `unsupported_scope`, `stale_selection`, `not_found`, `endpoint_unavailable`, `config_unavailable`, `config_unsafe`, `config_too_large`, `config_conflict`, `config_invalid`, `busy`. `diagnostic_id:null`; retryability ve `connection.<code>` mesaj anahtarı sabittir. Strict TS decoder dış/özel alanları, yanlış request/kurulum/client/nesil korelasyonunu, kaynak/presence/alan/URL tutarsızlığını, eksik config'teki sahte bildirimi, gösterilemeyen tarihi ve korunumu ihlal eden flag'leri reddeder. İstemci model/auth dosyası/sohbet DB'si çalıştırılmaz veya değiştirilmez. [Kanıt ve sınırlar](proofs/connection-plan.md).

## 3. Runtime kontrolü

Sabit payload yolu Rust bundle resolver'dan gelir. App state/workspace/config yollarını backend üretir. Helper giriş/çıkışı bounded JSON-lines protokolüdür; protokol sürümü/mesaj türü ve kaynak süreç handle'ı doğrulanır. Ham engine logları protokol sayılmaz.

Başlatma; manifest/guard kontrolü → uygulamaya ait helper → uygulamaya ait engine/listener → pinned readiness → snapshot yayımı sırasındadır. H006 packaged runtime ağacı başlangıç temeli ve referans payload'dır; H014 üretim Tauri bağını ayrıca doğrular. Yabancı listener, yanlış sürüm, timeout ve helper kapanışı ayrı hata kodudur. Start sırasında ikinci tıklama yeni motor üretmez.

EOF/normal çıkışta kontrol kanalı kapanışı bounded stop başlatır; timeout/fallback sonucu raporlanır. Başka PID/port öldürme yoktur. Bir önceki motorun portu boşalmadan yeni ready nesli ilan edilmez. Aniden kapanma/Job Object davranışı normal shutdown'dan ayrı sınanır.

H007'de kanıtlanan opaque cache marker shim'i, H014-a'da aynı hash ile üretim Python overlay/bootstrap'ına taşındı; H006 paket ağacı değiştirilmedi. Controller sabit manifest/profil ve payload hash'lerini kontrol ediyor; engine `0.41.0`/tam `prefix_tracker.py` source guard'ı bootstrap'ta çalışıyor. Guard başarısızlığında readiness yok. Rust/UI motor bağı mevcut hostta H014-b/c ile ayrıca kabul edildi; installer H037'de açık. Uyarlama yalnız Headroom'un opaque block'a eklediği marker'ı kaldırabilir; explicit client marker'larını veya tanınan blokları değiştiremez. Upstream sürüm/hash değişirse yeni proof gerekir; sessizce patch uygulanmaz. [H014-a kanıtı](proofs/desktop-runtime.md), [H014 kabulü](reviews/H014.md).

**H040 kararlı giriş sözleşmesi (21 Ekim2026):** `get_ingress_snapshot(request_id)` ayrı schema1 yanıtıdır; mevcut runtime snapshot alanlarını değiştirmez. `endpoint` yalnız durum, kalıcı UUID, uygulamanın seçtiği `127.0.0.1`/port ve sabit hata kodunu; `backend` durum, geçici motor nesli ve sabit hata kodunu taşır. Nullable `active_connection_count` TCP bağlantı sayısıdır; HTTP isteği veya SSE stream sayısı değildir. UI bu komuta URL, PID, executable veya hedef port veremez. H017 planlayıcısı aynı runtime yöneticisinin taze `ready_stable_endpoint()` accessor'ını kullanır; mevcut Başlat/Durdur komutları manager üzerinden ingress yaşam döngüsünü kullanır.

Endpoint gözlemi ve engine health ayrı güncellenir; süresi geçen gözlem `unknown/timeout` ve nullable sayaç üretir. Motor yeniden başlarken endpoint kimliği/adresi korunur; her TCP bağlantısı tek backend nesline sabitlenir. Kayıtlı port doluysa başka adres seçilmez. Backend socket'inin exact established peer PID'si doğrulanmadan istemci payload'ı aktarılmaz. Yalnız PID değil held process handle, JobObject üyeliği, parent chain ve nesil doğrulanır; Windows'un ek owned konsol süreçleri olabileceği için güvenlik tam üç süreç sayısına dayanmaz.

Stop sırası yeni route'u kaldırma → engine graceful stop → bounded TCP drain → owned cleanup doğrulamasıdır. Zorla iptal graceful başarı sayılmaz; tamamlanmış temizlik ve belirsiz temizlik ayrı tutulur. Doğrudan upstream fail-open kapalıdır; kopmuş POST/SSE başka backend'e tekrar gönderilmez. Gateway/OAuth, CCR/pause, canlı istemci ve model desteği bu transport sözleşmesiyle doğrulanmış olmaz. Uygulama tamamen kapalıyken port serbesttir; tray/background veya config geri alma H018/H029/H030/H034 kapılarında ayrıca çözülür. [Arıza matrisi](validation/ingress-failure-matrix.md), [karar](decisions/003-connection-ingress.md), [yerel kanıtlar](proofs/connection-ingress.md).

## 4. Config sahipliği ve geri alma

Plan sunucu tarafında tutulur; UI'ya auth/sır değerleri veya tüm config gövdesi verilmez. Plan, mevcut içeriğin fingerprint'i, seçili yüzey/kapsam ve yalnız değiştirilecek alanların sahiplik bilgisiyle bağlanır. Uygulama anında drift varsa yeniden plan gerekir.

JSONC yorumları ve ilgisiz TOML/JSON alanları korunur. H018'in Windows backend'i capability-gated KTM/TxF kullanır: config ve sırsız sahiplik kaydı aynı yerel sabit NTFS volume üzerinde, aynı transaction'da yayımlanır. Commit öncesi kimlik/içerik/kök ve süre yeniden kontrol edilir; unsupported filesystem, reparse/hardlink/EFS/readonly kapalıdır. Daha zayıf rename fallback yoktur. TxF bakım/gelecek desteği riski [karar005](decisions/005-config-transaction.md) içindedir. Commit cevabı belirsizse değişmedi/başardı denmez; yeniden kayıt okuma gerekir, otomatik write retry yoktur.

Geri alma yalnız hâlâ uygulamanın yazdığı endpoint alanını kaldırır. Kullanıcının sonradan değiştirdiği endpoint conflict kalır; ilgisiz değişiklikler ve normal atomic-save yeni dosya kimliği korunur. Bütün config yedeği tutulmaz/geri yazılmaz; oluşturulan dosya veya kullanıcı içeriği olan container silinmez. Aynı endpoint önceden kullanıcıya aitse yeni ownership ve Undo yetkisi yaratılmaz.

Üç komut aynı manager/işlem kilidini kullanır: `apply_connection_plan`, `undo_connection`, `get_connection_ownership`. İstekler schema1/request UUID ile sınırlıdır; Apply plan UUID+expected config fingerprint, Undo client enum+owner UUID+expected revision+record fingerprint, snapshot yalnız client enum taşır. Renderer serbest yol/URL/config/auth göndermediği gibi yanıttan da almaz. Wire client enum `codex|claude_code|opencode_v1`; discovery `opencode` UI'da açık eşlenir. Yanıt `applied|unchanged|released|snapshot` ve nullable ownership metadata içerir; strict TS decoder request/client/işlem/route/revision korelasyonunu doğrular. Sabit hata kodları ve message key dışında raw error gösterilmez.

Graphite bağlantı yönetimi açılışta pasif sahiplik snapshot'ı okur; Preview ayrı eylemdir. Apply için geçerli plan ve güncel başarılı kayıt okuması gerekir. Pending mutation sırasında tekrar yazım/Close/Escape/selection engellenir. Kurulum bulunmasa da mevcut sahiplik Undo yapılabilir. App restart'ında kayıt bulunur; eski süreli plan canlanmaz. Kaydedilmiş yönlendirme canlı bağlantı kanıtı sayılmaz; Headroom/motor açık olmalı ve istemci yeni süreçte ayarı kullanmalıdır. App kapanırken otomatik geri alma yapılmaz.

Mevcut model, reasoning, auth yöntemi, ajan sayısı ve izin ayarları değiştirilmez. Süreçlik fixture profili üretim bağlantı planı sayılmaz. Desteklenmeyen veya yönetilen config kapsamında UI başarı göstermez. Yeniden başlatma gerekiyorsa bunun gerçek etkisi plan ve bağlantı durumunda görünür olur.

## 5. İçeriksiz ölçüm

Normalize kayıt, kaynak/provenance, runtime nesli, benzersiz event kimliği, UTC zaman, izinli istemci/model etiketleri, nullable sohbet/proje/ajan ilişkisi, ölçüm temeli ve ölçüm durumunu taşır. Model adı tek başına ajan kimliği değildir. Kullanıcı yolu veya keyfi header/tag ilişkisi yerine güvenilir yerel eşleme gerekir.

Öncesi/sonrası `B/A` sayıları aynı tokenizer temelindeyse karşılaştırılır; biri eksikse çift kısmi kalır. İkisi de negatif olmayan sayılardır, fakat `S=B-A` negatif olabilir. Provider usage, yerel token tahmini, wire azalması, novel kazanç, cache/uncached kırılımı, tool-schema ve tahmini maliyet ayrı alan/temellerdir.

Her çiftin kapsadığı bileşenler de belirtilir; yalnız mesajları ölçen kaynak bütün girdi/sistem/tool schema toplamı sayılmaz. Ayrı kazanç katmanı, örtüşmeyen karşılaştırılabilir tabanı olmadan ana paya eklenmez. Output tasarrufu girdi oranına karışmaz.

Ana sorgu sonucu:

- Dönem başlangıç/bitişi ve saat dilimi; kaynak zamanı ve son güncelleme.
- Geçerli aynı temelli çiftlerin `before_total`, `after_total`, `saved_total` değerleri; yüzde `100×saved_total/before_total` yalnız pozitif tabanda.
- Ölçülebilen istek sayısı, bilinen toplam/gözlenen trafik sayısı, kısmi/kayıp aralık durumu. Toplam bilinmiyorsa kapsam yüzdesi uydurulmaz.
- `status=waiting | complete | partial | unavailable`; gerçek sıfır geçerli sayıdır, veri yokluğu ayrı durumdur. Farklı tokenizer temelleri sessizce tek kesin toplamda birleştirilmez.

Yinelenen event alınmaz. Snapshot farkı ve event toplamı aynı olaya iki kez katkı vermez. Restart/sayaç sıfırlaması yeni nesille ayrılır. Polling buffer'ı aşıldığında gap kaydı oluşur. Yalnız tasarruflu event'leri içeren ledger bütün trafik paydası sayılmaz.

UI, grafik ve JSON/CSV aynı sorgu sonucunu kullanır; yuvarlama gösterimde yapılır. Token artışı gizlenmez. TR/EN biçim değişikliği sayısal veriyi veya dönemi değiştirmez. Maliyet tahmini varsa fiyat tarihi/kaynağı ve cache temeli ayrıca görünür; ana token oranı fatura veya abonelik kotası indirimi diye sunulmaz.

Allowlist dışındaki raw log, `request_messages`, `compressed_messages`, auth, URL query/header ve serbest tags bu katmana alınmaz. Detaylar/fingerprint'ler [pinned metrik kaynak notunda](research/metrics-0.41.0.md).

### H023 normalizasyon API'si

`src-tauri/src/metrics/normalize.rs` içindeki saf API schema1 üretir. H023 UI veya polling açmaz; H024 kalıcı depo/sorgu ve toplayıcının kayıt girişini, H025 canlı owned kaynak toplama ve görünümü bağlar. H024 tek başına motoru polling'e başlatmaz.

- `normalize` güvenilir toplayıcının `Observation` dizisini işler. `Reading` nullable token, local/provider/unknown türü, sürümlü **sayım yöntemi + tokenizer kimliği** ve bileşeni taşır. Sadece model adı veya etiketteki tokenizer adı karşılaştırılabilirlik kanıtı değildir. `before`/`after` aynı kanıtlı temeldeyse miktar ve pozitif tabanda yüzde türetilir.
- `main_metric_eligible` yalnız tarihli, kökeni kanıtlı Headroom mesaj/bütün-girdi çiftinde true olur. Sıfır taban miktarı bilinse bile yüzde null'dır. RTK/tool-schema/novel/cache/output ayrı ölçümlerdir; provider kullanım alanları ana çifte eklenmez. Farklı tokenizer veya bileşenler tek toplam olamaz.
- `origin_id` runtime nesli + istek kökeninin SHA-256 anahtarıdır. `event_id` sabit optimizer/bileşen slotunu da içerir. Aynı isteğin farklı katmanları ayrı event olabilir: istek sayısı event satırı sayısı değildir. Başlıca metrik yalnız eligible satırlardan ve aynı bileşen/temelden hesaplanır. Mesaj/bütün-girdi slotu ortak tutulduğu için örtüşen tekrar katkıları conflict olur.
- `observed_via` yalnız gözlem kanallarını belirtir; MCP'nin güvenilir proxy köken eşlemesi yoksa ana toplama katkısı yoktur. İçerik/tag içindeki eşleme alanına güvenilmez. Canonical `Observation` bir renderer/ham engine giriş sözleşmesi değildir; native toplayıcı kökeni/temeli doğrular. Aynı payload tekrarı birleşir; çelişki miktar/yüzdeyi ve belirsiz temel/zamanı unknown yapar. Nesiller birbirinden ayrıdır. Pollingler arası upsert/retention H024'te aynı anahtarları kullanır.
- `project_headroom_041` bounded JSON metadata adaptörüdür: `recent_requests` tercih edilir, yoksa `request_logs`; çift liste birden sayılmaz. Raw payload, ID, auth, model/ajan/proje çıkarımı, serbest tags ve lifetime değerleri çıkışa taşınmaz. Eski sayıların temeli/bileşeni bilinmiyorsa ham before/after gözlemi korunur; saved/% ve provider kullanım provenance'ı varsayılmaz.
- `reported_request_count`, `unidentified_rows`, `rolling_window`, `aggregate_only` ve `complete_traffic=false` kaynak kapsamını açık tutar. Aggregate sayaç event toplamına eklenmez veya farklanmaz; reset/restart kazanç üretmez. Bilinmeyen toplam sıfıra çevrilmez; örneklerde görülen eski lifetime verisi güncel trafiğe taşınmaz. H024 kaynak uyarılarını ve retention'ı saklar; gerçek polling/gap kapsamı H025'te toplayıcıyla doğrulanmadan tam trafik iddiası kurulmaz.

Sınırlar ve karar: [010](decisions/010-metrics-normalization.md). Sabit `NormalizeError` değerleri payload içermez. Yeni bağımlılık yok; mevcut `time` kütüphanesinin parsing özelliği kullanılır.

### H024 kayıt ve sorgu API'si

- Crate içi `MetricsStore::ingest_projection`, H023 `EngineProjection` metadata'sını transaction içinde event_id upsert ile kaydeder. Renderer'dan event/import SQL veya dosya yolu kabul eden yazma IPC'si yoktur. İlk izleme oturumu/ilk görülme zamanı replay ile değişmez; ortak normalizer merge'i çatışmayı absorbe eder. Non-conflict event_id, origin + optimizer/bileşen slotuyla yeniden doğrulanır.
- `get_metrics_summary({request:{schema_version:1,request_id:<canonical non-nil UUID>,period:"session"|"today"|"seven_days"}})` yalnız yerel depoyu sorgular; motor/model çağırmaz. Tauri komutu blocking DB işini UI iş parçacığından ayırır. `status:"ok"` içinde `data` veya sabit kodlu `status:"error"` yanıtı verir; hata cevabı girdi/payload/yerel path içermez.
- `data`: `monitoring_session_id`, `period`, UTC `from_utc`/`to_utc`, `retention_days:30`, `totals`, `coverage`. Session bu Headroom sürecinin izleme oturumudur; Codex/Claude hesabı veya sohbeti değildir. Bugün ve7 gün sorgu anındaki yerel takvime göre çözülür. Zamanı yerleştirilemeyen retained kayıtlar seçili tarihte olmuş sayılmaz; yalnız belirsizliği görünür kılmak için `unplaced_events`/excluded içinde tutulur.
- `totals.status`: `measured | no_comparable_events | mixed_bases | counter_overflow`; nullable before/after/saved/% aynı temelde ağırlıklı hesaplanır. Gruplar farklı tokenizer/sayaç türü/bileşeni birleştirmez. En çok64 grup ayrıntısı ve `omitted_basis_groups` döner; kırpılan gruplar ana yüzdede birleştirilmez. Provider/cache/output/RTK ana girdiye eklenmez.
- `coverage`: event ve distinct origin sayısı, yerleştirilemeyen zaman, rolling/aggregate/kimliksiz kaynak uyarıları; `complete_traffic=false`. Bu sayılar tüm trafik veya kullanıcı turu adedi değildir. Ingest sayıları ve lifetime snapshot toplamları dönem tasarrufuna eklenmez.
- Başlangıç ve boş sorgu dosya yaratmaz. Debug kökü repo `work/app-state/metrics`, release kökü uygulamanın local-data `metrics` dizinidir.30 gün,100.000 event,128MiB DB ve4096 byte/event sınırları; schema/app-id, reparse/hardlink, busy ve transaction rollback koruması vardır. Ayrıntılar [012](decisions/012-metrics-store.md).

## 6. Uyumluluk kataloğu

Her kayıt istemci sürüm aralığı, yüzey, auth yöntemi, protokol/transport, capability ve kanıt referansıyla tanımlanır. Katalog [`compatibility/clients.json`](../compatibility/clients.json) içindedir. Durumlar `not_tested | unknown | local_fixture_partial | local_fixture_pass | live_verified | unsupported | environment_blocked`; gerekçe ve ölçüm tarihi zorunludur. `unknown`, denenen sonucun semantiği sınıflandırılamadığında; `not_tested`, ilgili kombinasyon hiç denenmediğinde kullanılır. Policy/OS engeli `environment_blocked` olur ve uyumsuzluk iddiası değildir. Kısmi kanıtta hangi alt davranışın geçtiği ve hangisinin bilinmediği ayrı yazılır. Yerel PASS yalnız o sürüm/yüzey/auth/protokol satırına aittir, canlı desteğe otomatik yükselmez. `unsupported` yalnız somut uyumsuzluk kanıtıyla verilir.

İlk katalog H006-a/H007, H008 kısmi, H009-a ve H010-a'nın eldeki gerçek kapsamını yansıtacak. Codex ChatGPT/API/desktop/subagent, Claude bare API/OAuth/extension ve OpenCode V1/V2/TUI ayrı kayıtlardır. Bir sürüm/transport değişince yalnız ilgili kanıt geçersizleşir; eski PASS yeniden koşulmadan yeni kombinasyona taşınmaz.

## 7. Kabul sırası

H012, taslakları gerçek kaynak ve katalogla karşılaştırıp kararları kesinleştirir. H013 scaffold'u; tipli snapshot komutu, Graphite dört ekran, boş metrikler, gezinme ve tema/dil temeliyle başlar. Gerçek start H014, config yazımı H018 ve başarılı bağlantı H019–H021 kapılarıdır. Uygulanmayan eylemler, fixture tasarrufu veya demo durumlar üretim başarısı olarak gösterilmez.

H011 pause/CCR/prefix ve H006/H037 temiz OS sonuçları tamamlanana kadar ilgili capability/yayın kabulü açık kalır. Bu sözleşme, eksik kanıtları kapatmaz veya üretim test komutlarının mevcut olduğunu iddia etmez.
