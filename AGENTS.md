# Headroom Desktop çalışma kuralları

## Amaç ve kaynaklar

- Kanonik çalışma kökü `D:\Projeler\Headroom`.
- Ürün kapsamı `HEADROOM_DESKTOP_SPEC.md`, iş sırası ve kabul koşulları `TASKS.md`.
- Kararlar `docs/decisions/`, doğrulama özetleri `docs/proofs/`, güncel devir noktası `docs/IMPLEMENTATION_STATUS.md` içinde tutulur.
- Windows 10 22H2 x64 ve Windows 11 x64 hedeflenir. Bir işletim sistemindeki başarı diğerinin kanıtı sayılmaz.
- Referans alınan an unrelated project projesinin finans, dosya ve çalışma ortamı kuralları bu projeye taşınmaz.

## Ana ajan ve tek çalışan

- Ana ajan kullanıcının seçili modeli ve reasoning ayarıyla koordinasyon, kapsam, karar, inceleme ve sonuçlandırmadan sorumludur. Ana model ayarı değiştirilmez.
- Anlamlı uygulama işleri tam bir çalışan ajana verilir: **GPT-6 Luna, xhigh**. Aynı anda ikinci çalışan veya alt çalışan açılmaz.
- Başlangıç parametreleri: `model="gpt-6-luna"`, `reasoning_effort="xhigh"`, `fork_turns="none"`.
- Bu model/efor tercihi yalnız bu depoyu geliştiren çalışan ajana aittir; ürün desteği veya kullanıcının diğer sohbetlerindeki ajan modelleri için koşul değildir. Headroom model adından bağımsız tasarlanır. Ürün doğrulaması farklı ana/alt ajan modellerini ve model değişimini kapsar; geliştirici çalışanının modeli zorunlu test modeli olarak sunulmaz.
- Çalışan ajan başka ajan açmaz. Ana ajan aynı uygulamayı paralel yeniden yazmaz.
- Aynı çalışan somut, sıralı görevlerde yeniden kullanılır. Sırf gecikme/timeout nedeniyle yeni ajan açılmaz.
- Her devir görev kimliği, sınırlı amaç, kabul koşulları, ilgili dosyalar, izinli değişiklikler ve odaklı doğrulamayı içerir. Tüm sohbet geçmişi kopyalanmaz.
- Varsayılan dilim yaklaşık 1–5 kaynak dosyasıdır; kurulum/bağımlılık dosyaları ayrıca sayılabilir. Daha büyük iş alt dilimlere bölünür.
- Çalışanın dönüşü: değişen dosyalar, gerçek davranış, kısa doğrulama sonucu/fingerprint, sınırlamalar ve sonraki somut adım. Ana ajan tek toplu inceleme/düzeltme listesiyle döner.

## Tasarruf ve kanıt

- Compact bir devam olayıdır; geçerli kanıtı geçersiz yapmaz.
- PASS yalnız ilgili kaynak/config, bağımlılık/araç zinciri, komut ve ortam fingerprint'leri eşleşiyorsa yeniden kullanılır.
- Aynı başarılı kontrol tekrarlanmadan önce somut geçersizleşme nedeni söylenir. Neden yoksa tekrar çalıştırılmaz.
- Uygulama sırasında odaklı test kullanılır. İnceleme ve düzeltmeler tamamlanınca son adayın tam yerel kalite kapısı bir kez çalıştırılır.
- Başarılı araç çıktısı kısa tutulur. Ham hata çıktısı kalıcılaştırılmaz; teşhis gerektiğinde sınırlı ve sırlardan arındırılmış bölüm gösterilir.
- İnternet araştırması ve kaynak okuması ilgili bölümle sınırlanır. Değişebilir teknik iddialar birincil güncel kaynak veya sabitlenen kaynak koduyla doğrulanır.
- Yalnız gerçekten ölçülen token/savings raporlanır. Ajanın bu projeyi geliştirirken ne kadar token tasarrufu sağladığı ölçülemiyorsa yüzde uydurulmaz.
- `models_cache.json` temel talimatları `model_instructions_file` içine kopyalanmaz. İlgisiz/global Codex model veya subagent ayarları değiştirilmez.

## Uygulama sınırları

- Önce gerçek motor/packaging/protokol kanıtı, sonra çalışan dikey ürün dilimleri. Mock veri gerçek entegrasyon veya başarı olarak sunulmaz.
- Keşif salt okunurdur. Kullanıcının mevcut istemci ayarları ve başka projelerdeki aktif oturumlar motor deneyinde değiştirilmez.
- Bağlantı değişikliği sahiplik, atomik yazım, fingerprint ve çatışmayı koruyan geri alma içerir. Mevcut auth sırları kalıcı log/state içine kopyalanmaz.
- Runtime yalnız uygulamaya ait süreç/loopback portunu yönetir. Başka proxy sahiplenilmez veya kapatılmaz.
- Model, reasoning, auth yöntemi ve ajan sayısı sessizce değiştirilmez. Aynı model kullanımı, aynı ajan/rota kanıtı değildir.
- Yerel ölçüm açık, anonim beacon kapalıdır. İçerik/secret yerine gereken en az metadata saklanır.
- Duraklatma aktif stream'i veya geçmiş CCR retrieval erişimini kesmez. Ölçülmemiş fail-open, model desteği veya Windows desteği iddia edilmez.
- Temel varsayılanlar: coding/cache profili, korumalı kaynak/sayısal içerik, CCR TTL 7200; otomatik model/reasoning değiştirme, memory/learn ve output shaper kapalı.
- Mevcut kullanıcı değişiklikleri korunur. Tüm ürün kaynakları ve proje tarafından indirilen geliştirme artefaktları bu proje veya açıkça belirlenen uygulama state kökü içinde kalır.
- İlgili skill talimatları uygulanır; rutin geri alınabilir iş için yeni onay akışı üretilmez. Eksik hesap/ikinci OS testi destek durumunda açıkça belirtilir; bağımsız ilerleme sürer.

## Tamamlama ve devir

- Ana görevler sırayla yürütülür; etkin ana görev kapanmadan sonraki ana göreve geçilmez. `TASKS.md` ilerledikçe güncellenir. Her ana görev tamamlandığında kullanıcıya açıkça bildirilir ve çalışma durur; sonraki ana görev için kullanıcının devam mesajı beklenir. Alt dilim kabulü ana görevin tamamlanması gibi sunulmaz.
- Görev ancak kabul koşulları somut kanıtla karşılandığında tamamlanır. Kısmi ve dış ortam doğrulaması bekleyen işler açık kalır.
- Uygulama davranışı, UI eylemi → yönetici → motor/istemci yolu üzerinden incelenir; yalnız buton veya mock testi yeterli sayılmaz.
- TASKS ve checkpoint'te tamamlanan/aktif iş, kanıt, bilinmeyenler ve bir sonraki adım kısa tutulur.
- Gerçek uyumluluk durumu, yerel geliştirme başarısı ve yayın adayı birbirinden açıkça ayrılır.
