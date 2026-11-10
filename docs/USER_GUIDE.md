# Headroom Desktop — kısa kullanım kılavuzu

Bu, yerel geliştirme sürümünün kılavuzudur; yayımlanmış veya yeni bilgisayara hazır bir sürüm değildir.

## İlk bağlantı

Headroom’u açmak istemciyi bağlamaz. Genel bakış’taki Başlat yalnızca Headroom motorunu çalıştırır; istemci ayarlarını değiştirmez. Uygulamalar ekranında kurulumları tara. Tarama bilinen yerel konumlarla sınırlıdır; bir kurulumun bulunmaması istemcinin desteklenmediği anlamına gelmez. Dosyadan ekle yalnızca kurulum keşfeder. Kurulum tercihi ancak açıkça kaydedildiğinde veya Bağlan seçildiğinde hatırlanır.

Bağlan’ı seçip doğru istemci yüzeyini/kurulumunu doğrula. Bu açık işlem seçimi kaydeder, gerekirse yalnız Headroom’a ait motoru başlatır, güncel yapılandırmayı denetler ve uygunsa bağlantı ayarını yazar. Plan engellenirse gösterilen nedeni düzeltmeden devam etme. Headroom için ayrı bir hesap oluşturulmaz; sağlayıcı hesabına istemcinde giriş yaparsın. Model, reasoning ve kimlik doğrulama yöntemi istemcinin ayarlarında kalır.

Bağlantı ayarının kaydedilmesi açık oturumun bunu kullandığını kanıtlamaz. Yeni bir CLI süreci başlat; masaüstü istemcisinde uygulamayı tamamen kapatıp yeniden aç. Açık sohbet, eski doğrudan rotayı kullanmaya devam edebilir. Trafik doğrulanmadıysa bağlantı kaydını gerçek istek kanıtı sayma.

## Kapsam ve ölçümler

Yalnız Headroom yerel endpoint’ine giden istekler motordan geçer. Aynı endpoint’i kullanan ana ve alt ajan istekleri bu rotayı izleyebilir; ayrı endpoint veya sağlayıcı kullanan trafik bu bağlantının kapsamında değildir. Kayıtlı ayar tüm modelleri, araçları veya oturumları doğrulamaz.

Genel bakış’taki önce/sonra girdileri Headroom’un yerel kayıtlarından hesaplanan input-token ölçümüdür; sohbet geçmişi değildir. Bu tahmin sağlayıcının token kullanımı, faturası veya maliyet hesabı değildir. Yüzde ve miktar yalnız karşılaştırılabilir ölçümlerde gösterilir; veri yoksa ya da temeller farklıysa toplam uydurulmaz.

Ölçüm uygunsa yüzde `(önce − sonra) / önce × 100` ile hesaplanır. Sonra değeri daha büyükse girdi artışı gösterilir; önce değeri sıfırsa yüzde hesaplanmaz. Oturum, Bugün ve 7 gün dönemlerini Genel bakış’tan seçebilirsin. Etkinlik ekranında istemci ve model adıyla filtreleyip kayıtları inceleyebilirsin; bilinmeyen model veya ajan bilgisi tahminle doldurulmaz.

## Duraklatma, durdurma ve tepsi

Duraklat yeni isteklerin sıkıştırılmasını durdurur; sürmekte olan yanıtlar ve önceki içerik erişimi devam eder. Devam et yeni istekler için sıkıştırmayı açar. Motoru durdurmak bağlantı ayarını geri almaz; bağlı istemciler bağlantı hatası görebilir. Geri almak için Bağlantıyı yönet bölümünden yalnız Headroom’un eklediği yönlendirme kaydını kaldır. Diğer istemci ayarları ve motor durumu korunur; açık istemcinin ayarı alması için yeniden açılması gerekebilir.

Tepsi tercihi açıksa pencerenin X’i Headroom’u tepsiye gizler ve motor çalışmaya devam eder. Headroom’dan çık seçeneği motoru ve tepsi simgesini kapatır; yönlendirme kayıtlarını otomatik kaldırmaz, bağlı istemciler hata alabilir ve süren yanıtlar kesilebilir. Windows başlangıcı açıksa oturum açınca pencere açılır; motor kendiliğinden başlamaz.

## Tercihler ve güncellemeler

**Yerel öneriler** Ayarlar’dan açılır. Genel bakış, yeterli kayıt olduğunda girdi artışını, yüksek yerel sıkıştırma süresini, kaydedilen cache isabetindeki düşüşü veya yeniden taranması gereken kurulumu gösterir. Her öneride ölçüm kapsamı ve gözlem zamanı görünür. Önerileri aynı ayardan kapatabilirsin.

Ölçüm saklama süresi kısaltıldığında eski kayıtlar kalıcı olarak silinebilir; süreyi uzatmak silinen geçmişi geri getirmez. Gelişmiş korunan araç listesi yalnız sonraki elle motor başlatmasında uygulanır, otomatik yeniden başlatma yapmaz.

Güncellemeler ekranı yalnız bu derlemede güvenilir kaynak yapılandırılmışsa imzalı güncelleme gösterebilir. Kaynak kullanılamıyor mesajı varsa bu derleme için hazır bir üretim güncellemesi yoktur. Windows 10’da yerel güncelleme ve geri dönüş kabulü geçti; üretim yayın kaynağı/anahtarı ve Windows 11 doğrulanmadı.

## Doğrulama sınırları

Yerel kanıt Windows 10 22H2 içindir. Yerel optimizasyon testleri geçti; bu, sağlayıcı token/fatura tasarrufunu doğrulamaz. Windows 11 test edilmedi; macOS ve Linux için yerel uygulama henüz taşınmadı. Codex CLI ve Desktop için ChatGPT oturumu belirli Windows 10 akışlarında doğrulandı; Codex’in gerçek API-key sağlayıcı yolu henüz doğrulanmadı. Claude Code ve OpenCode bağlantıları sentetik kimlik bilgileri ve yerel test sağlayıcısıyla denendi; gerçek hesap/sağlayıcı doğrulaması değildir. Modellerin veya alt ajanların tamamı için garanti vermez. Ayrıntılar için [uyumluluk kayıtlarına](../compatibility/clients.json) bak.
