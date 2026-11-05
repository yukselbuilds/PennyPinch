# H025 — canlı metadata ve Genel bakış kabulü

5 Kasım 2026, Windows10 22H2 x64/build19045. [Plan013](../decisions/013-live-overview.md), [ana ajan incelemesi](../reviews/H025.md). Ana ajan runtime/nesil, ölçüm, kalıcı kayıt, bağlantı güveni ve kabulü üstlendi. Aynı tek GPT-6 Luna xhigh çalışanı yalnız sabit props'lu üç sunum dosyasını ve ayrı frontend sözleşme testlerini yazdı; birleştirilmiş düzeltmeler sonrası completed/durdurulmuş durumu doğrulandı. Ana ajan gerçek bağlı akışı inceledi ve kalan uygulamayı tamamladı. Model/efor değişmedi; geliştirme token tasarrufu veya süre uydurulmadı.

## Sonuç

- Uygulama açıkken5 saniyede bir yalnız kendi ready motorundan metadata toplar. Exact peer PID, held lease ve nesil denetlenir; sabit loopback rota,2MiB gövde ve1.5s HTTP bütçesi korunur. Prompt/auth içeriği depoya veya renderer'a geçmez.
- H023 → H024 upsert; tekrar polling toplamı şişirmez. Motor kapalıyken geçmiş okunur. İzleme oturumu uygulamanın bu açılışıdır; istemci sohbeti/hesabı değildir.
- Session/bugün/7 yerel gün, aynı toplamdan token ve yüzde, karşılaştırılabilir sparse grafik. Boş/bilinmeyen sıfır olmaz; eksik/karma temel veya sayaç taşması ortak yüzde üretmez. Negatif sonuç açık artıştır. Kaynak/zaman/saklama/kısmi kapsam görünür.
- Tek sonraki eylem ve gerçek bağlantı kaydı; eski/karışık hedef otomatik seçilmez. Hesap, açık sohbet veya model trafiği kaydedilmiş bağlantıdan türetilmez. Teknik ayrıntı açılır bölümlerdedir. Ayarlar100/125/150 yalnız pencere ölçeğidir.

## Doğrulama

| Senaryo | Gözlenen / kapsam |
| --- | --- |
| Sahipli HTTP → SQLite | Fixed GET, doğru peer/lease/nesil, büyük ama bounded metadata, malformed ve owner reddi. Aynı HTTP kaynağı iki kez → tek event; body/auth canary dışarı çıkmaz. Bu yerel fixture gerçek sağlayıcı değildir. |
| Replay/generation/source hatası | Collector devam eder; yeni nesil ayrı kimliktir. Kaynak hatası geçmişi silmez, kesinti görünür. Stopped kaynak motoru başlatmaz. |
| Dönem/yarış | Geç yanıt yeni dönemin etiketinde görünmez; aynı dönem hatası önceki görüntüyü stale tutar. Grafik/toplam aynı yerel sorgudan; saat/gün kovaları ve karma temel kontrolü PASS. |
| Gerçek pinned motor | Uygulamanın gerçek Başlat/native manager yolu ready oldu; collection recorded. Overview dışındayken yeni capture; UI Durdur → geçmiş ve capture zamanı korundu; restart → farklı motor nesli/aynı izleme kimliği. Model isteği0, olay0, yüzde bilinmiyor. |
| Sayısal UI | İzole fixture120.000→75.000 =45.000/%37,5; dönem değişimi%14; artış%25; gerçek0, B=0, unknown/mixed, stale/hata ve TR/EN geçti. Bunlar gerçek kullanıcı tasarrufu değildir. |
| Gelişmiş ve eşzamanlılık | Gerçek readonly Gelişmiş açıkken rescan PASS. Ayrı modül mock'u native çağrıları dışlayan fixture'da verify beklerken collapse ve gezinme kilidi korunur; tamamlanınca açılır. Native doğrulama/provider testi yapılmadı. |
| Bağlantı seçimi | Native planda eşdeğer2–4 adayın geçici birleşimi ve farklı/stale/unsupported planın reddi unit testlerinde PASS. Gerçek kurulu üç CLI/Desktop adayı için eski seçim unavailable; otomatik başka hedefe düşülmedi. Mevcut config/ownership değişmedi. |
| Ekran/klavye/ölçek |1280×800/1024×720 ve dar CSS viewport kontrolleri, dönem klavye odağı, sıfır pageerror. Ayarlar → gerçek Tauri WebView zoom125/150, devicePixelRatio1.25/1.5 ve100'e dönüş; yatay taşma yok. OS DPI değişmedi, onun testi sayılmaz. |
| Kapanış | Son UI/live/zoom pencereleri WM_CLOSE→exit0; sahipli Job ve debug portları kapandı. Normal Codex config, preferences, onboarding ve ownership fingerprint'leri aynı. |

Son aday `work/proofs/overview/final2.json`: **120 frontend PASS,327 Rust PASS,5 mevcut opt-in ignored**, TypeScript/Vite build, fmt ve offline embedded build PASS. Focused kayıtlar `generation-red3.json`, `generation-green2.json`, `generation-transport-red.json`, `generation-transport-green.json`, `metrics-green3.json`; başlangıç testlerinin tireli kimlik varsayımı canlı kabulde geçersizleşti, bunun gerçek32hex regresyonu eklendi. İlk son-aday kapısı canlı kabulden sonra değişen kaynak için tekrar kanıt sayılmadı. Ham hata/HTTP gövdesi saklanmadı; yalnız sabit metadata/sonuç/fingerprint kayıtları tutuldu.

Güncel native kanıt `ui-latest.json`, `live-latest.json`, `zoom-latest.json`; bütünleştirilmiş uygunluk `work/proofs/overview/acceptance.json`. Bunlar proje içi geliştirme artefaktlarıdır, Git'e gerçek app-state veya browser profili girmez. Rust1.92.0, Node25.2.1, pnpm11.25.0, Python3.14.0. Yeni browser, Vite sunucusu veya Codex kurulumu başlatılmadı. CodeGraph kaynak değişimi sonrası proje launcher'ı üzerinden sync edildi; graph test kanıtı değildir.

## Ortam olayı ve sınırlar

Windows ayrılmış port aralığı51717–51816 eski geliştirme uç noktasını10013 ile engelledi. OS rezervasyonu değiştirilmedi. Aktif sahiplik olmadığı ve bilinen config'lerin eski portu kullanmadığı doğrulandı; yalnız geliştirme app-state uç noktası yedek/atomik yazım/fingerprint ile52175'e yenilendi. Üç released ownership kaydı korundu. Yardımcının ilk revision artırımı fixed1 sözleşmesine düzeltildi; native doğrulayıcı gevşetilmedi. Kanıt `work/proofs/overview/endpoint-refresh/0ae1a8d8-3ff5-4680-8bb9-d3753d720d3a/`.

Pinned0.41.0 sayaçlarının karşılaştırılabilir tokenizer temeli bilinmediği sürece gerçek olaylar gözlem olarak saklanır, ana kart tasarruf uydurmaz. Model/ajan/platform dağılımı ve hesap/sohbet listesi bu görevin kabulü değildir. Windows11/macOS/Linux, installer, gerçek OS DPI, güç kesintisi, uzun çalışma/yük ve canlı API-key bu koşuda test edilmedi. H026 sonraki görevdir; H041 canlı API-key kabulü açık kalır. H025 bitince çalışma durur.
