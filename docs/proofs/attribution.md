# H026 — model ve ajan kapsamı kabulü

5 Kasım 2026, Windows10 22H2 x64/build19045. [Plan014](../decisions/014-attribution.md), [root inceleme](../reviews/H026.md). H025 toplamı/rotası korunarak model/provider gözlemi ve kanıtlı attribution sözleşmesi eklendi; model/istemci/ajan seçimi veya filtre kurulmadı.

Ana ajan provenance/origin, normalizasyon, kalıcı kayıt ve parser/kabul sahibi. Aynı tek GPT-6 Luna xhigh çalışanı yalnız Coverage.tsx/coverage.css sunumunu yazdı, sonra completed/durdurulmuş olarak devralındı. Çalışan modeli ürünün destek koşulu veya zorunlu test modeli değildir. Geliştirme token tasarrufu/süre ölçülmedi.

## Davranış

- Owned pinned metadata satırındaki model/provider etiketleri bounded olarak korunur. Motorun model adından yaptığı uygulama/ajan tahmini, tags, raw agent/client/role ve raw attribution nesnesi alınmaz.
- Native request-route witness aynı runtime/proxy origin'e bağlanmışsa istemci, opaque upstream ve mevcutsa opaque ajan/ebeveyn/rol taşınabilir. Bugünkü pinned adapter böyle bir witness üretmediğinden gerçek client/agent durumları unknown kalır; kullanıcı bağlantısı veya aynı model adı bunları verified yapmaz. Bu bilinmezlik optimizasyonu engellemez.
- Eski kayıtlar okunur; aynı event enrichment/replay ile tekrar sayılmaz. Metadata çatışması token değerini bozmaz, yeniden metadata replay'iyle kendiliğinden çözülmez. Kalıcı tablo/sürüm değişmedi.
- Dönem toplamı ve gruplar aynı event kümesidir. Aynı origin'in katmanları tek istek kimliği sayılır; korelasyonsuz kaynaklar fiziksel çağrı olarak birleştirilmez. Farklı upstream ve iki aynı-model ajan ayrı kalır.64 satırı aşan gruplar açık Diğer satırında korunur. Sayılar API faturası/hesap kullanımı değildir.
- Graphite tablo istemci/model, açık rol ve kısa opaque kimlik, istek kaydı ve karşılaştırılabilir input/% miktarını gösterir. Karşılaştırılamayan legacy sayaçlar hâlâ `—` kalır. Atama statüsü unknown/partial/verified yalnız gözlenen alt kümeyi kapsar; açık sohbet/hesap listesi değildir.

## Kanıt

| Senaryo | Gözlenen |
| --- | --- |
| Aynı model/iki ajan | İki ayrı role/opaque identity; aynı modele göre birleştirme yok. Aynı modelin farklı upstream'leri de ayrı. Native witness girdileri fixture'dır; canlı sağlayıcı kanıtı değildir. |
| Eksik ve sahte metadata | Model adında codex/claude bulunması, tags/agent/client iddiası ve ham attribution nesnesi rol/client üretmez. Eksik rol kısmi, bütün eksik atama unknown kalır. |
| Yanlış origin/identity | Farklı request ID'ye taşınan witness, raw URL upstream, self-parent/main-parent ve geçersiz/uzun etiket reddi; ham ID/auth canary çıktı/depo/UI'ya geçmez. |
| Metadata/metrics bağımsızlığı | Unknown → model enrichment ve absorbing model/agent/upstream çatışması token50 farkını korur. Aynı origin'in katmanları events2/requests1; metrik çift sayımı yok. |
| Depo/reopen/dönem | Attribution alanı olmayan eski payload decode edilir. Aynı event zenginleşir; yeniden açılışta model ve tek kayıt korunur, yeni izleme oturumu ve önceki gün kapsamı ayrıdır. |
| Grup sınırı |80 grup →63 ayrıntı +17 grupluk Diğer; istek80 ve fark4.000 korunur. Eksik/kırpılmış grup veya sahte kesinlik strict decoder'dan geçmez. |
| Gerçek owned HTTP | Sabit stats GET/exact peer/lease → pinned adaptör → SQLite → Overview; model/provider korunur, raw client/role/agent canary witness olmaz; iki capture tek event kalır. Yerel HTTP fixture'dır. |
| Native uygulama | Gerçek session/today/seven_days yanıtlarında attribution event/origin toplamları geçmişle eşleşti. Pinned motor ready → collection recording; yeni DTO renderer tarafından okundu. Gerçek kaynakta0 istek, model çağrısı0; hiç canlı kimlik/yüzde uydurulmadı. |
| UI | İki aynı-model ajan, ayrı rota, partial/unknown, metadata conflict ile korunmuş yüzde, artış ve Diğer tablosu; TR/EN, klavye scroll odağı,1280×800/1024×720/dar viewport ve gerçek WebView125/150/100 ölçeği PASS. Sayısal/kimlikli satırlar izole modül mock'lu fixture'dır. |
| Kapanış | Son native pencere WM_CLOSE→exit0, sahipli Job/debug port cleanup; normal Codex config/preferences/onboarding/ownership fingerprint'leri korundu. |

İlk8 yeni Rust testi RED, ardından50 metrik PASS; actual HTTP metadata testinin yeni assert'leri ayrıca PASS. Frontend attribution5 test önce1 PASS/4 RED, sonra5 PASS; mevcut metrik client ile17 PASS. Son adayın tek tam kapısı **125 frontend/335 Rust PASS**,5 mevcut opt-in ignored; TypeScript/Vite build, fmt ve offline embedded build PASS. Ara dilimlerin kapısı tekrarlanmadı. Son inceleme sonrası kaynak/copy değişimiyle yeni EXE üretildiği için native kanıt güncel artefakt üzerinde yenilendi.

`work/proofs/attribution/`: `pure-red.json`, `pure-green.json`, `metrics-green.json`, `pure-green2.json`, `source-green.json`, `decoder-red.json`, `decoder-green.json`, `decoder-integrated-green.json`, `final.json`, `ui-latest.json`, `acceptance.json`. Son kaynak/config girdisi128; önce/sonra fingerprint, Rust1.92.0/Node25.2.1/pnpm11.25.0/Python3.14.0 ve Win10 ortamı aynı doğrulama adımında kaydedildi. Ham failure/HTTP/secret kaydı tutulmadı. CodeGraph proje launcher'ıyla sync edildi; keşif sonucu test kanıtı sayılmadı. Browser/Codex/Vite sunucusu kurulmadı; yeni bağımlılık yoktur.

Bu kabul yeni bir canlı model/auth/ajan veya başka OS desteği iddiası değildir. Ücretli sağlayıcı veya CI çağrısı yapılmadı. Windows11/macOS/Linux, installer/güç kesintisi ve uzun yük ölçülmedi; API-key H041, hesap/sohbet H050/H051 kapsamı sürer. Sıradaki H027 başlamaz; ana görev sonunda kullanıcı devamı beklenir.
