# H041 — Context Mode ve Context Diet yerel pilotu

16 Kasım 2026. **Pilot tamamlandı; iki araç genel otomatik filtre olarak kabul edilmedi.** Billion Context kullanıcı isteğiyle kapsam dışında. Gerçek istemci ayarları, ürün kaynakları ve varsayılanlar değişmedi.

Windows 10 x64 üzerinde Context Diet 0.5.0 saf filtre/CLI kodu ve Context Mode 2.2.0 gerçek stdio MCP paketi sınandı. Diet sabit Git commit’inden, Mode SHA-512 bütünlüğü doğrulanmış npm paketinden geldi. Diet upstream’inin 49 testi geçti; aynı 23 kaynak/Node fingerprint’iyle son koşuda yeniden kullanıldı. MCP kurulumu veya Codex hook uyumluluğu iddia edilmez.

**14 sentetik komut çıktısı × 3 tekrar × 3 kol = 126 karşılaştırma; ayrıca 6 JSON sorgu isteği.** Her tekrarda ayrı Mode/Headroom süreçleri vardı. Üç motorda da coding/cache, TTL 7200, yerel ölçüm açık, beacon/model yönlendirme/learn/output shaper kapalı kaldı. Mode’un üç EOF kapanışı kendi SIGTERM davranışıyla Windows exit 1; sahiplik dosyası kalmadı. Headroom üç kez graceful kapandı. 838 girdi önce/sonra eşleşti.

## Ölçüm tablosu

Aşağıdakiler aynı Responses zarfındaki **ilk upstream istek JSON’unun yerel o200k_base token sayımıdır**; sağlayıcı tokenı veya fatura değildir. Mode sütunu kendi MCP metnini, diğer kollar stdout/stderr/exit JSON zarfını taşır. Yalnız zarf/escaping farkı da sayıya etki edebilir. Üç tekrarın token ve kabul alanları eşleşti.

| Senaryo | Headroom | Diet → Headroom | Mode → Headroom | Kabul notu |
| --- | ---: | ---: | ---: | --- |
| build_fail | 173 | 173 | 1350 | Çıktı kapsamı ve varsa geri alma ayrı doğrulandı. |
| event_count | 156 | 144 | 139 | Diet 200 olayı 1 satıra indirdi; tekrar sayısı model çıktısında kayıp. |
| git_diff | 287 | 287 | 265 | Çıktı kapsamı ve varsa geri alma ayrı doğrulandı. |
| git_status | 158 | 158 | 939 | Çıktı kapsamı ve varsa geri alma ayrı doğrulandı. |
| json_exact | 2220 | 2065 | 1766 | Diet tam JSON’u eksiltti; kazanç sayılmaz. |
| long_source | 13406 | 1882 | 1460 | İki adayın ilk yanıtı tam kaynağı taşımıyor; tam Read için geçiş gerekli. |
| repeated_log | 155 | 155 | 179 | Diet ek küçülme sağlamadı; Mode ilk isteği büyüttü. |
| short_unicode | 160 | 160 | 146 | Çıktı kapsamı ve varsa geri alma ayrı doğrulandı. |
| source_read | 2790 | 1882 | 2615 | Diet tam kaynağı eksiltti; kazanç sayılmaz. |
| stderr_exit | 175 | 177 | 248 | Çıktı kapsamı ve varsa geri alma ayrı doğrulandı. |
| stderr_success | 163 | 163 | 136 | Mode başarılı komutun stderr uyarısını yanıttan ve indeksinden çıkardı. |
| stdout_fail | 156 | 173 | 219 | Hata satırı iki adayda görünür; Headroom’da birebir CCR geri alımı var. |
| test_fail | 184 | 184 | 1086 | Hata ayrıntısı Headroom’dan birebir geri alındı; adaylarda doğrudan görünür. |
| test_pass | 157 | 157 | 1051 | Çıktı kapsamı ve varsa geri alma ayrı doğrulandı. |

Headroom’un bu 132 istekte uyguladığı dönüşümler CCR veya sabitlenmiş kayıpsız run/block çözücüsüyle **birebir geri açıldı**. 42 CCR GET doğrulaması yapıldı. Bu, Diet’in önceden attığı içerik veya Mode’un toplamayı atladığı stderr için bir geri alma garantisi değildir. Modelin bu geri çağrıları seçmesi ve sonraki token maliyeti ölçülmedi.

## Faydalı kullanım: hedefli veri sorgusu

1.000 satırlık sayısal JSONL dosyası 36,000 yerel metin tokenıydı. İstenen satır ve satır sayısı, elde yazılan yerel sorguyla 41 tokena indi: **bu yanıt kapsamında %99.89 küçülme**. 18446744073709551617 ve 0.1234567890123456789012345 metinleri üç tekrarda birebir kaldı. JSON sayıları JavaScript Number’a dönüştürülmedi. Tam dosya okuması ile aynı görev değildir; sınanan görev yalnız sayı ve hedef satırdı.

Giderler ayrıca kayıtlı: yürütme argümanları 104, MCP şemalarının kanonik JSON’u 1732, başlangıç yönergeleri 115, indeks yanıtı 20, arama yanıtı 265 yerel token. Bunlar genel sohbet/fatura için net tasarruf yüzdesine çevrilmedi. Üç ayrı FTS araması da tam hedef satırı getirdi.

## Hata ve geri alma sınırları

- Diet exactOnly ve korumalı satırlarla çalıştı. Bütçeye sığmayan zorunlu satır açık hata üretti. Manifestteki verbatim, tüm bilginin kaldığı anlamına gelmiyor. Tekrar adedi, kaynak/JSON bütünlüğü ve özgün arşiv sağlanmadan genel filtre yapılamaz. Exit ve kanal zarfını pilot adaptörü korudu; bu Diet’in kendiliğinden özelliği değildir.
- Mode’da exit 0 olduğunda stderr birleşik metne eklenmedi. Uyarı, tekil anahtarla yapılan ek aramada da geri gelmedi. Shell stdout-only exit 1, metinde exit 1 görünmesine rağmen isError=false oldu. Tam Read koruması ve uygulamanın izin/çıkış sözleşmesi ayrıca gerekir.
- Mode’un indekslediği metin satır sonlarını normalleştiriyor. Uzun CRLF kaynağı normalleştirilmiş biçimde geri kuruldu; birebir bayt arşivi olarak kabul edilmedi.
- Genel kısa sorguların 21 kontrolünden 12’si gereken tüm satırları gösterdi. Test özeti için geniş test sorgusu yeterli değildi; result ve tekil hata kimliğiyle yapılan altı odaklı aramanın altısı geçti. Kayıp stderr’in üç araması yine başarısız kaldı. Sorgu yankısı kanıt sayılmadı. Bu aramalar model görev başarısı testi değildir.

## Karar ve sonraki adım

Context Mode’un **büyük veriyi yerelde sorgula, gereken kısmı getir** yaklaşımı değerli bir aday. Otomatik genel çıktı yakalama için bu paket doğrudan açılmayacak. Her istemcinin mevcut izinleri, tam stdout/stderr/exit/araç kimliği, byte-exact özgün depolama ve model/efor değişmeden görev başarısı kabulü gerekir. Diet ayrı runtime bağımlılığı olarak seçilmedi; mevcut Headroom güçlü olduğu için log örneğinde ek kazanç yok, doğrudan uygulandığında veri kaybı var. Korumalı kaynak/sayısal Read ve ürün varsayılanları korunur.

H041 ana görevi açık ve toplam 45/54. Sıradaki normal dilim Chat Completions kullanım ölçümüdür; sonrasında güvenli hedefli sorgu prototipi değerlendirilebilir. Canlı model, API faturası, toplam görev kalitesi, yeni işletim sistemi, yeni kurucu veya Actions testi yapılmadı. Geliştirme sohbeti token tasarrufu ölçülmedi.

[Programatik kabul](context-tools-pilot.json) · [Tekrarlanabilir sürücü](../../bench/context_tools/README.md) · [Context Diet sabit kaynak](https://github.com/techdevfun/context-diet/tree/3899b48dbf41ecc9c0e29aaec8634900101c52d2) · [Context Mode incelenen kaynak](https://github.com/mksglu/context-mode/blob/751f818d0889f7e7da09d050e695c94ec77463f0/src/core/tools/execute.ts#L138-L167)
