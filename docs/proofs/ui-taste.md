# UX001 — yeni görsel dilin yerel kabulü

Başlangıç5 Kasım, son kabul6 Kasım2026. [Tasarım sözleşmesi](../../DESIGN.md), [inceleme](../reviews/UX001.md). Graphite/mint ailesinde dört ekranın ortak hiyerarşi, tipografi, boşluk, SVG ikon ve boş durum sunumu yenilendi. Yeni ürün işlevi eklenmedi; H029 başlamadı.

## Kaynak ve kurulum

Kullanıcının verdiği dört kaynak birincil GitHub kaynaklarından okundu. Günlük ürün arayüzü için [Emil design engineering](https://github.com/emilkowalski/skills/tree/c8a79627b59d72d764d6ab6f2c87c09a48169833/skills/emil-design-eng) seçildi. Resmî skill-installer ile yalnız `.agents/skills/emil-design-eng` kuruldu; MIT lisansı ve immutable commit/hash `UPSTREAM.json` içindedir. Global ayar/model değişimi ve yeni UI/motion bağımlılığı yoktur. Diğer üç kaynak kurulmadı; inceleme metadata'sı `work/references/design-skills/` altındadır.

| Kontrol | Gözlenen | Kanıt |
| --- | --- | --- |
| Gerçek native kontroller | UI Start/Pause/Resume aynı nesil/endpoint; ayrı Stop ve yeni nesille restart. Dört gerçek ekran: Genel bakış/Uygulamalar/Etkinlik/Ayarlar. Model çağrısı0. | `work/proofs/taste/ui-runs/b6f83419-3f3a-4833-a6e7-6d8071cb1224/result.json` |
| Pending ve hata | Aynı gerçek App, native IPC'siz iframe'de ack öncesi başarı göstermedi, ikinci eylemi kilitledi; başarısız ack+read unknown oldu, taze okuma toparlandı. | Aynı native koşu, izole full-App fixture |
| Veriler ve görsel durum | Strict client'tan geçen normal/0/negatif/mixed; stale önceki veriyi korur, ilk okuma hatası ayrı görünür.16 gerçek fixture kovası,2 model grubu; üretimde örnek veri yok. | Aynı native koşu; `data-validation-1.json` dört dataset doğrulaması |
| Dil/tema/ölçek | TR/EN, açık/koyu,1024/819/683 frame genişlikleri; gerçek WebView125/150/100, reset; Enter/Space/Escape ve activity dialog; reduced-motion. | Aynı native koşu;10 kontrol grubu,0 page error |
| Kontrast | Dört kritik metin/kontrol seçicisi, iki dil/iki temada ölçüldü; en düşük4.68:1. Bu sınırlı kontrol tam WCAG sertifikası değildir. | Native koşunun `contrasts` dizisi |
| Görsel inceleme | Gerçek boş ekranlar ile açıkça işaretlenmiş örnek ölçüm ekranları gözle incelendi. Tam yüzey örnekleri, arka native pencereyi karıştırmayan genişletilmiş izole frame'de alındı. | `visual-runs/f392ab84-2f92-4177-8818-4369ba11f221/result.json`, `sample-dark.png`, `sample-light.png` |
| Yerel ürün kapısı |138 frontend/349 Rust PASS,6 mevcut opt-in ignored; TypeScript/Vite,fmt ve offline custom-protocol build PASS. | `render-build-2.json` ilk iki ürün derlemesi + `final-checks-1.json`;152 girdi aynı son adaya aittir |

Tüm work yolları `work/proofs/taste/` altındadır. `acceptance.json` sonuç ile kaynak/config, araç/ortam, native EXE/dist/fixture/runner, tasarım rehberi/lisans ve devir kayıtlarını güncel baytlarla karşılaştırır. Son aday değişmediği için başarılı ürün derlemeleri tekrar çalıştırılmadı; iki ölçüm bloğu aynı fingerprint ile birleştirildi. Örnek dataset düzeltmesi yalnız QA bundle'ını yeniledi; üretim kodu ve EXE değişmedi.

Test pencereleri exit0 ile kapandı; sahipli debug portları kapalı, mevcut Codex config/preferences/onboarding/ownership fingerprint'leri aynı. Ham provider içeriği, auth veya exception dökümü tutulmadı. UI screenshot'ları bu görevin uygulamasına/etiketli fixture'ına aittir.

Bu Windows10 22H2/build19045 geliştirme kabulüdür. Native motorun soğuk başlangıcı bazı40s test beklemelerini aştı; üretimin mevcut100s/90s sözleşmesiyle kontrol geçti. Bu, başlatma performansı/uzun yük kabulü değildir. Windows11/macOS/Linux/installer ve canlı sağlayıcı bu işte sınanmadı. Örnek yüzde kullanıcı veya geliştirme tasarrufu sayılmaz.27/51 işlevsel ürün kabulü aynı; UX001 sunum yenilemesidir.
