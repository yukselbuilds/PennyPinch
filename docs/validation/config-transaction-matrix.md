# H018 — Config işlemi kabul matrisi

22 Ekim2026. **Yerel kabul tamamlandı.** Normal yetkide Windows10 Pro22H2 x64/build19045, aynı yerel sabit NTFS volume ve disposable proje fixture'ları. Gerçek istemci/auth/sohbet ayarı değiştirilmedi. A1 native TxF probe; A2 transaction_fs_tests; B1 transaction_tests; B2 ownership_tests; C1 connection_actions_tests. Güncel Rust testleri son160 PASS içinde kapsanır.

| Durum | Gerekli davranış | Sonuç / kanıt |
| --- | --- | --- |
| Engelli, süresi dolmuş veya bulunamayan plan | Config/sahiplik değişmez; sabit hata; sahte başarı yok. | **PASS** - C1 blocked/expired/replay/stale; UI Apply kapalı. |
| Tarama, kurulum, endpoint veya env kökü değişmiş | Stale/conflict; eski plandan keyfi hedefe yazım yok. | **PASS** - C1 root/discovery/endpoint drift; lease ve güncel kurulum kontrolü. |
| Aynı baytlarla farklı dosya kimliği; JSON↔JSONC slot drift | Eski fingerprint reddedilir; alternatif dosya sessiz seçilmez. | **PASS** - C1 same-bytes/new-ID ve alternate-slot appearance. |
| Config'te özel endpoint var | Engelli; önceki URL/auth hiçbir sahiplik kaydına yazılmaz. | **PASS** - B1 custom endpoint reddi; planner blocked. |
| Aynı endpoint zaten var, sahiplik yok | Yazım ve yeni sahiplik yok; kullanıcı ayarı Headroom'a aitmiş gibi geri alınmaz. | **PASS** - C1 no-op/store pasif; UI unowned Undo yok. |
| TOML/JSON/JSONC normal apply | Yalnız endpoint değişir; yorum/ilgisiz bayt/model/efor/auth/hook değişmez; parse geçerli. | **PASS** - B1 byte-exact/Unicode/CRLF/comment/comma; C1 ve Native üç format. |
| Var olmayan config | No-clobber yaratım; aynı anda kullanıcı dosyası oluşursa kullanıcı dosyası korunur. | **PASS** - C1 missing Codex, B1 missing Claude; A1/A2 absent-slot dış CREATE_NEW engeli. |
| Açık writer veya eşzamanlı replace/rename/delete | Seçilen primitive'in gerçek Windows davranışı kanıtlanır; kayıp değişiklik veya ölçülmemiş CAS iddiası yok. | **PASS** - A1 ayrı racer ve A2 parent/child intent lock; önceki writer admission fail-closed. |
| Commit öncesi hata | Eski config geçerli ve değişmemiş; sahiplik başarıya dönüşmez. | **PASS** - C1 Apply ve Undo ayrı precommit rollback; A2 poisoned/sealed commit reddi. |
| Commit/kayıt sınırlarında süreç kesintisi | Gerçek child fixture ile crash/reopen; bozuk config veya sahipsiz sessiz yönlendirme yok; belirsiz sonuç açık recovery/conflict. | **PASS** - A1 gerçek child exit73/74; iki dosya eski/yeni. Rust kill veya power-cut testi değil. |
| İlgisiz kullanıcı değişikliği sonrası undo | Yeni kullanıcı alanları/yorumları kalır; sadece hâlâ sahip olunan endpoint kaldırılır. | **PASS** - B1/C1; Native Codex/OpenCode atomic-save yeni inode+userNote korunumu. |
| Kullanıcı endpoint'i değiştirmiş/silmiş | Yeni değer ezilmez; conflict veya açık no-op sonucu; tüm yedek geri yüklenmez. | **PASS** - B1/C1; Native OpenCode değişmiş endpoint conflict, config+record hash unchanged. |
| Oluşturulan container/dosyaya kullanıcı alan eklemiş | Kullanıcı içeriğiyle birlikte container/dosya silinmez. | **PASS** - B1 created-ancestor edits; C1 missing-file leaf undo. Bütün dosya silinmez. |
| Tekrarlanan apply/undo, aynı config'i paylaşan kurulum | Çift sahiplik, yinelenen alan veya yanlış kurulumdan geniş yetki oluşmaz. | **PASS** - C1 replay/no-op/revision ve B2 owner guard; UI pending tek dispatch. |
| Ownership kaydı bozuk, büyük, bilinmeyen şemalı veya değişmiş | Fail-closed; config değiştirilmez; ham kayıt/sır hata çıktısına taşınmaz. | **PASS** - B2 strict codec/schema/duplicate-key/source/mask/16KiB/root/revision. |
| Junction/reparse/hardlink/unsafe root veya salt okunur dosya | Desteklenmeyen yazım reddedilir; ACL/attribute sessiz gevşetilmez; hedef dışı değişiklik yok. | **PASS** - A2/B2 gerçek junction/hardlink/readonly ve parent rename kilidi. EFS guard kaynak incelemesi var; gerçek EFS fixture yok. |
| Uygulama restart'ı | Kalıcı sahiplik ve gerekli geri alma durumu bulunur; eski süreli plan kendiliğinden canlanmaz. | **PASS** - B2 reload; Native Claude yeni süreçte Preview olmadan aynı kayıtla Undo. |
| UI eylemi → komut → manager → gerçek dosya | Native disposable köklerde apply/undo gözlenir; sentetik keşif metadata'sı gerçek istemci uyumluluğu sayılmaz. | **PASS** - Native üç format Apply/Undo, OpenCode conflict/Reload ve Claude restart; iki exit0. |

Kanıtlar: [son gate](../../work/proofs/config-transaction/final-quality.json), [statik UI](../../work/proofs/config-transaction/ui-review/summary.json), [native](../../work/proofs/config-transaction/native-proof.json), [12 dosya kontrolü](../../work/proofs/config-transaction/native-checks.json), [A1 ve alt dilimler](../proofs/config-transaction.md).160 Rust/49 Node PASS; frontend/typecheck/embedded build PASS. Geçerli format/frontend hash/toolchain ile tekrar edilmeden kullanıldı. Windows11, cross-volume, gerçek EFS/güç kesintisi ve canlı auth/model/ana-alt ajan ölçülmedi. [TxF bakım/gelecek desteği riski](../decisions/005-config-transaction.md) korunur. H019 başlamadı.
