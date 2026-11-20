# PennyPinch — görsel dil

5 Kasım2026 başladı,6 Kasım yerel kabul tamamlandı. Kullanıcı mevcut tasarımı reddetti; UX001 ile Graphite/mint ailesi yeniden kuruldu. Eski kutu düzeni ve hiyerarşi tasarım referansı değildir. [Görsel kabul](docs/proofs/ui-taste.md); H029 başlamadı.

## Ürün ve yön

Headroom günlük kullanılan yerel bir masaüstü aracıdır. Kullanıcı önce ne kadar girdi azaldığını, sonra motor durumunu ve uygulama bağlantılarını anlamalıdır. Yüzde ve miktar aynı gerçek dönemden gelir. Hesap/sohbet envanteri, canlı sağlayıcı veya model desteği bu görsel işle eklenmez.

Emil Kowalski'nin [design engineering](.agents/skills/emil-design-eng/SKILL.md) rehberi esas alınır; MIT lisansı ve sabit kaynak `UPSTREAM.json` içindedir. İncelenen diğer kaynaklar: [Taste](https://github.com/Leonxlnx/taste-skill), [Impeccable](https://github.com/pbakaus/impeccable), [Hallmark](https://github.com/Nutlope/hallmark). Dört ayrı tasarım sistemi üst üste kurulmaz. Yeni kurulum yalnız proje içindedir.

- Yön: sakin ve keskin bir çalışma aracı. Daha ince gezinme, geniş nefes alan ana tuval, tek güçlü tasarruf alanı, altında gerçek grafik ve uygulamalar. Pazarlama sayfası/hero animasyonu değil.
- Tipografi: Windows Segoe UI Variable/Segoe UI; native fallback korunur.14px gövde,12px yardımcı metin,36px ekran başlığı,72px ölçüm. Tabular rakamlar, az sayıda ağırlık, izli büyük harf etiketleri yok.
- Renk: koyuda sıcak grafit ve yumuşak mint; açıkta kırık beyaz ve koyu yeşil. Renkler semantik CSS token'larından gelir; durum yalnız renkle açıklanmaz. Yerel düşük kontrastlı border, primary/ölçümde vurgu, ciddi hatada görünür uyarı.
- Biçim: tek hero yüzeyi, ince ayırıcılar ve8–16px köşe yarıçapı. Eşit ağırlıklı kart yığını, neon parıltı, cam efekti ve dekoratif gradient yok. Veri yoğun tabloda satır ve boşluk hiyerarşisi.
- İkonlar: tek stroke dili, gerçek SVG; Unicode sembol karışımı yok. İşlevi belli olmayan buton yok. İkon tek başınaysa erişilebilir ad zorunlu.
- Hareket: sık kullanılan gezinme ve sayısal ölçüm anında değişir. Butonun pointer basışı kısa geri bildirim alır; klavye eylemi animasyon beklemez. Reduced-motion ve hover/pointer ayrımı korunur; yeni motion bağımlılığı yok.
- Boş durum: sahte grafik/yüzde/hesap/ajan sayısı yok. Veri bekleme ve okuma hatası farklıdır. Eksik ölçümde anlamlı metin ve mevcut bağlantı ekranına gerçek eylem; ölçülmüş0 kaybolmaz.
- Güven: unknown, scope/ölçüm sınırı, pending, çatışma ve kalıcı ayar uyarıları okunabilir kalır. Daha sade metin, daha güçlü gerçeklik iddiası yapmaz. Başlat/Pause/Resume yalnız gerçek motor ack'iyle değişir.

## UX001 kabulü

1. Native uygulamada Genel bakış, Uygulamalar, Etkinlik ve Ayarlar aynı görsel dili kullanır. Gerçek Start/Pause/Resume/Stop, gezinme, dialog ve tercih kontrolleri çalışır.
2. Gerçek boş/unknown/hata ile açıkça izole örnek veri,0/negatif/mixed/stale durumları ayrılır. TR/EN, açık/koyu,1280/1024 ve gerçek125/150 WebView ölçeğinde temel kontrol ve metinler taşmaz; focus/klavye ve reduced-motion gözlenir.
3. Son render gözle incelenir; önce/sonra karşılaştırması ve kaynak/ortam fingerprint'i kaydedilir. Son aday yerel kalite kapısı, TASKS/checkpoint ve commit/push ile kapanır. Motor/auth/ölçüm semantiği değişmez. Geliştirme token tasarrufu ölçülmedikçe oran verilmez.
