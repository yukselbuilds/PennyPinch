# H036 — uyum kataloğu ve güncelleme davranışı

8 Kasım 2026. Kullanıcı H035'in manuel tepsi kontrolünü beklerken açıkça sonraki göreve geçilmesini istedi. Bu, yalnız H036 için sıra istisnasıdır; H035 açık, ürün toplamı33/51 kalır. H036 tamamlanınca durulur. Ana ajan kanıt/auth/sürüm güveni, native ve kabul sahibi; mevcut tek Luna önce salt okunur kanıt eşleştirmesi yapar, sözleşmesi kesinleşen frontend dilimi ayrıca devredilebilir. Ponytail full ve mevcut Emil/Graphite kullanılır.

## Amaç ve kapsam

Uygulamalar'da isteğe bağlı, kapalı başlayan “Uyumluluk” ayrıntısı. Kayıtlı doğrulamalar sürüm/yüzey/auth/OS ve kanıt kapsamıyla gösterilir. Canlı doğrulanmış kapsam, yerel fixture/deneysel, desteklenmeyen ve denenmemiş ayrı kalır. Metadata eşleşmesi mevcut sohbet/auth/ajan doğrulaması sayılmaz. Modeller için yeni allowlist veya otomatik model/efor değişimi yoktur.

Kaynak `compatibility/clients.json` uygulamaya gömülür. Tarihsel kayıtlar korunur; eski pause/native-wiring ve README durumu güncel kanıtla düzeltilir. Runtime pini gerçek `runtime/manifest.json`, native sürüm sabitleri ve adaptör kodu fingerprint'leriyle çapraz doğrulanır. Kanal “development”; dağıtılmış stable/preview sürümü varmış gibi gösterilmez.

İstemci keşfi mevcut salt okunur `recovery_scan`, config gözlemi mevcut sahiplik/kapsam koruyan `recovery_config` üzerinden yapılır. Okunan config içeriği dönmez, kalıcılaştırılmaz; yalnız bellekte karşılaştırılan opaque bağlam fingerprint'i kullanılır. Yeni tarama sırasında/sonrasında eski sonuç güncel gösterilmez. Kurulum metadata'sı, sürüm veya sahip olunan config/layer/environment değişince yeniden doğrulama bildirimi; bilinmeyen kapsamda güven düşürülür. Config/model/auth yazımı, istemci çalıştırma veya hesap girişine ihtiyaç yoktur.

Bu görev motor kurma/güncelleme düğmesi eklemez. Etkin motoru/catalog okuması otomatik değiştirmez; mevcut sürüm/hash guard'ları korunur. Sürüm geçişi güvenliği, uyumsuz pinin reddi ve önceki tam pinin geri konduğunda yeniden geçerli olmasıyla doğrulanır. Gerçek installer/manifest imzası, indirme ve kurulu sürümler arasında uygulanabilir rollback H042'nin ayrı kabulüdür; bu görevde hazır gösterilmez.

## Sıralı dilimler ve kabul

1. Native kanonik katalog + salt okunur snapshot: kayıtlar ve kanıt yolları doğrulanır; tam sürüm/yüzey/OS eşleşmesi yalnız ilgili tarihsel kanıta bağlanır. Fixture canlı auth başarısına yükselmez. Bozuk katalog/istek/race, public hata veya unknown olur; ayar/motor değiştirmez.
2. Strict istemci ve sakin, açılır UI: TR/EN, tema, klavye, pending/error/unavailable/stale; bilinmeyen veya değişmiş kurulum/bağlamın yeniden doğrulama ihtiyacı görünür. Yenile aynı anda tek istek; eski cevabı yeni bağlamın sonucu yapmaz. Ana ekrana ek teknik kart yığını gelmez.
3. Kabul: sürüm/metadata/config fingerprint değişimi fixture'ları; kayıtlı her doğrulanmış satırın kanıtı ve runtime/adaptör/channel/README tutarlılığı. Pin değişimini reddet → orijinal pini geri koy → kabul et; aynı aktif motor nesli ve normal owned cleanup. Canlı sağlayıcı/hesap/ikinci OS tekrarlanmaz; önceki kanıtın tarihsel kapsamı korunur.

Odaklı Rust `compatibility`/ilgili config ve manifest testleri, frontend compatibility client testleri ve gerçek-App UI; son adayda WORKFLOW tam yerel kapısı bir kez. Kaynak/config/test/araç/ortam + önce/sonra fingerprint aynı doğrulama adımında. Native UI gerçek get_compatibility_snapshot yolunu kullanır; mock yalnız izole UI olarak kaydedilir. Geçen bağımsız kontroller değişmedikçe tekrarlanmaz. Kanıt `work/proofs/compatibility/`, özet `docs/proofs/compatibility.md`, inceleme `docs/reviews/H036.md`.

## Sonuç —8 Kasım

H036 kabul edildi:173 frontend/386 Rust PASS,8 mevcut opt-in ignored; fmt/TS/Vite/native build ve84 audit/0 ihlal. Gerçek native Sky okuma/tarama/yenileme ve normal çıkış aynı motor nesliyle geçti;28 kontrol kaynak/artefakt/araç/OS bağlarını doğruladı. Tarama-busy eski veri bulgusu son adayda kapandı; ilgili frontend/build/UI yenilendi, değişmeyen Rust başarısı programatik yeniden bağlandı. [Kanıt](../proofs/compatibility.md), [inceleme](../reviews/H036.md).

Ana ajan bütün uygulamayı ve kabulü tamamladı. Aynı tek Luna salt okunur kanıt raporundan sonra kullanım limitine ulaştı; ikinci çalışan/model değişimi yapılmadı. Ölçülmüş geliştirme tasarrufu yok. Toplam34/51; H035 manuel tepsi kabulü açık, H037 başlamaz. H035'in tarihsel performans/DPI kanıtları yeni binary'nin toplu kabulü sayılmaz; kalan işe dönünce ilgili fingerprint kapsamı değerlendirilir. Gerçek indirme/installer/kurulu sürüm değiştirme/rollback H042'ye ait kalır.
