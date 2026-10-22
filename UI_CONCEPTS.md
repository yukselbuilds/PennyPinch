# Headroom Desktop — Üç UI Konsepti

**Tarih:** 19 Ekim 2026  
**Kapsam:** Görsel yön ve etkileşim önizlemesi. Bütün sayaçlar örnek veridir; istemci ayarı veya gerçek model trafiği değiştirilmez.  
**İlgili görev:** [H003 / H004](TASKS.md). Üretim tasarımı henüz seçilmedi.

[Üç konsepti birlikte aç](outputs/ui-concepts/index.html)

## 01 — Graphite

**Önerilen başlangıç yönü.** Koyu grafit yüzeyler, yumuşak mint vurgu, metinli sol menü. Girdi azalması geniş bir alanda; grafik ve uygulamalar dengeli iki sütunda. Gün boyu açık kalan geliştirici aracı için sakin ve okunaklı.

- Düzen: kalıcı sol gezinme; geniş özet, grafik ve bağlantı sütunu.
- Karakter: sade, yerel masaüstü hissi; düşük görsel gürültü.
- Gücü: ana eylem ve durum kolay bulunur; dört ekran için genişlemeye uygundur.
- Dikkat: üst özetin baskınlığı küçük pencerede dengelenmeli.

[Etkileşimli Graphite](outputs/ui-concepts/graphite.html) · [Ekran görüntüsü](outputs/ui-concepts/graphite.png)

## 02 — Ivory

Sıcak açık zemin, orman yeşili, büyük serif rakamlar. Üst gezinme ekranı yatay açar; kazanç ve grafik bir sütunda, bağlı uygulamalar ayrı sakin bir yüzeyde kalır.

- Düzen: yatay üst menü; asimetrik içerik ve uygulama alanı.
- Karakter: ferah, editoryal, daha az teknik yoğunluk.
- Gücü: rakamlar ve temel eylemler öne çıkar; açık tema güçlüdür.
- Dikkat: ileri düzey filtreler büyüdüğünde üst gezinme kalabalıklaşmamalı; koyu eşinin tasarımı ayrıca yapılmalı.

[Etkileşimli Ivory](outputs/ui-concepts/ivory.html) · [Ekran görüntüsü](outputs/ui-concepts/ivory.png)

## 03 — Signal

Gece mavisi yüzey, buz mavisi vurgu ve dar ikon menüsü. Özet ile grafik solda, uygulama bağlantıları kalıcı sağ sütunda. Daha kompakt bir izleme aracı hissi verir.

- Düzen: dar gezinme şeridi; çalışma alanı ve bağlantı rail'i.
- Karakter: teknik, kontrollü, biraz daha yoğun.
- Gücü: bağlantı durumu sürekli göz önünde; geniş pencerede alanı iyi kullanır.
- Dikkat: ikonların anlamı erişilebilir adlarla ve son üründe tooltip ile desteklenmeli; dar pencerede yerleşim sadeleşir.

[Etkileşimli Signal](outputs/ui-concepts/signal.html) · [Ekran görüntüsü](outputs/ui-concepts/signal.png)

## Karşılaştırma

| Karar | Graphite | Ivory | Signal |
| --- | --- | --- | --- |
| Önizleme teması | Koyu / mint | Açık / yeşil | Koyu / mavi |
| Gezinme | Etiketli sol menü | Yatay üst menü | Dar ikon menüsü |
| Bilgi yoğunluğu | Dengeli | Düşük | Orta |
| Kullanım odağı | Günlük kontrol | Sadelik ve kazanç | Trafik ve bağlantılar |
| Seçim önerisi | İlk aday | Açık/ferah yaklaşım isteyenler | Teknik izleme isteyenler |

Bu üçü yalnız renk varyasyonu değildir. Yerleşim, rakam hiyerarşisi ve gezinme değişir. Seçilen **tek** yön, son üründe hem açık hem koyu temayla geliştirilecek; üç ayrı uygulama sürdürülmeyecek. İstenirse Graphite düzeni ile Ivory'nin açık paleti birleştirilebilir.

## Önizlemede denenebilenler

1. Üç konsept arasında üst çubuktan geçiş.
2. Genel bakış, Uygulamalar, Etkinlik ve Ayarlar gezinmesi.
3. **Örnek oturum / İlk açılış / Bağlantı sorunu** durumları.
4. Başlat/duraklat ve örnek hata toparlama.
5. Bir defalık bağlantı diyaloğu, bekleyen doğrulama ve bağlantıyı kaldırma.
6. Bugün / Son 7 gün filtresi; ana ajan / Luna örnek etkinlik filtresi.
7. İstek ayrıntısı, yerel tercihler ve örnek CSV indirme.

Önizleme saf HTML/CSS/JavaScript'tir; kurulum gerektirmez. `index.html` veya konsept dosyaları tarayıcıda doğrudan açılabilir. Çalışan yerel önizleme sunucusu varsa aynı dosyalar HTTP üzerinden de kullanılabilir. Üretim teknolojisi, gerçek entegrasyon testleri ve Windows installer bu tasarım çıktısının kapsamında değildir.

## Yapılan önizleme doğrulaması

- Üç konsept × dört ekran × beş genişlik: 1440, 1024, 768, 390 ve 320 px; toplam 60 ekran/boyut kontrolünde sayfa düzeyinde yatay taşma yok.
- Başlat/duraklat, dönem filtresi, bağlantı ekleme/kaldırma, etkinlik filtresi/ayrıntısı, klavye ile switch, örnek CSV indirme ve hata toparlama akışları geçti.
- İlk açılışta bağlantı eklemek veri üretmiyor; istek olmadan grafik ve kazanç değeri oluşturulmuyor.
- Tarayıcı JavaScript hatası ve önizleme dışına giden ağ isteği görülmedi. Ekran görüntüleri görsel olarak incelendi.
- Görev listesinde 39 tekil kimlik, döngüsüz bağımlılıklar ve geçerli yerel belge bağlantıları doğrulandı.

Bu kontroller yerel Edge tarayıcısındaki tasarım önizlemesine aittir; Windows 10/11 native uygulama, motor veya gerçek istemci uyumluluk testlerinin yerine geçmez.

## Karar kaydı

- [x] Kullanıcı **Graphite — koyu grafit ve mint** yönünü seçti.
- [x] Aynı Graphite düzeninde koyu/açık semantik tema ve dengeli yoğunluk yaklaşımı netleştirildi.
- [x] Seçim spesifikasyona ve [H004 karar dosyasına](docs/decisions/001-ui-direction.md) işlendi.

Seçilen üretim yönü **Graphite**. Ivory ve Signal karşılaştırma önizlemeleri olarak korunur; demo rakamları üretim uygulamasına taşınmaz.
