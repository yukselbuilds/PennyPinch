# H041 — Context Diet / Context Mode yerel pilotu

16 Kasım 2026. Uygulayıcı ana ajan: ölçüm temeli ve hata/özgün çıktı sözleşmesi karar gerektiriyor. Billion Context kullanıcı kararıyla kapsam dışında.

Amaç, iki adayın Headroom 0.44.0'a **ek** katkısını model çağrısı yapmadan belirlemek. Bu pilot istemciye otomatik entegrasyon, görev kalitesi, sağlayıcı faturası veya yeni OS desteği kanıtlamaz. Ürün ve gerçek istemci ayarları değişmez; paket, süreç ve state yalnız `work/proofs/context-tools/` altında tutulur. İndirmeler sabit commit/sürüm ve programatik hash ile kaydedilir. Context Mode'un yerel MCP paketi kullanılır; Gateway, setup ve global hook kurulmaz. Context Diet'in değiştirilmeyen saf filtreleri mevcut TypeScript derleyicisiyle çalıştırılır; MCP kurulumu iddia edilmez.

## Önceden belirlenen kabul

1. **Normal:** Sabit tekrarlı log, benzersiz satırlı test çıktısı ve hedef kaydı olan büyük JSON; ham çıktı, Headroom, Diet → Headroom ve Mode → Headroom karşılaştırılır. İlk araç yanıtı ve gerektiğinde arama yanıtı ayrı sayılır. JSON alt kümesi hesaplaması tam JSON sıkıştırmasıyla aynı iş sayılmaz.
2. **Hata:** stdout üzerinden başarısız test, stderr uyarısı ve sıfır olmayan çıkış; hata kimliği/sayıları ve çıkış durumu korunmalı. Başarılı süreçte stderr de kontrol edilir. Kayıp, düşük token sayısıyla başarıya çevrilmez. Diet korumalı satırı sığdıramazsa açık hata vermeli. Mode özgün çıktısı ve arama sonucu sentetik beklenen içerikle karşılaştırılır.
3. **Sınır:** Kısa çıktı, Türkçe/Unicode, uzun kesin tamsayı/ondalık ve kaynak dosyası. Kaynak ve sayısal tam-okuma girdileri ürün entegrasyonunda geçiş gerektirir. Filtrenin doğrudan sonucu ayrıca gözlenir; pilot bunu sessizce geçişle gizlemez.

Her fixture en fazla 1 MiB; araç çağrısı en fazla 30 saniye; aynı anda tek sahipli Mode ve Headroom süreciyle sıralı çağrılar. Her fixture üç tekrar; her tekrar ayrı süreç/state ile başlar ve açılış ayrıca ölçülür. Her motorda 44 proxy isteği vardır: ürünün 60 istek/dakika varsayılan sınırı değiştirilmez. Önceki tek-motor denemesi 64. hızlı istekte yerel HTTP 429 aldı; bu araç sıkıştırma hatası değildir. Token ve kabul sonuçları tekrarlarda eşleşmeli. Kaynak/toolchain/config fingerprint'i doğrulama öncesi ve sonrası eşleşmeli. Performans için üretim SLA'sı koyulmaz; süreler bu küçük koşunun gözlemidir.

Ölçüm `o200k_base` yerel tokenizer'ıyla tam UTF-8 araç metni ve aynı Responses istek zarfı üzerinden yapılır. Diet'in karakter/4 tahmini yüzde hesabında kullanılmaz. Araç şeması/çağrı argümanı ve ek arama yanıtı maliyetleri ayrıca tutulur; seçilen çıktının küçülmesi bütün sohbetin/faturanın tasarrufu değildir. Model/auth/reasoning/araç kimliği Headroom kollarında aynı kalır. Özgün arşiv geri getirilemezse veya hata kaybolursa aday otomatik genel filtre olarak kabul edilmez.

Sonuçlar `docs/proofs/context-tools-pilot.json` ve kısa Türkçe değerlendirmeye bağlanır. Ürün kodu değişmediyse uygulamanın tam kalite kapısı tekrarlanmaz; pilotun gerçek MCP/filtre/Headroom yolu ve kendi hata kontrolleri çalıştırılır. H041 ana kabulü açık kalır.

## Yeniden üretme

Yeni ve henüz bulunmayan bir çalışma diziniyle, proje kökünde:

```powershell
python -B bench/context_tools/prepare.py work/proofs/context-tools/yeni-kosu
& 'work/proofs/packaging/artifacts/headroom-engine-0.44.0/runtime/python.exe' -I -B bench/context_tools/pilot.py work/proofs/context-tools/yeni-kosu
```

Hazırlık yalnız sabit kaynak/paketi indirir ve mevcut proje TypeScript 7.0.2 derleyicisiyle Diet'i JavaScript'e çevirir; bağımlılık veya global araç kurmaz. Transpilasyon typecheck değildir. Filtre/CLI davranışı altı upstream test dosyasıyla ayrıca doğrulanır. Eşleşen test kaynakları ve Node fingerprint'i bulunan önceki başarılı filtre kabulü `--reuse-diet-proof <result.json>` ile kullanılabilir; tüm pilotun başarılı olduğu varsayılmaz.

Küçülen her çıktı kabul edilmiş sayılmaz. Rapor; doğrudan görünür hata, tam-okuma gereği, tekrar sayısı, Mode arşivinin normalleştirmesi ve Headroom'un kendi kayıpsız çözücüsüyle/CCR ile birebir geri açılmayı ayrı alanlarda tutar. `evaluation_completed` deneyin bittiğini belirtir; otomatik ürün entegrasyonu onayı değildir.

Arama oracle'ı, beklenen tam hata satırını `intent` veya `queries` içinde göndermez. Araç sıfır sonuçta bile sorguyu yankılayabilir. Sorgu yalnız kısa anahtar sözcüktür; başarılı sayılan yanıt beklenen tam satırı dosya/indeksten getirmelidir. Özellikle kayıp stderr uyarısının sorgu yankısıyla bulunmuş görünmesi reddedilir.
