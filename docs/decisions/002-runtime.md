# 002 — Yerel masaüstü mimarisi

**Tarih:** 19 Ekim 2026; 15 Kasım H012 ana kabulüyle güncellendi. **Durum:** Mimari ve uyumluluk sözleşmesi kesinleştirildi. Uygulama ve destek kapsamı aşağıdaki kanıtlara bağlıdır. [H012 kabulü](../proofs/architecture-acceptance.md)

## Seçim

Tauri 2 / Rust yönetici, React / TypeScript arayüz ve uygulamaya ait CPython 3.13.13 + `headroom-ai[proxy]==0.41.0` runtime dizini kullanılır. Windows 10 22H2 ve Windows 11 x64 hedeflenir. Arayüz yerel paketlenir; Windows'ta sistem WebView2 kullanır. Mevcut manifestlerde Tauri 2.12.4, React 19.3.1, TypeScript 7.0.2 ve Vite 8.3.4 sabittir; bağımlılık çözümü Cargo/pnpm lock dosyalarındadır. Bunlar yerel adayın sürümleridir, en güncel sürüm iddiası değildir.

Bu bileşen seçimi korunur; başka masaüstü framework'ü veya genel amaçlı yardımcı servis eklenmez. Tauri yanıtları `schema_version=1`, üretim helper stdio JSONL mesajları `protocol_version=1` kullanır; bu iki sürüm alanı birbirinden bağımsızdır. Girdi ve hata zarfları komut ailesine özgüdür; [yerel sözleşmedeki kayıt](../contracts.md#8-uygulanan-ipc-kaydı-14-kasım-2026) mevcut 39 Tauri komutunu kaynakla eşler. H006-a'nın tarihsel helper'ı ile H014/H028'de kullanılan üretim controller/helper aynı kanıt sayılmaz. Graphite arayüzü gerçek backend durumunu, sıfır/bilinmeyen metrikleri ve ayrı hesap/sohbet görünümlerini kullanır.

H006-a taşınabilir runtime dizinini ve konsolsuz üst süreç → gizli konsollu helper → motor yaşam döngüsünü kanıtladı; 523.394.231 bayt tarihsel payload ölçüsüdür. H037 yerel Windows kurulumunu, H006-b ise güncel 22.333 dosya / 536.626.939 bayt tam paketle hazır durumdaki Rust controller sahibinin zorla kaybını doğruladı. Ölçülen ağaçlar aynı boyut kapsamı değildir. Paket [Windows kurulum kaydında](../validation/windows-install.md), mekanizma sınırı [H006-b kanıtında](../proofs/windows-engine-crash.json) tanımlıdır. Gerçek paketli GUI kaybı ve aynı profilde yeniden açılış 15 Kasım [H006-c](../proofs/windows-gui-crash.md) kapsamında ayrıca doğrulandı. Temiz Windows 10/Windows 11 ve Job ataması öncesi yarış açık kalır. Son kullanıcıya geliştirme Python/Node kurulumu yüklenmez.

## Süreç ve güven sınırı

- WebView yalnız dar Tauri komutlarını çağırır. Genel shell, keyfi URL/port/path üzerinden proxy yönetimi ve uzak sayfa çalıştırma yetkisi verilmez. UI içerik veya secret taşıyan motor debug/log nesneleri almaz.
- Rust yönetici, kendi runtime nesli için helper handle/PID ve loopback sahipliğini izler. Helper/manifest sürümü doğrulanır; listener ve pinned readiness uyuşmadan `ready` yayımlanmaz. Başka motorun bulunduğu port sahiplenilmez veya kapatılmaz.
- Helper kontrolü boyut sınırlı stdio JSON mesajlarıyla yürür. Engine logları ayrı ve içeriksiz durum kodlarına indirgenir. stdout'taki herhangi bir JSON satırı tek başına kimlik/ready kanıtı sayılmaz.
- Rust yönetici tek runtime nesli için tek sahip olur; her istek/yanıt `request_id`, motor yanıtları `runtime_generation` ile bağlanır. Unknown snapshot `ready` sayılmaz. Sabit UI hata zarfı ve başlangıç durumları [yerel sözleşmede](../contracts.md) tanımlıdır; ham child-process çıktısı UI DTO'suna girmez.
- Uygulama state kökü runtime payload'ından ayrıdır. Sabit bundle dosyaları değiştirilmez. Bağlantı ayarları ve kullanıcı tercihleri dayanıklı/atomik yazılır; mevcut giriş sırları uygulama state'ine kopyalanmaz.
- Normal kapanış stdio isteği ve bounded drain ile başlar. Yalnız sahip olunan süreçler için timeout/Job Object fallback vardır; force kapanış graceful diye sunulmaz. H009'un test controller'ındaki force sonucu, H006-a helper yaklaşımını geçersiz kılmaz ve gerçek Tauri bağlantısının kanıtı yerine geçmez.
- H007'nin opaque-block cache marker uyarlaması üretim shim'ine taşınmıştır. Paket sürümü, manifest, controller/helper, shim/control ve tam `prefix_tracker.py` hash guard'ları [Rust başlatma yolunda](../../src-tauri/src/runtime/manager.rs) kontrol edilir; uyuşmazlıkta motor ready olmaz. H014 üretim bağı ve H028 pause/CCR kanıtı yerel Windows 10 kabulüdür. Paket/manifesti sessizce değiştirmek veya guard olmadan patch uygulamak kabul edilmez; başka OS, yeni paket veya canlı sağlayıcı için aynı kabul varsayılmaz.

## İstemci adaptörleri

Keşif, değişiklik planı, uygulama, doğrulama ve geri alma ayrı işlemlerdir. Keşif dosya değiştirmez. Plan etkin model/reasoning/auth yöntemini ve kapsamı koruyarak yalnız gerekli yönlendirme alanlarını önerir. Uygulama anında fingerprint tekrar kontrol edilir; geri alma sonradan yapılan kullanıcı değişikliğini ezmez.

İstemci sürümü, yüzey, auth, protokol/transport ve capability ayrı uyumluluk boyutlarıdır. Başlangıç kayıtları [`compatibility/clients.json`](../../compatibility/clients.json) içinde sürüm ve kanıtla tutulur. API-fixture başarısı canlı OAuth veya desktop/subagent desteğine yükseltilmez. Kurulum bulunamaması, desteklenmiyor demek değildir. Doğrulanmamış kombinasyon doğru etiketlenir; uygulama model/efor/ajan sayısını sessizce değiştirmez.

Codex'in proje config'inde yok sayılan yönlendirme anahtarlarına güvenilmez; süreç/kullanıcı kapsamı ve mevcut ChatGPT gateway yolu ayrı doğrulanır. Claude bare/API-fixture ve OpenCode V1/V2 aynı destek kaydı değildir. Kullanıcının açık oturumlarını otomatik yeniden başlatmak veya auth dosyalarını yeni fixture'a kopyalamak entegrasyon yöntemi sayılmaz.

OpenCode 1.19.3 ve 2.3.0 CLI, Windows'ta sabitlenen dosya digest'iyle tanınır. V1 ve V2 aynı bağlantı/sahiplik yöneticisini kullanır. V2 plan ve receipt'i opsiyonel `opencode_schema:"v2"` taşır; alanın yokluğu eski V1 kaydını korur. Uygun V2 `providers` endpoint'leri ve `mcp.servers.headroom` tek atomik işlemde bağlanır. Sürüm, şema, kullanıcı izinleri ve retrieval köprüsü uygunluğu ayrı kapılardır. [H010 kabulü](../proofs/opencode-acceptance.md), [rota ve kayıt sözleşmesi](009-opencode-provider-routes.md).

## Duraklatma

Motoru durdurmak, istemci bağlantısını kaldırmak ve sıkıştırmayı duraklatmak farklı işlemlerdir. Duraklatma mevcut stream'i bitirmeli, geçmiş CCR deposunu erişilebilir tutmalı ve önceki prefix'i bozmamalıdır. Yalnız `HEADROOM_OPTIMIZE=0`, bypass header veya proxy restart uygulayarak bu sözleşmenin sağlandığı varsayılmaz.

H011-a kaynak incelemesi [`pause.md`](../proofs/pause.md) içinde tarihsel kayıttır. Pinned upstream bypass/runtime-env/restart kontrolleri güvenli pause yerine geçmez. H028'de üretim kontrol modülü, nesil/revision ve özel ack üzerinden gerçek pause/resume'a bağlandı; aktif HTTP/SSE ve WebSocket, eski prefix/CCR ve resume sonrası yeni sıkıştırma [yerel replay ile doğrulandı](../proofs/safe-pause.md). Motor çalışırken kontrol okunamıyorsa durum `unknown`, motor kullanılamıyorsa `unavailable` olur; yalnız onaylanmış durum paused gösterilir. Bu Windows 10 fixture kanıtı canlı sağlayıcı veya tüm OS desteği değildir.

## Ölçüm

Ana kart `Σ(B-A)` ve `100×Σ(B-A)/ΣB` çiftini aynı kapsamdan gösterir. İçeriksiz alan seçimi, aynı tokenizer temeli, event dedup, runtime restart ve kayıp aralığı normalizasyon sözleşmesine dahildir. Ham `/stats` yüzdeleri ürün anlamı doğrulanmadan kopyalanmaz; kaynak analizi [metrik notunda](../research/metrics-0.41.0.md).

İstek düzeyindeki wire azalması, benzersiz içerik kazanımı, yeni içerik/cache ve maliyet farklı temellerdir. Tahmini fatura karşılığı ana token metriğiyle eşitlenmez. Atanamayan proje/ajan/model alanları tahmin edilmez. Panel raporu için LLM çağrısı gerekmez.

## Uygulama ve yayın sırası

H013–H036, H038–H040, H045 ve H050–H051'in yerel ürün dilimleri uygulanmıştır. H014 gerçek helper bağlantısı; H015–H018 state/keşif/plan/işlem; H019–H021 istemci bağlantısı; H023–H027 içeriksiz ölçüm/atama ve H028 güvenli pause ayrı kanıtlarla değerlendirilir. H050 hesap kaydı ile H051 sohbet metadata'sı ayrı kabiliyetlerdir; hesap varlığından canlı rota, sohbet/model bilgisinden trafik veya tasarruf türetilmez. Yerel imzalı update/kurtarma H042'de, üretim anahtarı/kaynağı ve yayın ayrı açık kapsamdır.

H012, bu bileşen, süreç, IPC, kayıt, ölçüm ve uyumluluk sözleşmesinin kabulüdür. H008'in Windows 10 ChatGPT CLI/Desktop kanıtı ve H010'un yerel OpenCode CLI/TUI/CLI-ayarlı Desktop kabulü kendi kapsamlarında kayıtlıdır. Katalogda 14 istemci, 1 kabiliyet ve 3 ortam kapısı bulunur. Canlı Claude/OpenCode auth, API-key, temiz Windows/Windows 11 ve üretim yayın kabulleri kendi ilgili destek ve yayın kapılarında açık kalır.

K3, mimari seçimlerin somut kanıta dayanmasını ister. Bilinmeyen birleşimler katalogda gösterildiği için bu sözleşme kesinleştirilebilir. H048 macOS 15.8.2 Apple Silicon sonucu yeni hesap/sohbet kabiliyetlerini kendiliğinden doğrulamaz. H049 ve H052–H054 kullanıcı kararıyla ertelidir. [H012 kapanış kaydı](../proofs/architecture-acceptance.md) mevcut ürün/katalog ve son kalite kapısı fingerprint'ini bağlar.

## H006-c — gerçek GUI sahibi kaybı için yerel kabul planı

**15 Kasım yerel dilim kabul edildi.** Önceden belirlenen aşağıdaki kapsam/eşikle hazır GUI kaybı ve yeniden açılış geçti. [Sonuç](../proofs/windows-gui-crash.md). Ana H006 tamamlanmadı.

15 Kasım 2026. Ana ajan süreç/ayar sahipliği ve kabulü üstlenir. Amaç, H006-b'nin controller fixture'ından ayrı olarak gerçek paketli Tauri penceresinin beklenmedik kaybını doğrulamaktır. Temiz OS, Windows 11, Job atamasından önceki yarış ve üretim yayını kapsam dışıdır. Ana H006 kutusu açık kalır.

Mevcut kaynak ve staged runtime, ayrı uygulama kimliğiyle release modunda derlenir. Test profili yalnız bu kimliğe ait `%LOCALAPPDATA%/dev.headroom.desktop.gui-crash-<id>` altında, kanıt ve WebView profili proje `work/proofs/windows-gui-crash/` içindedir. Gerçek istemci ayarına bağlanılmaz. Native UI Başlat → gerçek manager → packaged controller/helper/engine Ready yolu kullanılır. CLI/model/auth/ajan ayarları değiştirilmez. Sağlayıcı uçları yalnız yerel reddeden fixture'a aittir.

Gözlemciye ait dış Job handle'ı deney boyunca açık tutulur. Bu Job yalnız hata temizliğidir, gözlenen başarıyı üretmek için kapatılmaz. Yalnız başlatılan GUI'nin eldeki Child handle'ına TerminateProcess uygulanır. Önceden açılmış üç motor process handle'ının bitmesi ve motor/sabit ingress portlarının kapanması 5000ms içinde gözlenmelidir. Yabancı listener doğru tanık baytlarını almaya devam etmelidir. Doğal kapanış gözlemi tamamlanmadan dış Job kapatılırsa deney PASS olamaz. Bu GUI deneyi EOF ile Job etkisini birbirinden ayırmaz; mekanizma ayrımı H006-b'nin ayrı kontrol kolundadır.

Aynı izole kimlikle yeniden açılan pencere Stopped olmalı, sabit endpoint kimliği/portu korunmalı ve açık Başlat → Ready → Durdur → normal Çıkış başarılı olmalıdır. Son kontrol, çökmeden sonraki dosya kilidi ve yeniden başlatma durumunu sınar. Tüm sahipli süreç/portlar kapanmalı, fixture model istek sayısı sıfır olmalıdır. Yalnız tek çökme ve tek kurtarma koşusu genel kararlılık veya gecikme benchmark'ı sayılmaz.

Doğrulama `bench/windows/gui_crash.py` ve native WebView sürücüsüdür. Önce/sonra kaynak, iki benchmark dosyası, tam staged payload, exe/dist, araç ve Windows fingerprint'leri sonuçla birlikte kaydedilir. Değişmeyen 344 ürün girdisi ve araç/OS aynıysa son H009 tam kalite kapısı yeniden kullanılır. İlgili ürün kodu değişirse odaklı doğrulama ve son adayda tek tam yerel kapı gerekir. Başarısız koşular sabit neden/metadata ile korunur; ham çıktı veya secret rapora girmez.
