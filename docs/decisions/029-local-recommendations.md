# H045 — Yerel öneriler

10 Kasım 2026. H044 uygun RTK adayı bekliyor. H045, H039, H024, H026, H034 ve H041’in yerel kanıtına dayanır. Ana ajan sinyal, zaman, kayıt ve kabul sözleşmesini üstlenir. Tek Luna xhigh çalışanının kapsamı sabit Genel bakış sunumudur.

Özellik Ayarlar’dan etkinleşir ve varsayılan olarak kapalıdır. Tercih mevcut revision ve fingerprint korumalı kayda eklenir. Eski belgede alan yoksa kapalıdır. Eski kayıt isteği alanı göndermediğinde mevcut değer korunur. Genel bakış etkin ve tercih açıksa mevcut yerel ölçüm sorgusu önerileri de hesaplar. Kaynak satırlar veya ek istek içeriği arayüze aktarılmaz. Öneri bir sonraki adımı gösterir. Ayar, model, rota ve motor değişikliği kullanıcının mevcut akışlarına aittir.

## Sinyal sözleşmesi

| Sinyal | Kabul edilen kanıt | Gösterilen kapsam |
| --- | --- | --- |
| Girdi artışı | Seçili dönemde aynı ölçüm temeli ve girdi bileşenine sahip `measured` toplam. Önceki değer pozitif, sonraki daha büyük, olay zamanları yerleştirilebilir. | Kaydedilen karşılaştırılabilir olay sayısı, önce/sonra token, sayaç temeli, dönem ve ölçüm zamanı. |
| Yerel sıkıştırma süresi | Kayıt yapan motor neslinde son 15 dakikada en az 20 ayrı isteğin pozitif `optimization_latency_ms` kaydı. Çakışan ve hata bildiren kayıtlar hariç. En yakın sıra p95 en az 1.000 ms. | Motorun kaydettiği sıkıştırma süresi, örnek adedi, pencere ve zaman. |
| Kaydedilen cache isabetinde düşüş | Aynı motor nesli, model, sağlayıcı ve kanıt varsa aynı doğrulanmış rota. Son 15 dakika ve önceki 15 dakikanın her birinde en az 20 ayrı `cache_hit` bildirimi. Fark en az 20 yüzde puanı. Son dönemde en yakın uygun model/sağlayıcı grubu değerlendirilir. | İki penceredeki isabet ve kayıt sayıları, model/sağlayıcı, ölçüm zamanı. 0.41.0 işareti sağlayıcı ile yerel response cache isabetini birleştirir. |
| Eski kurulum kontrolü | H034 okuması mevcut keşif kaydının dosya eşleşmesini artık doğrulayamıyor ve `stale` döndürüyor. | İstemci adı, yerel kontrol zamanı, Uygulamalar’da yeniden tarama eylemi. |

Boş, belirsiz, karışık, eksik zamanlı ve eski snapshot’lardan ilgili sinyal üretilmez. Sayısal sinyaller kaydedilen örneklerle sınırlıdır. Ölçüm sayısı toplam trafik veya fatura kullanımına dönüştürülmez. Ekranda bir başlık altında sabit anahtarlarla satırlar güncellenir. Bildirim, arka planda yeni toplama işi ve otomatik onarım kullanılmaz.

## Kabul

1. İzole tercihte özellik kapalı başlar. Kullanıcı açtığında ölçüm yeterliyse ilgili satır görünür. Kapatınca satır kaldırılır ve yeniden açılış tercihi korur. Eski kayıt, geçersiz değer veya yazma çatışması önceki tercihleri korur.
2. Negatif karşılaştırılabilir girdi, eşikli gecikme ve cache örnekleri doğru kapsamla görünür. Boş, kısmi, çakışan, eski ve başka motor nesline ait kayıtlar ilgili uyarıyı üretmez. Yeni sağlıklı örnekler ve yeniden tarama geçersiz kalan öneriyi kaldırır.
3. TR/EN, klavye ve dar görünüm kontrolü. Etkinlik ve Uygulamalar eylemleri yalnız gezinir. Yerel test motoru/ölçüm deposu ile native yol ve son kalite kapısı doğrulanır. Windows 11 ve canlı sağlayıcı kanıtı mevcut planın dış kabullerinde kalır.

10 Kasım’da yerel kabul tamamlandı. Kapalı değer önceki tercih biçimini korur. Açık değer yeni bir tercih alanı içerir. Eski ikiliyle ileriye dönük tercih uyumluluğu üretim geri dönüş kabulünde ayrıca değerlendirilir. [H045 kanıtı](../proofs/recommendations.md)
