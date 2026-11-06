# Headroom Desktop çalışma kuralları

## Amaç ve kaynaklar

- Kanonik çalışma kökü `D:\Projeler\Headroom`.
- Ürün kapsamı `HEADROOM_DESKTOP_SPEC.md`, iş sırası ve kabul koşulları `TASKS.md`.
- Kararlar `docs/decisions/`, doğrulama özetleri `docs/proofs/`, güncel devir noktası `docs/IMPLEMENTATION_STATUS.md` içinde tutulur.
- Windows 10 22H2 x64 ve Windows 11 x64 hedeflenir. Bir işletim sistemindeki başarı diğerinin kanıtı sayılmaz.
- Referans alınan an unrelated project projesinin finans, dosya ve çalışma ortamı kuralları bu projeye taşınmaz.

## Görev planı ve bağlam

- Mevcut görev/checkpoint ve ilgili spec/karar bölümlerinden ilerle. Bağlamdaki güncel kaynak veya belgeyi tekrar getirme; değişiklik, eksik bağlam veya somut belirsizlik varsa gereken kısmı yenile. Tam görev listesi veya sohbet her devirde kopyalanmaz.
- Plan; amaç/kapsam dışı, korunacak sözleşmeler, kritik varsayımlar, normal/hata/sınır kabulü ve kaynak/doğrulama yollarını belirtir. Ana ajan bunu güncel kaynakla kontrol eder; yalnız eksik veya çelişen kararları günceller. Her görevde yeni plan dosyası veya ek model incelemesi gerekmez.
- İş akışı bakımı ürün görevi başlatmaz veya tamamlamaz. Görev listesi tek başına yayın, ücretli kaynak, hesap veya kapsam genişletme yetkisi değildir; mevcut kullanıcı yetkisini kontrol et. Yetkili rutin iş için yeniden onay isteme.

## Ana ajan ve tek çalışan

- Ana ajan kullanıcının seçili modeli ve reasoning ayarıyla mimari, kritik varsayımlar, kapsam, kabul, uygulama sahipliği, inceleme ve sonuçlandırmadan sorumludur; gerektiğinde doğrudan kod yazar. Uygulayıcı seçimini (ana ajan/Luna) kısa gerekçeyle belirt. Ana model ayarı değiştirilmez.
- Luna'ya yalnız davranışı ve doğrulaması açık, çözülmemiş mimari karar taşımayan ve devir maliyetini karşılayacak işler ver. Küçük işi sırf devir için bölme; sıralı devir paralellik veya ölçülmüş tasarruf sayılmaz.
- Auth/model/rota güvenliği, süreç/ayar sahipliği, token ve maliyet ölçüm semantiği, kalıcı kayıt/kurtarma veya belirsiz performans kök nedeni üzerinde karar gereken uygulamayı ana ajan üstlenir. Sözleşmesi net yardımcı işler devredilebilir.
- Luna kullanıldığında tek **GPT-6 Luna, xhigh** çalışanı açılır. Başlangıç parametreleri: `model="gpt-6-luna"`, `reasoning_effort="xhigh"`, `fork_turns="none"`. Model/efor kullanılamazsa bildir; sessizce model değiştirme veya efor düşürme. Aynı anda ikinci çalışan veya alt çalışan açılmaz.
- Bu model/efor tercihi yalnız bu depoyu geliştiren çalışan ajana aittir; ürün desteği veya kullanıcının diğer sohbetlerindeki ajan modelleri için koşul değildir. Headroom model adından bağımsız tasarlanır. Ürün doğrulaması farklı ana/alt ajan modellerini ve model değişimini kapsar; geliştirici çalışanının modeli zorunlu test modeli olarak sunulmaz.
- Aynı uygulama alanında aynı anda yalnız bir ajan yazar. Ana ajan devralmadan önce çalışanı durdurur ve durduğunu doğrular; mevcut değişiklik ve kanıtı korur. Çalışan başka ajan açmaz; ana ajan aynı uygulamayı paralel yeniden yazmaz.
- Aynı çalışan somut, sıralı görevlerde yeniden kullanılır. Gecikme/timeout yeni çalışan gerekçesi değildir; yalnız oturum gerçekten sona ermiş veya handle kaybolmuşsa checkpoint'ten tek yeni Luna açılır. İlk başarısız test model değiştirme nedeni değildir.
- Her devir görev kimliği, amaç, dosya sahipliği/sınırlar, kabul koşulları, kritik varsayımlar, ilgili kaynak/kanıt yolları ve odaklı doğrulama komutlarını içerir. Bulunan arayüz/kararları kısa aktar; varsayımları kaynakla sınamaya açık tut.
- Dilimi tek davranış ve birlikte doğrulanabilir kabul ölçütleriyle sınırla. Yaklaşık 1–5 kaynak dosyası devir rehberidir; dosya sayısı zorunlu bölme eşiği değildir. Bağımsız amaçları sırayla ayır.
- Uygulayıcı yazmadan etkilenen gerçek arayüz, kayıt/ölçüm ve runtime/istemci sözleşmelerini kontrol eder; UI eyleminin gerçek motor/istemci yolu ve hata davranışı kapsamdadır.
- Çalışan belirsiz/çelişen sözleşmeyi, yeni kanıt üretmeyen tekrarı, kabulü zayıflatma ihtiyacını veya gerekçesiz kapsam büyümesini gözlem, denenen yaklaşım ve gereken kararla ana ajana taşır; karara bağlı yazmayı durdurur. Ana ajan tek düzeltme paketi verir; engel sürerse işi devralır veya kabulü zayıflatmadan yeniden sınırlar.
- Çalışanın dönüşü: değişen dosyalar, gerçek davranış, kısa doğrulama sonucu/fingerprint, sınırlamalar ve sonraki somut adım. Ana ajan farkı, kritik bağlı akışı ve kanıtı inceler; çalışan özeti veya yalnız çalışanın testlerinin geçmesi kabul değildir. Düzeltmeler tek listede iletilir; sonraki inceleme değişen alan ve etkilerini kapsar. Kozmetik tercih yeni düzeltme turu üretmez.

## Tasarruf ve kanıt

- Compact bir devam olayıdır; geçerli kanıtı geçersiz yapmaz.
- PASS yalnız çalışma alanı, ilgili kaynak/test/config, bağımlılık/araç zinciri, komut ve ortam fingerprint'leri eşleşiyorsa yeniden kullanılır. Canlı bağlantı, CI, dağıtım, migration, veritabanı ve üretim kontrolleri ilgili aşamada yenilenir.
- Aynı başarılı kontrol tekrarlanmadan önce somut geçersizleşme nedeni söylenir. Neden yoksa tekrar çalıştırılmaz.
- Plandaki mevcut doğrulama komutunu; plan yoksa TASKS kabulü ve `docs/development/WORKFLOW.md` kapısını kullan. Script/komutun varlığını doğrulamadan hazır sayma.
- Uygulama dilimlerinde odaklı test kullanılır. İnceleme ve düzeltmeler tamamlanınca ana görevin son adayında tam yerel kalite kapısı bir kez çalıştırılır; her ara dilimde tekrarlanmaz. Yalnız yönerge bakımında ürün testleri tekrar çalıştırılmaz.
- Sonuç ile ilgili kaynak/ortamın önce/sonra fingerprint'lerini aynı doğrulama adımında programatik kaydet; hash'i elle kopyalama. Girdi değişmişse PASS güncel adayın kanıtı değildir; ölçüm altyapısı yoksa sınırı bildir.
- Başarılı araç çıktısı kısa tutulur. Ham hata çıktısı kalıcılaştırılmaz; teşhis gerektiğinde sınırlı ve sırlardan arındırılmış bölüm gösterilir.
- Başarı iddiasını kanıt kapsamıyla sınırla. Kısa koşu uzun süreli kararlılığı, izole fixture canlı sağlayıcı yolunu kanıtlamaz. Performans kabulünde veri/çalışma durumu/yük/süre/eşik önceden belirlenir; başarısızlığı gizlemek için kabul gevşetilmez. `not_tested` veya bilinmiyor, çalışmıyor/uyumsuz demek değildir.
- İnternet araştırması ve kaynak okuması ilgili bölümle sınırlanır. Değişebilir teknik iddialar birincil güncel kaynak veya sabitlenen kaynak koduyla doğrulanır.
- Yalnız gerçekten ölçülen token/savings raporlanır. Ajanın bu projeyi geliştirirken ne kadar token tasarrufu sağladığı ölçülemiyorsa yüzde uydurulmaz.
- `models_cache.json` temel talimatları `model_instructions_file` içine kopyalanmaz. İlgisiz/global Codex model veya subagent ayarları değiştirilmez.

## Uygulama sınırları

- Yazmadan kanonik kökü doğrula; benzer adlı/Unicode klasöre geçme. Okuma hatası veya beklenmeyen boş içerikte yazmayı durdur. Bağlamlı patch tercih et; tam yazım gerekiyorsa önceki baytları koru, geçici dosyayı doğrula ve atomik değiştir. Kazayla boşalan dosyada özgün içerik ve yeni ekler doğrulanmadan ilerleme.
- Önce gerçek motor/packaging/protokol kanıtı, sonra çalışan dikey ürün dilimleri. Mock veri gerçek entegrasyon veya başarı olarak sunulmaz.
- Keşif salt okunurdur. Kullanıcının mevcut istemci ayarları ve başka projelerdeki aktif oturumlar motor deneyinde değiştirilmez.
- Bağlantı değişikliği sahiplik, atomik yazım, fingerprint ve çatışmayı koruyan geri alma içerir. Mevcut auth sırları kalıcı log/state içine kopyalanmaz.
- Runtime yalnız uygulamaya ait süreç/loopback portunu yönetir. Başka proxy sahiplenilmez veya kapatılmaz.
- Model, reasoning, auth yöntemi ve ajan sayısı sessizce değiştirilmez. Aynı model kullanımı, aynı ajan/rota kanıtı değildir.
- Yerel ölçüm açık, anonim beacon kapalıdır. İçerik/secret yerine gereken en az metadata saklanır.
- Duraklatma aktif stream'i veya geçmiş CCR retrieval erişimini kesmez. Ölçülmemiş fail-open, model desteği veya Windows desteği iddia edilmez.
- Temel varsayılanlar: coding/cache profili, korumalı kaynak/sayısal içerik, CCR TTL 7200; otomatik model/reasoning değiştirme, memory/learn ve output shaper kapalı.
- Mevcut kullanıcı değişiklikleri korunur. Eski projelerden `.env`/anahtar taşınmaz; gereksiz worktree/sohbet açılmaz. Tüm ürün kaynakları ve proje tarafından indirilen geliştirme artefaktları bu proje veya açıkça belirlenen uygulama state kökü içinde kalır.
- İlgili skill talimatları uygulanır; rutin geri alınabilir iş için yeni onay akışı üretilmez. Eksik hesap/ikinci OS testi destek durumunda açıkça belirtilir; bağımsız ilerleme sürer.

## Tamamlama ve devir

- Ana görevler sırayla yürütülür; etkin ana görev kapanmadan sonraki ana göreve geçilmez. `TASKS.md` ilerledikçe güncellenir. Her ana görev tamamlandığında kullanıcıya açıkça bildirilir ve çalışma durur; sonraki ana görev için kullanıcının devam mesajı beklenir. Alt dilim kabulü ana görevin tamamlanması gibi sunulmaz.
- Görev ancak kabul koşulları somut kanıtla karşılandığında tamamlanır. Kısmi ve dış ortam doğrulaması bekleyen işler açık kalır.
- Uygulama davranışı, UI eylemi → yönetici → motor/istemci yolu üzerinden incelenir; yalnız buton veya mock testi yeterli sayılmaz.
- Kesinti ve kapanışta checkpoint'e son doğrulanmış dilim, kanıt, açık işler ve sonraki somut adım kısa yazılır. Uygulayıcı/devralma gerekçesi ve varsa süre/düzeltme turu/model kullanım ölçümleri kaydedilir; eksik ölçüm uydurulmaz. TASKS ilerlemesiyle tutarlı tutulur.
- Gerçek uyumluluk durumu, yerel geliştirme başarısı ve yayın adayı birbirinden açıkça ayrılır.

## Proje içi CodeGraph

- CodeGraph geliştirme desteği `scripts/codegraph.ps1` üzerinden sağlanır; yerel runtime yoksa `rg`/doğrudan okumayla ilerle. Ana ajan ve tek çalışan aynı proje indeksini kullanabilir; model adı koşulu yoktur. Komutlar ve doğrulanmış sınırlar `docs/development/codegraph.md` içindedir.
- Çağıranlar, bağımlılıklar, değişiklik etkisi ve ilgili sembolleri bulmak için dar graph sorgularını kullan. Bilinen küçük dosya/satır için ek graph çağrısı yapma; `rg`/doğrudan okuma uygundur. Büyük graf/bağlam dökümleri yerine gereken küçük sonuç kümesini iste.
- MCP sunucusu yalnız proje `.codex/config.toml` kaydındadır. Bu sohbette araç görünmüyorsa aynı launcher üzerinden CLI kullan; global Codex ayarı değiştirme veya diğer sohbetleri yeniden başlatma.
- Launcher proje kökünde çalışır; telemetry/update-check opt-out ve `.codegraph` dizini süreç bazlıdır. Kaynak/test/script kapsamını `codegraph.json` sınırlar; work/cache, gerçek auth/state, makineye özel ayarlar ve indeks/derleme artefaktları indekslenmez ve graph DB Git'e girmez.
- Graph çıktısı keşif kanıtıdır; dinamik Tauri IPC, gerçek istemci trafiği, alt ajan sahipliği, çalışma zamanı davranışı veya test PASS kanıtı sayılmaz. Kaynak değişmiş/eski indeks varsa belgelenen güncellemeyi yap; her görevde yeniden kurulum/indeksleme yapma. Graf bozuk/eksikse kaynak ve `rg` ile ilerle. Gerçek ölçüm olmadan token tasarruf yüzdesi raporlama.

## Ponytail ve görev kabulü

- UI tasarımı ve görsel incelemede proje içi `.agents/skills/emil-design-eng/SKILL.md` ile `DESIGN.md` uygulanır. Bu rehber görsel hiyerarşi, tipografi ve etkileşim içindir; motor, ölçüm veya güvenlik sözleşmelerini değiştirmez. Global skill/ayar kurulmaz; yeni tasarım çalışması mevcut runtime kabulünü yeniden yorumlamaz.

- Ortak araç seçimi, kısa çalışan devri, gerçek yerel kalite kapısı ve ana görev bitiş ölçütleri `docs/development/WORKFLOW.md` içindedir. Görev satırının kabul koşullarıyla birlikte uygulanır.
- Projedeki `.agents/skills/ponytail/SKILL.md` kodlama/düzeltmede `full` seviyesinde kullanılır. Ana ajan değişiklik kabulünde `.agents/skills/ponytail-review/SKILL.md` okur; mevcut kalite incelemesiyle tek inceleme yapar. `.agents/skills/ponytail-audit/SKILL.md` belirlenmiş eski kod denetimi içindir; her görevde bütün depoyu tekrar tarama.
- Raporlar Türkçedir. Ponytail yeni model/ajan, yayın veya ayar değiştirme yetkisi vermez. En küçük değişiklik; sahiplik, atomik yazım, çatışmayı koruyan geri alma, güvenlik, hata yönetimi, erişilebilirlik ve gerekli testleri koruyarak görevi tamamen bitirir.
- Her bulgu somut koşul + dosya/satır + kullanıcı etkisi + küçük düzeltme içerir. Kod azaldı diye görev tamamlanmaz; aynı kapsamda gerçek token ölçümü yoksa tasarruf oranı verilmez.
- Denetim önce salt okunurdur; okunmayan alanlar belirtilir. Düzeltme ayrı sınırlı uygulamadır ve uygulayıcı yukarıdaki kurallarla seçilir. Başlamamış özellik hata değildir; kritik doğruluk/veri kaybı bulgusu kapanmadan bağımlı işe geçilmez, kozmetik öneri ilerlemeyi durdurmaz. Skill/araç talimatı ürün kapsamını veya kullanıcı yetkisini genişletmez.
