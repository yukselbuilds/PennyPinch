# 001 — Seçilen görsel yön: Graphite

**Tarih:** 19 Ekim 2026  
**Durum:** Kullanıcı tarafından seçildi.

Kullanıcı asenkron konsept sorusuna **Graphite — koyu grafit ve mint** yanıtını verdi. Üretim arayüzü Graphite düzenini esas alır; açık/koyu renk temaları aynı gezinme ve bileşen yapısını paylaşır.

## Uygulama yaklaşımı

- Dört ekran: Genel bakış, Uygulamalar, Etkinlik, Ayarlar. Solda etiketli gezinme; motor, bağlantı ve sıkıştırma durumları ayrı.
- Koyu ve açık renkler semantik CSS token'larıyla uygulanır; sistem teması varsayılandır. Mevcut konsept HTML'i üretim entegrasyonu sayılmaz.
- Segoe UI Variable/Segoe UI sistem fontu; metriklerde tabular rakamlar. Renkler tek durum göstergesi değildir; görünür klavye odağı korunur.
- Ana zemin/panel/metin/vurgu: koyu `#111315` / `#1A1D21` / `#F0F2F4` / `#47C7A5`; açık `#F7F8FA` / `#FFFFFF` / `#1C2027` / `#147D68`. Alt metin ve etkileşim durumlarının kontrastı üretim uygulamasında ayrıca doğrulanır.
- Gerçek ölçüm yoksa boş durum gösterilir; konseptteki demo sayıları taşınmaz.
- Windows 10'da temel arayüz için Windows 11'e özel efekt gerekmez. Başlat/duraklat eylemi gerçek yönetici sonucuna bağlı olur.

Ivory ve Signal, karşılaştırılabilir tasarım alternatifleri olarak `outputs/ui-concepts/` içinde korunur. Üç farklı gezinme düzenini tek uygulamada tema adı altında desteklemek ilk sürüm kapsamını gereksiz büyütür; renk teması ile düzen konsepti ayrı kararlardır.

Kaynaklar: `HEADROOM_DESKTOP_SPEC.md` bölüm 8, `UI_CONCEPTS.md`, `outputs/ui-concepts/graphite.html`.

## 5 Kasım2026 — kullanıcının istediği tasarım yenilemesi

Kullanıcı eski görünümü reddetti. Graphite/mint ailesi korunarak hiyerarşi, boşluk, tipografi, ikonlar ve dört ekranın sunumu yeniden kurulur; yukarıdaki eski palette/layout değerleri tarihsel kalır. Güncel görsel sözleşme [DESIGN.md](../../DESIGN.md) ve proje içi Emil design engineering rehberidir. UX001 mevcut işlevi görsel olarak yeniler; yeni hesap/sohbet özelliği veya H029 kabulü üretmez.
