# DEV001 — araçlar ve kabul düzeni

26 Ekim 2026. **Kabul edildi.** Ürün kodu değiştirilmedi; ürün sayımı 18/51, H022 başlamadı. Sonraki geliştirme işi DEV002 mevcut kod denetimidir.

| Kontrol | Sonuç ve sınır |
| --- | --- |
| Ponytail kurulumu | Kodlama, audit ve review skill'leri yalnız `.agents/skills` altında. Commit `09c434ab73ca43761c5d8b0c5d341926c2fba710`; üç dosya kaynak Git blob'larıyla byte-exact. MIT lisansı korunuyor. |
| Kaynak kaydı | `docs/development/ponytail-source.json` taşınabilir pin/hash/argv kaydı. Makineye özel kurulum ayrıntıları ignored kanıtta. |
| CodeGraph | Sabit 1.6.2 runtime/launcher/MCP kaydı korundu; `.agents/` ile 59 dışlama. Config değişimi sonrası sync/status/files PASS: 76 kaynak girdisi, sıfır `.agents` girdisi; Rust ve TS hedefleri mevcut. Son girinti düzeltmesinde parse edilmiş config aynı olduğundan bu kontroller tekrarlanmadı. |
| Canlı MCP | Bu sohbetin gerçek `codegraph_explore` aracı `undo_owned_endpoint`, `maxFiles: 1` sorgusunu başarıyla yanıtladı; sembol ve `transaction.rs` bulundu. Yanıt 15.331 karakter; yalnız sonuç metadata'sı kaydedildi. |
| Görev kabulü | Ortak beş koşul, kısa çalışan devri, mevcut komutlar ve kanıt tekrar kullanımı WORKFLOW'a bağlandı. H022 senaryoları somutlaştırıldı; H039'daki henüz mevcut olmayan scriptler PASS yolu olmaktan çıkarıldı. |
| Ürün testleri | Önceki H021 gate'inin 73 kaynak/config girdisi aynı. Araç/skill/belge değişimi için ürün testleri yeniden çalıştırılmadı; tarihsel gate yeni çalıştırılmış gibi sunulmadı. |

Ana ajan `ponytail-review` dosyasını okuyarak değişiklikleri ve bağlı kuralları inceledi. Somut inceleme düzeltmeleri: taşınabilir kaynak manifesti, yeniden kullanılabilir gerçek argv ve gereksiz config girinti farkı. Kodlama seviyesi `full`, rapor dili Türkçe; güvenlik/rollback/test hükümleri ve tek çalışan düzeni korunur.

Skill dosyalarının konumu, metadata'sı ve kaynak eşleşmesi doğrulandı. Çalışmakta olan host'un yeni skill'i otomatik seçmesi ayrı gözlemlenmedi; bu tur review dosyası doğrudan okunarak kullanıldı. Sonraki tur skill keşfi beklenir. Bu durum CodeGraph MCP canlı sorgusundan ayrıdır. Global ayar, provider/model testi, CI veya push yapılmadı; token tasarrufu ölçülmedi.

Makineye özel kısa kanıtlar `work/proofs/dev-tools/` altında: `ponytail-install-v1.json`, `mcp-live-root.json`, `root-acceptance.json`. Sonuncusu belge bağlantıları, görev sayımı, protected checkpoint sınırı ve son dosya fingerprint'lerini taşır. Ham sorgu, kaynak veya hata dökümü tutulmaz.

[Ortak akış ve bitiş kabulü](../development/WORKFLOW.md) · [Ponytail](../development/ponytail.md) · [CodeGraph](../development/codegraph.md)
