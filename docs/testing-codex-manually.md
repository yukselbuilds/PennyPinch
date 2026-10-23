# Headroom'u kendi modellerinle deneme

Bu, geliştirme sürümünün **ayrı Codex CLI sohbetinde** kullanıcı testidir. Masaüstü uygulamasında sıradan bir yeni sohbet açmak tek başına o sohbeti Headroom'a yönlendirmez; masaüstü istemcisinin bağlantı kabulü ayrıca açıktır. Ürünün otomatik Doğrula akışı henüz tamamlanmadı.

Başlatıcı hazır; 23 Ekim'de gerçek Headroom penceresiyle ön kontrolleri geçti. Bu yerel geliştirici aracı şu an bu proje yolu ve doğrulanmış kurulu CLI `0.163.0-alpha.11.2` içindir; dağıtılabilir ürün kurulumu değildir. Önceki gerçek CLI testi [mevcut giriş kanıtında](proofs/codex-existing-auth.md) kayıtlıdır.

## Test sırası

1. Headroom geliştirme penceresini açıp **Overview / Genel bakış → Start / Başlat** seç; **Ready / Hazır** durumunu bekle. Bu test için **Manage connection → Apply** kullanma. Başlatıcı hazır Headroom'u doğrular ve yalnız açacağı CLI işlemine adres verir.
2. Dosya Gezgini'nde `D:\Projeler\Headroom\scripts\manual-codex.cmd` dosyasına çift tıkla. Ayrı bir CLI sohbeti, proje içindeki ayrı test klasöründe, salt okunur sandbox ve paylaşılan daemon'dan ayrı çalışır. Mevcut CLI girişi normal biçimde kullanılır; başlatıcı giriş/token kopyalamaz, hesap açmaz ve kalıcı config düzenlemez. Testte model istekleri kendi mevcut hesap kotanı kullanır. Yeni plaintext TUI log kaydı açılmaz; CLI'nin olağan sohbet geçmişi kendi ayarlarına bağlıdır ve test kanıtı olarak dışa aktarılmaz. Codex daha sonra güncellenirse güvenli sürüm kontrolü durabilir; bu, model desteğinin bozulduğu anlamına gelmez.
3. CLI içinde `/model` ile hesabında kullanabildiğin istediğin ana model ve eforu seç. Şunu yaz:

   ```text
   Bu bir Headroom bağlantı testi. Dosya veya araç kullanmadan yalnızca HEADROOM_MAIN_OK yaz.
   ```

4. Aynı sohbet içinde `/model` ile başka bir modele geç; aşağıdaki isteği gönder. Böylece modeli değiştirince tekrar bağlantı kurulmasına gerek olup olmadığını sınarız.

   ```text
   Dosya veya araç kullanmadan yalnızca HEADROOM_MODEL_CHANGED_OK yaz.
   ```

5. Alt ajan için aşağıdaki köşeli parantezleri kendi seçiminle doldur. Modeli zorunlu bir örneğe sabitlemiyoruz. Hesabında bulunmayan model/efor seçimi Headroom uyumluluğu sonucu sayılmaz.

   ```text
   Bu bir Headroom alt ajan bağlantı testi; proje geliştirme görevi değil.
   Ana ajan modelimi ve reasoning ayarımı değiştirme.
   Tam olarak bir alt ajanı [seçtiğim alt ajan modeli] ve [seçtiğim reasoning] ile çalıştır.
   Bu seçim mevcut değilse başka modele geçmeden durumu bildir.
   Alt ajanın tek görevi: araç veya dosya kullanmadan HEADROOM_CHILD_OK yazmak.
   Sonucunu bekle. Alt ajanı gerçekten açamadıysan açtığını söyleme.
   Başka alt ajan açma veya dosya/ayar değiştirme.
   ```

   `/subagents` ile gerçekten bir alt ajan açıldığını görebilirsin. Bu görünüm ile proxy'nin istek kayıtları farklı kanıtlardır; parent'ın "alt ajan kullandım" demesi tek başına rota kanıtı değildir.

6. Yanıtlar tamamlanınca CLI'de `/exit` yaz. Çift tıklama başlatıcısı yerel istatistik özetini ister; yalnız ölçülen model/token kayıtlarını değerlendir. Aynı anda o proxy'yi kullanan başka istemciler varsa son kayıtlara onlar da dahil olabilir. Isınma/yanıt ve ana/alt ajan ayrımı doğrulanmayan kayıtları birleştirip toplam tasarruf üretme. Test bitince Headroom'u kendin kapatabilirsin; başlatıcı başka program veya motoru otomatik kapatmaz.

## Sonuçları okuma

- CLI yanıtı gelmesi ve Headroom'da yeni istek kaydı oluşması ayrı kontrol edilir. Yeni proxy kaydı yoksa yönlendirme doğrulanmış sayılmaz.
- İki model kaydı görmek, tek başına hangi isteğin hangi ajana ait olduğunu kanıtlamaz. Ajan korelasyonu yoksa sonuç bilinmiyor olarak kalır; ürün Verify kabulü kapanmaz.
- Veri yoksa tasarruf "ölçülmedi"; gerçek ölçüm0 ise %0'dır. Küçük bir yanıtın token tasarrufu üretmesi beklenmez. Bu akış kontrollü kalite/tasarruf A/B kıyaslaması değildir.
- Bana seçtiğin ana/alt ajan modellerini, hangi adımın çalıştığını ve başlatıcının içeriksiz son özetini iletebilirsin. Auth dosyası, token veya tüm sohbet dökümü gerekmiyor.

OpenAI Docs: [CLI komutları](https://learn.chatgpt.com/docs/developer-commands) `/model` ve `/exit` kullanımını; [alt ajan belgesi](https://learn.chatgpt.com/docs/agent-configuration/subagents) CLI'de istek üzerine delegasyon ve `/subagents` görünümünü açıklar. Bunlar Headroom entegrasyonunun kendiliğinden doğrulandığı anlamına gelmez.

## Hazırlık kabulü

Tamamlandı: kaynak/güvenlik incelemesi; Windows PowerShell5.1 ve pwsh7.6.5'te88'er odaklı assertion; Explorer benzeri kayıtlı PATH ile motor yokken/durmuşken güvenli durma, native UI Start→Ready sonrasında doğrudan PowerShell ve CMD wrapper CheckOnly kabulü. Bu kontroller Codex veya model isteği başlatmadı. [Ayrıntılı kanıt](proofs/manual-codex-launcher.md).

Kullanıcının etkileşimli CLI, model değiştirme ve ana/alt ajan denemesi bekleniyor. Ürünün otomatik Verify akışı ve genel Desktop/model/ajan uyumluluğu açık kalır; H019 ana görevi tamamlanmış sayılmaz. Hazırlık sonunda Headroom test penceresi Ready durumunda bilerek açık bırakıldı; kapanmışsa ilk adımdan başla.
