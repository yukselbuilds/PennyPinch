# H023'e kadar hızlı toplu kontrol

3 Kasım 2026. Kullanıcının toplu kontrol isteği; yeni ürün görevi veya H024 başlangıcı değildir. Ana ajan yaptı. **Yerel kontrol PASS; bütün23 görevin tam kabulü anlamına gelmez.**

| Kontrol | Sonuç |
| --- | --- |
| Geçerli standart kapı |97 uygulama/285 Rust testi PASS; standart kapıda3 ignored.81 girdi,3 dist, binary, araç/ortam aynı olduğu için başarılar yeniden kullanıldı. Format/frontend build/native build kanıtları da eşleşiyor. |
| Yeni gerçek motor testi | PASS: izole native manager start → Ready → stop → restart; aynı ingress, yeni nesil, yanlış nesille stop reddi, yabancı listener'ın korunması ve kapanış/temizlik. Yalnız yerel readiness GET'leri, sağlayıcı isteği yok. |
| Yeni gerçek keşif testi | PASS: boş ve çalıştırılmayan manuel OpenCode adayı gerçek ortam taramasında kaldı; kurulum/istemci başlatma veya kullanıcı config Apply yok. |
| Opt-in süre | İki test birlikte83,5 saniye; performans/uzun kararlılık ölçümü değildir. Filesystem child yardımcısı tek başına çalıştırılmadı. |
| Runtime bütünlüğü |8 kritik manifest/helper/runtime pin'i eşleşti; bütün22.225 payload dosyasının yeniden denetimi değildir. |
| Görev/kanıt düzeni |51 ana görev/20 kabul; H001–H023 içinde17 kabul,6 açık/kısmi.66 yerel kanıt bağlantısı mevcut. |

İki belge kayması düzeltildi: TASKS tarihsel paragrafındaki eski18/51 değerinin güncel yetkili toplam gibi gösterilmesi ve source-checks'in test dosyalarını sekizle sabitlemesi. Ürün kodu değişmedi; H024 başlamadı.

H006 ve H008–H012 açık/kısmi kalır. Canlı hesap/sağlayıcı ve bütün model/alt ajan kapsamı, Win11/temiz kurulum/macOS/Linux, yeni bütün-GUI/aktif stream-pause/retrieval veya uzun süreli kararlılık bu hızlı kontrolle tamamlanmış sayılmaz. [H023 kapsamı](metrics-normalization.md), [görev durumu](../../TASKS.md).

Komut: `cargo test --offline --manifest-path src-tauri/Cargo.toml -- --ignored --test-threads=1 --skip external_filesystem_operation_child`. Mevcut Cargo/Rust1.92.0, proje içi offline cache/target ve tek test thread'i kullanıldı. Bu opt-in operasyon kontrolü güncel aşama için yenilendi; değişmeyen deterministik standart testler yinelenmedi.

Kanıt: `work/proofs/quick-local-check/23538ad5-e0de-4d61-b726-cc39f93b4025/{live-check,summary}.json`. Sonuç ve ilgili kaynak/ortam önce/sonra fingerprint'leri aynı adımda kaydedildi. Ham hata/HTTP/prompt/auth kaydı, ücretli kaynak veya CI kullanılmadı. Geliştirme token tasarrufu ölçülmedi.
