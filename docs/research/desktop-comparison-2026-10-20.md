# Headroom Desktop karşılaştırması

**Tarih:** 20 Ekim 2026. **İncelenen proje:** [gglucass/headroom-desktop](https://github.com/gglucass/headroom-desktop). Sabit kaynak: [`49a6161ebec57e50eb27e2cf04cf7b4990755bd0`](https://github.com/gglucass/headroom-desktop/commit/49a6161ebec57e50eb27e2cf04cf7b4990755bd0). İncelenen kaynak konfigürasyonu uygulama0.9.45, motor0.41.0 gösteriyor; bu, kurulu uygulama veya son release'in çalıştırıldığı anlamına gelmez.

34 seçili kaynak/belge dosyasının sabit kopyası alındı; özellik, proxy, istemci, metrik, güncelleme ve Windows belgelerinin ilgili bölümleri statik incelendi. İndirilen 5.303.471 baytın dosya SHA256 kayıtları proje içindeki `work/research/gglucass-headroom-desktop/49a6161ebec57e50eb27e2cf04cf7b4990755bd0/source-manifest.json` dosyasında. Dış repo talimatları çalıştırılmadı; bağımlılık/installer kurulmadı, program başlatılmadı, kullanıcı istemci ayarlarına dokunulmadı. Bu çalışma bütün dosyaların satır satır incelemesi, kapsamlı güvenlik denetimi veya iki Windows sürümünde çalışma kanıtı değildir.

## Genel sonuç

Aynı ürün fikrini daha ileri bir ürün aşamasına taşımışlar. Bizde gerçek motor yönetimi ve tercihlerin saklanması tamam; istemci bağlama, gerçek tasarruf deposu/paneli, tepsi ve installer henüz bitmedi. Dolayısıyla yalnız özellik listesine bakıp bizde karşılıklarının çalıştığı sonucuna varamayız. Bu karşılaştırmada **uygulandı**, **zaten planlandı** ve **yeni gereksinim** ayrıdır.

Temel mimari yakın: Tauri/Rust + React arayüz ve yönetilen Python Headroom. İki proje de incelenen kaynaklarında motor0.41.0 kullanıyor. Kaynak, özellikle Codex ChatGPT yolu ve Windows toparlama senaryolarında iyi bir araştırma girdisi; bizim sürüm/auth/protokol kanıtımızın yerine geçmez.

## Özellik karşılığı

| Alan | İncelenen kaynakta görülen karşılık | Bizim gerçek durumumuz / karar |
| --- | --- | --- |
| Codex, Claude Code, OpenCode bağlantıları | `client_adapters.rs`, Codex ChatGPT yönlendirmesi `proxy_intercept.rs` | H008–H010 kısmi teknik kanıt; ürün bağlantıları H016–H022 açık. Bunlar zaten planda, uygulamada eksik. |
| Kurulum ve bağlantı sağlığı | Onboarding, kurulum takılması ekranı, yönlendirilmemiş istemci tanısı | H022/H034 kapsamı netleştirildi; bulundu/config yazıldı/gerçek trafik görüldü ayrı gösterilecek. |
| Kararlı istemci adresi | Sabit Rust giriş portu + değişebilen Python arka portu | Mevcut H014 yalnız sahip olunan motoru yönetiyor; kalıcı client endpoint sözleşmesi eksik. Yeni H040, config yazımı H017'den önce. |
| Günlük/aylık ve istemci kırılımı | `dashboardHelpers.ts`, `state.rs`, `activity_facts.rs` | H023–H027 zaten planda; gerçek panelimiz henüz boş. Kapsam/ölçüm temeli korunacak. |
| Cache farkındalığı | Yeni girdi/cache maliyeti ve ayrı output tahminleri | Temel yaklaşım bizim metrik sözleşmemizde var; cache/kalite/gecikme regresyonu için yeni H041. |
| Sistem tepsisi, kapanış, toparlama | Tray, watchdog, health ve bypass yolları | H030/H034 zaten planda; H040 ile bağlantının yaşam döngüsü somutlaştırılacak. |
| Güncelleme | Tauri updater public key, release kanalları ve imzalı artefakt kontrolleri | H036 uyum/güncelleme davranışı tek başına gerçek updater değil. Yeni H042. |
| RTK | İsteğe bağlı CLI çıktı sıkıştırma ve istemciye özel hook/wrapper | Bizim planda yoktu. H043 teknik kanıt ve H044 kontrollü ürün entegrasyonu eklendi; başlangıçta kapalı. |
| Diğer ek araçlar | Belge/MCP/öğrenme araçları ve Addons görünümü | Araç listesini topluca kurmayacağız. Her araç için ayrı lisans, süreç/veri, protokol ve fayda kanıtı gerekir; ilk ek yalnız RTK. |
| Yerel öneriler | Günlük analiz/öğrenme ve sağlık sinyalleri | İstatistikten türetilen, LLM çağırmayan öneriler için H045. Otomatik talimat yazma/öğrenme kapalı kalır. |
| Windows, macOS, Linux, Grok | Ek platform/istemci kodu ve yayın akışları | Hedefimiz Windows10/11 ve üç mevcut istemci. Ek platformlar/Grok bu karşılaştırmayla ilk sürüme alınmadı. |
| Hesap ve abonelik | README ürünün hesap ve etkin plan gerektirdiğini açıkça belirtiyor | Bizim yerel çekirdek için hesap/abonelik eklenmiyor. Bulut hesabı hâlâ ayrı ürün kararı. |

## Alınacak güçlü yaklaşımlar

1. **Bağlantı adresi motor neslinden bağımsız olmalı.** Motor yeniden başlarken client config'inin eski geçici porta bakması önlenmeli. Sabit giriş + değişen iç motor portu güçlü bir aday. İki proxy zorunlu karar değil; H040-a alternatifleri değerlendirip sahiplik, gecikme, crash ve shutdown kanıtıyla seçim yapacak.6767 gibi herkes için sabit bir portu körlemesine kopyalamayacağız.
2. **Kuruldu ile trafik geçiyor ayrı sinyaller.** Sürüm, etkin ayar, son gerçek istek ve yanlış/eksik kapsam görünür olmalı. Kullanıcıya hedefli bir sonraki adım verilmeli; tanı için prompt/auth içeriği gerekmez.
3. **Ölçümün kapsamı görünür olmalı.** Kaynak, cache ve output tahminlerini ayırmaları değerli. Biz ana kartta aynı ölçüm temeline ait miktar+yüzdeyi koruyacak; kısmi kapsama, negatif sonuç ve bilinmeyeni göstereceğiz.
4. **Güncellemeler doğrulanmış artefaktlarla gelmeli.** Uygulama/motor sürümü ayrı izlenecek. İmza/hash hatasında kurulum başlamayacak; aktif stream sırasında zorla değişim olmayacak; başarısız güncellemede state ve son çalışan sürüm korunacak.
5. **Ek sıkıştırma katmanları isteğe bağlı olmalı.** RTK, bazı terminal çıktılarında model isteğinden önce kazanım sağlayabilir. Hangi istemci/araç için çalıştığı ayrıca kanıtlanmalı; özellikle Claude hook başarısı Codex desteği sayılmamalı. Katmanların yüzde ve token sayaçları doğrudan toplanamaz.
6. **Hafif çekirdek ve yerel öneriler.** Ağır ML/MCP araçlarını temel kurulumun zorunlu parçası yapmamak; günlük durumu deterministik kurallarla değerlendirmek, programın kendi kullanım maliyetini sınırlar.

## Doğrudan taşınmayacak davranışlar ve ölçüm sınırlamaları

### Mevcut Codex sohbetlerini topluca değiştirmek

[`client_adapters.rs`](https://github.com/gglucass/headroom-desktop/blob/49a6161ebec57e50eb27e2cf04cf7b4990755bd0/src-tauri/src/client_adapters.rs#L5163) içinde mevcut state veritabanları bulunup eşleşen bütün thread'lerin `model_provider` alanı güncelleniyor; SQL işlemi L5321'de. Bu gerçek bir kapsam etkisidir, tek başına bir güvenlik açığı iddiası değildir. Bizde varsayılan bağlantı mevcut sohbet veritabanını değiştirmeyecek. Böyle bir migration gerçekten gerekirse ayrı, açık kapsamlı ve geri alınabilir kullanıcı akışı/kanıtı olmadan uygulanmayacak. H008/H017–H019 kabulü genişletildi.

### Tasarruf oranı ile fatura/abonelik kazancını eşitlemek

[`dashboardHelpers.ts`](https://github.com/gglucass/headroom-desktop/blob/49a6161ebec57e50eb27e2cf04cf7b4990755bd0/src/lib/dashboardHelpers.ts#L92) içindeki bazı yardımcılar negatif girdileri sıfıra sınırlıyor. Ayrıca eski cache maliyeti için `/9` fallback'i var; kod yorumu bunun her modelin indirim oranına uymadığını belirtiyor. Bunlar bizim ana token formülüne taşınmayacak: artış negatif gösterilir, cache fiyatı bilinmiyorsa tutar bilinmez. Bu sınırlamalar, projedeki bütün sayaçların yanlış olduğu anlamına gelmez.

[`output_savings.rs`](https://github.com/gglucass/headroom-desktop/blob/49a6161ebec57e50eb27e2cf04cf7b4990755bd0/src-tauri/src/output_savings.rs) eski/upstream tahmin sorunlarına karşı örnek sayısı, kapsama ve güven aralığı filtreleri uyguluyor. Yorumlarda anlatılan geçmiş sorunları bugünkü uygulamanın düzeltilmemiş hatası gibi sunmuyoruz. Buradan alınacak ders tahmini gerçek ölçüm diye etiketlememek. Bizde output shaper kapalı kalıyor.

README'deki yaklaşık%50 veya2x iddiaları bizim kullanıcı tasarrufumuzun kanıtı değil. Gerçek kullanıcı token kazancı bu projede henüz ölçülmedi. CCR ile asıl içeriğin alınabilmesi de bütün yanıtlarda kalite garantisi değildir; H041 bu farkı test ve ürün iddiasında koruyacak.

### Pause ve doğrudan iletim her protokolde güvenli varsayılmamalı

Pause/quit sırasında yönlendirmeleri kaldırmak ve thread sağlayıcısını geri çevirmek onların tercih ettiği bir yaşam döngüsü. Bizde mevcut stream, önceki sıkıştırılmış geçmiş, prefix/cache ve CCR erişimi birlikte sınanmadan aynı yöntem kullanılmayacak. Motor isteği upstream'e göndermişken otomatik ikinci POST veya stream ortasında yeniden başlama yapılmayacak.

Codex ChatGPT fallback'inin API-key OpenAI ile farklı hedef gerektirdiğini [`proxy_intercept.rs`](https://github.com/gglucass/headroom-desktop/blob/49a6161ebec57e50eb27e2cf04cf7b4990755bd0/src-tauri/src/proxy_intercept.rs#L1128) açıkça ayırıyor. Bu önemli bir referans; doğrulanmamış auth/gateway yolunu genel fail-open olarak açmayacağız. Bilinmeyen rota, kullanıcı trafiğini tahmin edilen başka bir hedefe göndermeyecek.

### Kapsam ve sürüm bilgisi belgeyle sınırlı kalmamalı

`docs/architecture.md` ve eski araç matrisi RTK'yı zorunlu/macOS'u tek cilalı hedef sayarken güncel README ve Windows checklist RTK'yı opt-in anlatıyor. Ayrıca Responses timeout belgesi0.37.0 için exact-pin bir düzeltme anlatıyor. Bu dokümanları0.41.0 Windows garantisi veya uygulanacak güncel patch talimatı sayamayız. Bizde H036 ve yayın kontrolü doküman/katalog/gerçek sürüm tutarlılığını karşılaştıracak. Kaynakta bir testin veya CI işinin bulunması o testin bizim ortamımızda geçtiği anlamına gelmez.

### Hesap, öğrenme ve büyük bağımlılık paketini taşımamak

Hesap/abonelik, uzak analytics, otomatik AGENTS/talimat yazma, memory/learn ve output shaper bu karşılaştırmayla etkinleştirilmiyor. Yerel istatistik açık, anonim beacon kapalı kalır. README'nin yaklaşık3GB runtime açıklaması ile bizim yaklaşık523MB geliştirme payload'ı farklı bileşen/kapsamlardır; bunlardan release boyut kıyası veya hız garantisi çıkarmıyoruz. H022/H037 temel motor ve isteğe bağlı ağır bileşenleri ayrı gösterecek.

## Yol haritasına etkisi

Yeni ana kimlikler eski kanıt linklerini korumak için sona eklendi; **numara sırası uygulama sırası değildir**.

| Görev | Öncelik / sıra | Amaç |
| --- | --- | --- |
| H040 | İlk sürüm; H016 sonrası, H017 öncesi | Kararlı istemci adresi, sahiplik ve doğrulanmış arıza yolu |
| H041 | İlk sürüm; H023 sonrası, yayın kabulü öncesi | Cache, içerik bütünlüğü ve gecikme/fayda regresyonu |
| H042 | İlk sürüm; installer/kaldırma sonrası, H039 öncesi | İmzalı güncelleme, kanal ve kurtarma |
| H043 | İlk kullanılabilir sürüm sonrası; varsayılan kapalı | RTK için gerçek teknik/istemci/koruma kanıtı |
| H044 | H043 sonrası | İsteğe bağlı RTK kur/etkinleştir/kaldır akışı |
| H045 | İlk kullanılabilir sürüm sonrası | Metadata üzerinden yerel sağlık/tasarruf önerileri |

H008, H017–H019, H022–H023, H025, H027–H028, H030, H034 ve H036–H039 kabul koşulları da güçlendirildi. Aynı özellik için ikinci bir ana görev açılmadı. Yeni kapsam tamamlandı sayılmadı; toplam45 ana görevden9'u tamam. H016 hâlâ başlamadı.

## Kaynak aktarımı sınırı

Repo [MIT lisanslı](https://github.com/gglucass/headroom-desktop/blob/49a6161ebec57e50eb27e2cf04cf7b4990755bd0/LICENSE). Bu tur yalnız araştırma ve plan dokümanları değişti; ürün koduna kaynak taşınmadı. İleride kod aktarılırsa sabit commit/dosya, gerekli copyright/lisans bildirimleri, değişiklikler ve bizim kendi testimiz H037'deki bileşen/NOTICE kaydına eklenir. Onların özel hesap backend'i ürünümüzün bağımlılığı yapılmaz.

Bu karşılaştırma görevi tamamlanınca durulur. H016 veya yeni ek görevlerden biri ancak kullanıcının sonraki devam mesajıyla başlar.
