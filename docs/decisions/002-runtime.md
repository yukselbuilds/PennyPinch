# 002 — Yerel masaüstü mimarisi

**Tarih:** 19 Ekim 2026. **Durum:** H012-a mimari temel kararı; yerel motor/protokol kanıtlarına dayanır. Ürün uygulaması ve H012'nin kalan kabulü açık; bu karar yayın kabulü değildir.

## Seçim

Tauri 2 / Rust yönetici, React / TypeScript arayüz ve uygulamaya ait CPython 3.13.13 + `headroom-ai[proxy]==0.41.0` runtime dizini kullanılacak. Windows 10 22H2 ve Windows 11 x64 hedeflenir. Arayüz yerel paketlenir; sistem WebView2 kullanır. Yeni Node/Rust/JS bağımlılıklarının kesin sürümleri H013'te mevcut araç zinciriyle doğrulanıp lock dosyalarına yazılır; bu belge en güncel sürüm iddiası taşımaz.

H012-a için bu bileşen seçimi sabittir; başka masaüstü framework'ü veya genel amaçlı yardımcı servis eklenmez. Tauri DTO'ları `schema_version=1`, üretim helper stdio JSONL mesajları `protocol_version=1` kullanır; bu iki sürüm alanı birbirinden bağımsızdır. H006 proof helper'ının yaşam döngüsü yerel kanıttır, son ürün helper/IPC uygulaması değildir. Başlangıç arayüz yönü Graphite'dır; H013 görsel kabukta gerçek durumu ve boş metriği gösterecek.

H006-a taşınabilir runtime dizinini ve konsolsuz üst süreç → gizli konsollu helper → motor yaşam döngüsünü kanıtladı. Dizinin mevcut boyutu 523.394.231 bayttır. İlk çalışan dilim bu doğrulanmış yaklaşımı kullanır; boyut azaltma için bağımlılıkların körlemesine kaldırılması ayrı bir iş ve yeniden paket doğrulaması gerektirir. Son kullanıcıya geliştirme Python/Node kurulumu yüklenmez.

## Süreç ve güven sınırı

- WebView yalnız dar Tauri komutlarını çağırır. Genel shell, keyfi URL/port/path üzerinden proxy yönetimi ve uzak sayfa çalıştırma yetkisi verilmez. UI içerik veya secret taşıyan motor debug/log nesneleri almaz.
- Rust yönetici, kendi runtime nesli için helper handle/PID ve loopback sahipliğini izler. Helper/manifest sürümü doğrulanır; listener ve pinned readiness uyuşmadan `ready` yayımlanmaz. Başka motorun bulunduğu port sahiplenilmez veya kapatılmaz.
- Helper kontrolü boyut sınırlı stdio JSON mesajlarıyla yürür. Engine logları ayrı ve içeriksiz durum kodlarına indirgenir. stdout'taki herhangi bir JSON satırı tek başına kimlik/ready kanıtı sayılmaz.
- Rust yönetici tek runtime nesli için tek sahip olur; her istek/yanıt `request_id`, motor yanıtları `runtime_generation` ile bağlanır. Unknown snapshot `ready` sayılmaz. Sabit UI hata zarfı ve başlangıç durumları [yerel sözleşmede](../contracts.md) tanımlıdır; ham child-process çıktısı UI DTO'suna girmez.
- Uygulama state kökü runtime payload'ından ayrıdır. Sabit bundle dosyaları değiştirilmez. Bağlantı ayarları ve kullanıcı tercihleri dayanıklı/atomik yazılır; mevcut giriş sırları uygulama state'ine kopyalanmaz.
- Normal kapanış stdio isteği ve bounded drain ile başlar. Yalnız sahip olunan süreçler için timeout/Job Object fallback vardır; force kapanış graceful diye sunulmaz. H009'un test controller'ındaki force sonucu, H006-a helper yaklaşımını geçersiz kılmaz ve gerçek Tauri bağlantısının kanıtı yerine geçmez.
- H007'deki opaque-block cache marker uyarlaması proof-scope'tur. Üretimde yalnız paket içinde sürüm ve tam `prefix_tracker.py` source-hash guard'ı ile bulunabilir; uyumsuzlukta bootstrap durur ve motor ready olmaz. H006 paketini/manifestini sessizce değiştirmek veya guard olmadan patch uygulamak kabul edilmez. Üretim shim paketi, manifest ve readiness testi H014/H028'in açık işi olarak kalır.

## İstemci adaptörleri

Keşif, değişiklik planı, uygulama, doğrulama ve geri alma ayrı işlemlerdir. Keşif dosya değiştirmez. Plan etkin model/reasoning/auth yöntemini ve kapsamı koruyarak yalnız gerekli yönlendirme alanlarını önerir. Uygulama anında fingerprint tekrar kontrol edilir; geri alma sonradan yapılan kullanıcı değişikliğini ezmez.

İstemci sürümü, yüzey, auth, protokol/transport ve capability ayrı uyumluluk boyutlarıdır. Başlangıç kayıtları [`compatibility/clients.json`](../../compatibility/clients.json) içinde sürüm ve kanıtla tutulur. API-fixture başarısı canlı OAuth veya desktop/subagent desteğine yükseltilmez. Kurulum bulunamaması, desteklenmiyor demek değildir. Doğrulanmamış kombinasyon doğru etiketlenir; uygulama model/efor/ajan sayısını sessizce değiştirmez.

Codex'in proje config'inde yok sayılan yönlendirme anahtarlarına güvenilmez; süreç/kullanıcı kapsamı ve mevcut ChatGPT gateway yolu ayrı doğrulanır. Claude bare/API-fixture ve OpenCode V1/V2 aynı destek kaydı değildir. Kullanıcının açık oturumlarını otomatik yeniden başlatmak veya auth dosyalarını yeni fixture'a kopyalamak entegrasyon yöntemi sayılmaz.

## Duraklatma

Motoru durdurmak, istemci bağlantısını kaldırmak ve sıkıştırmayı duraklatmak farklı işlemlerdir. Duraklatma mevcut stream'i bitirmeli, geçmiş CCR deposunu erişilebilir tutmalı ve önceki prefix'i bozmamalıdır. Yalnız `HEADROOM_OPTIMIZE=0`, bypass header veya proxy restart uygulayarak bu sözleşmenin sağlandığı varsayılmaz.

H011-a kaynak incelemesi [`pause.md`](../proofs/pause.md) içinde kaydedilmiştir. Pinned sürümde bypass yalnız bir request'in compression kararını etkiler; runtime-env hot-reload allowlist'inde `HEADROOM_OPTIMIZE` yoktur ve `/settings/apply` proxy'yi yeniden başlatır. Bu kontrollerin hiçbiri aktif stream/prefix/CCR sözleşmesini kanıtlamaz. Durum `not_implemented/not_tested`; H011/H028 kabul edilene kadar üretim UI'sı pause eylemi veya başarılı `paused` durumu sunmaz. H011/H028, geçişin gerçek request snapshot ve prefix davranışını belirleyecek.

## Ölçüm

Ana kart `Σ(B-A)` ve `100×Σ(B-A)/ΣB` çiftini aynı kapsamdan gösterir. İçeriksiz alan seçimi, aynı tokenizer temeli, event dedup, runtime restart ve kayıp aralığı normalizasyon sözleşmesine dahildir. Ham `/stats` yüzdeleri ürün anlamı doğrulanmadan kopyalanmaz; kaynak analizi [metrik notunda](../research/metrics-0.41.0.md).

İstek düzeyindeki wire azalması, benzersiz içerik kazanımı, yeni içerik/cache ve maliyet farklı temellerdir. Tahmini fatura karşılığı ana token metriğiyle eşitlenmez. Atanamayan proje/ajan/model alanları tahmin edilmez. Panel raporu için LLM çağrısı gerekmez.

## Uygulama ve yayın sırası

H012 sözleşme/katalog, bilinen sınırları temsil ederek uygulanabilir bir taban oluşturacak. H013'te Graphite kabuğu gerçek backend durumunu ve boş ölçümleri gösterecek; henüz uygulanmayan eylemler demo başarı üretmeyecek. H014 gerçek helper bağlantısı, H015–H018 state/keşif/plan/işlem, H019–H021 doğrulanan istemci dilimleridir.

H012-a yalnız teknik temeli, sürümlü DTO/IPC sınırını ve ilk içeriksiz uyumluluk kataloğunu karara bağlar. Kalan H012 kabulü, H014 helper/Tauri bağı, H011/H028 pause, canlı OAuth/desktop/subagent/CCR client yolları ve temiz Windows 10/11 kapıları açık kalır. Uygulama iskeletinin geliştirilmesi destek/yayın sözü değildir; [`compatibility/clients.json`](../../compatibility/clients.json) kayıtları yalnız gerçek kanıt gelince güncellenir.
