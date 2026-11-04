# Optimizasyon hedefleri ve kanıt sınırları

**Tarih:** 4 Kasım 2026. Bu kısa not, H012/H025/H034/H041/H043/H044 için kaynak-hedef eşlemesidir; yeni ana görev, ürün kabiliyeti veya ölçülmüş tasarruf iddiası açmaz. RTK ve karşılaştırma kaynakları ana ajan tarafından aynı gün okundu; bu değişiklikte kaynak kodu çalıştırılmadı.

## Karar

- Ana bağlantı mimarisi, istemcinin desteklediği endpoint üzerinden Headroom'un yerel proxy/motor yoludur.
- RTK-OSS'un Apache-2.0 terminal filtreleri isteğe bağlı, tamamlayıcı adaydır; varsayılan kurulumun parçası değildir. RTK engine ve ayrı proxy BUSL-1.1 olarak ayrı lisans/dağıtım kararı bekler; otomatik içeri alma veya dağıtım kararı verilmez.
- Model, reasoning, auth yöntemi ve ajan sayısı aynen korunur. Ucuz modele geçiş, otomatik model/reasoning yönlendirmesi, memory/learn, global shell/hook kurulumu yoktur.
- Bilinmeyen auth/rota/protokolde kör bypass yapılmaz. İstemci güncellemesi sonrasında yüzey ve trafik kanıtı gerektiğinde yenilenir; not_tested uyumsuzluk veya bozukluk değildir.

## Kaynak → ürün hedefi

| Kaynakta görülen | Planlanan hedef | Kanıt sınırı |
| --- | --- | --- |
| [RTK v1.1.0 README](https://github.com/rtk-ai/rtk/blob/v1.1.0/README.md), 25 Ekim 2026'da yayımlandı; 4 Kasım GitHub API incelemesinde en güncel stable sürüm olarak görüldü. Release API'sindeki `target_commitish` değeri [`753a3b0e939f2e4490fc24e3a1984010f40a103c`](https://github.com/rtk-ai/rtk/commit/753a3b0e939f2e4490fc24e3a1984010f40a103c): hook/wrapper belirli terminal komutlarını RTK eşdeğerine çevirebilir. | H043 yalnız hangi araç/komut/shell/client yolunun gerçekten yakalandığını sınar; H044 yalnız başarılı ve yararlı olan filtreyi kullanıcı opt-in’iyle sunabilir. | Hook başarıları başka client'a taşınmaz. Bash hook Windows PowerShell, Codex Desktop veya uygulamanın yerel Read/Grep araç çağrısını kanıtlamaz. |
| RTK sayaç anlatımındaki gain yaklaşık bytes/4 tahminidir. | H041 komut baytı değişimini ayrı byte metriği olarak; H043 aynı girdinin RTK kolunda bu tahmini ek etiketle gösterir. | Bu değer gerçek/sağlayıcı token ölçümü değildir; tahmin olarak etiketlenir, token farkı veya fatura yerine kullanılmaz. |
| [RTK proxy README](https://github.com/rtk-ai/rtk/blob/develop/proxy/README.md) rtk claude örneği verir. | H043/H044 kapsamı dışındaki kaynak/lisans karşılaştırmasıdır. | develop değişebilir ve gerçek uygulama için pinlenmelidir. Bu not proxy kurulumunu/uygulamasını veya Codex/OpenCode desteğini planlamaz. |
| Pinned masaüstü karşılaştırması: [20 Ekim 2026 araştırması](desktop-comparison-2026-10-20.md), upstream gglucass/headroom-desktop commit 49a6161ebec57e50eb27e2cf04cf7b4990755bd0. | H025 aynı kapsamdan ölçülen token miktarı+yüzdeyi, kaynağı ve son güncellemeyi sunar; H034 hata/sonraki adımı sadeleştirir. | Kaynak özellikleri ürünümüzde uygulanmış veya ölçülmüş sayılmaz. Buradaki araştırma kendi eski veri/sürüm sınırlarını korur. |
| 4 Kasımda gözden geçirilen ek sabit kaynak: [gglucass/headroom-desktop commit 67fb0e164df942b71166aeb878791489abf41a9f](https://github.com/gglucass/headroom-desktop/commit/67fb0e164df942b71166aeb878791489abf41a9f), seçili README ve Codex adapter/proxy dosyaları. | H025 sade kullanım ve ölçüm sunumunu; H034 yüzey/rota bazlı hata ayrımını hedefler. | Kaynak incelemesi özellik karşılaştırmasıdır; Headroom uygulaması, canlı trafik veya tasarruf kanıtı değildir. |

## H041 tabanı ve H043 genişletmesi

H041 ilk sürüm kapısı yalnız iki kolu ister: **kontrol** (Headroom yok) ve **Headroom**. Aynı sabit fixture/yük, istemci yüzeyi, model, auth, reasoning, ajan ayarı, sürümler, girdi ve araç sonucu kullanılır; soğuk ve sıcak cache ayrı eşleştirilir. Tekrar sayısı, yük/süre ve başarı/gecikme eşikleri ölçümden önce kaydedilir.

H043 isteğe bağlı RTK değerlendirmesinde iki kol eklenerek dört kollu kıyas tamamlanır: **RTK filtresi** ve **RTK → Headroom birlikte**. H041 taban sonucu yalnız ilgili fixture, ayar, makine/OS, sürüm ve yük fingerprint'i aynıysa yeniden kullanılır; aksi halde fark ayrıca kaydedilir. Gerçek sağlayıcı deneyi otomatik başlamaz ve ayrıca yetkili kapsam ister.

Her kol için kaynak/sayısal değer/kimlik/JSON bütünlüğü, hata ve exit code, stderr sonucu, katman sırası, komut baytları, karşılaştırılabilir token temeli, sağlayıcının cache/output sayaçları, ilk yanıt/toplam gecikme ve CPU/RAM ayrı kaydedilir. Ham konuşma/secret rapora alınmaz. Kalite ölçümü yapılmadıysa byte-exact replay kalite garantisi değildir.

- **Komut baytı:** Önce/sonra byte sayısı; RTK’nın bytes/4 tahmini ayrı etiketlenir.
- **Toplam girdi tokenı:** Aynı ölçüm temeliyle önce/sonra çiftinden hesaplanır; temeller uyuşmuyorsa sonuç bilinmiyor kalır.
- **Fatura/maliyet:** Yalnız resmî billable kullanım ölçümüyse ölçülmüş, sürümlü fiyat kaynağı kullanılıyorsa tahmini olarak etiketlenir. Abonelik kotası veya katman yüzdelerinden tutar türetilmez.

Katman yüzdeleri toplanmaz. Birleşik kol, yalnız aynı iş/istek bağında ve açık önce-sonra ölçümü varsa kendi sonucu olarak raporlanır. Önceden belirlenen fayda eşiği karşılanmıyorsa, veri/çıkış bütünlüğü bozuluyorsa veya anlamlı gecikme/hataya yol açıyorsa araç destek kapsamına alınmaz. Gerçek kullanıcı kazancı veya kalite garantisi bu plandan çıkmaz.

RTK etiketinin commit ile bire bir eşlemesi bu tur doğrulanmadı. Etiket sorgusunu içeren komut otomatik denetimde reddedildi ve tekrar denenmedi. H043 uygulamasından önce kullanılacak gerçek sürüm/artefakt ve lisans bağımsız olarak sabitlenmelidir.
