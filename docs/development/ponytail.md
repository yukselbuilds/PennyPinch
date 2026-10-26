# Proje içi Ponytail

26 Ekim 2026. Yalnız Headroom geliştirme süreci için üç talimat becerisi kullanılır:

| Beceri | Kullanım |
| --- | --- |
| `.agents/skills/ponytail/SKILL.md` | Kodlama ve düzeltmede `full`: görevi bütünüyle çözen küçük değişiklik. |
| `.agents/skills/ponytail-review/SKILL.md` | Değişen kod, bağlı çağıranlar ve testler üzerinden kabul incelemesi. |
| `.agents/skills/ponytail-audit/SKILL.md` | İstenen kapsamda mevcut kodu denetleyen, kod değiştirmeyen rapor. |

Kaynak `DietrichGebert/ponytail`, sabit commit `09c434ab73ca43761c5d8b0c5d341926c2fba710`. Upstream metinleri ve MIT lisansı korunur; dosya hash'leri [kaynak manifestinde](ponytail-source.json), lisans [buradadır](ponytail-LICENSE.txt). Kurulumun yerel doğrulaması `work/proofs/dev-tools/` altında tutulur. Paket yöneticisi, ürün runtime'ı veya otomatik ağ servisi eklenmez.

## Projede uygulanışı

Ana ajan ve tek çalışan aynı dosyaları kullanır. Proje `AGENTS.md` kuralları skill metniyle birlikte uygulanır: `full` seviyesi, Türkçe rapor, mevcut model/çalışan düzeni, kapsam ve gerçek kanıt. Skill içindeki İngilizce rapor kalıpları Türkçe karşılanır. `Ship` ifadesi yayın/push eylemi için yetki değildir. Kurallar gerektiğinde skill dosyasından okunabilir; otomatik keşif tek başına doğru davranışın kanıtı sayılmaz.

Yeni kodlama işi normal skill seçimiyle `ponytail` kullanabilir. Görev kabulünde `ponytail-review`, ayrıca istenen eski kod denetiminde `ponytail-audit` açıkça seçilir. Günlük her küçük değişiklik bütün repo denetimi başlatmaz. Gerekli doğrulamalar, veri kaybını önleyen hata yönetimi, güvenlik, erişilebilirlik ve geri alma kontrolleri korunur.

Claude Code'a özgü hook, global aç/kapat ayarı, `gain` raporu veya model yönlendirmesi kurulmaz. Kullanım ölçümü yokken tasarruf yüzdesi verilmez. Ortak görev akışı ve kabul koşulları [WORKFLOW.md](WORKFLOW.md) içindedir.

## Keşif ve doğrulama sınırı

OpenAI'nin yerel skill belgesi repo içindeki `.agents/skills` yolunu ve otomatik keşfi tanımlar. Kurulum; doğru konum, geçerli metadata, sabit kaynakla hash eşleşmesi ve dosyaların okunabilmesiyle kontrol edilir. Bu mevcut konuşmada yeni skill kataloğu yüklenmediyse dosyalar doğrudan okunarak kullanılabilir; sonraki turda skill keşfi beklenir. Sırf keşif testi için model/sağlayıcı isteği veya başka sohbeti yeniden başlatma gerekmez.

Kaynaklar: [OpenAI yerel skill belgesi](https://learn.chatgpt.com/docs/build-skills), [sabitlenen Ponytail kaynağı](https://github.com/DietrichGebert/ponytail/tree/09c434ab73ca43761c5d8b0c5d341926c2fba710).
