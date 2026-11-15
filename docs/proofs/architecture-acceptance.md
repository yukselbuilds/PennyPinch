# H012 mimari ve uyumluluk kabulü

15 Kasım 2026. H012'nin bileşen, süreç, adaptör, ölçüm ve destek sözleşmesi mevcut Rust/TypeScript kaynağıyla uzlaştırıldı. [Programatik kabul](architecture-acceptance.json), [inceleme](../reviews/H012.md), [runtime kararı](../decisions/002-runtime.md), [arayüz sözleşmesi](../contracts.md).

| Kabul | Gözlenen |
| --- | --- |
| Kabuk ve motor seçimi | Tauri 2.12.4, React 19.3.1, TypeScript 7.0.2, Vite 8.3.4, CPython 3.13.13 ve Headroom 0.41.0 manifestlerle eşleşiyor. |
| IPC ve güven sınırı | Kaynaktaki 39 Tauri komutunun tamamı belgede bir kez var. Runtime, hesap, sohbet, update ve export ailelerinin farklı zarfları korunuyor. |
| Adaptör ve kalıcı kayıt | V1/V2 plan ve receipt ayrımı, ortak `opencode_v1` slotu, V2 `mcp.servers.headroom` ve izin kapısı belgelendi. |
| Ölçüm temeli | Dönem ve istek toplamlarının gerçek alan/durum adları, boş runtime placeholder'ı ve kaynak toplama durumu ayrıldı. |
| Destek kataloğu | 14 istemci, 1 kabiliyet ve 3 ortam kaydının kimliği, durumu ve kanıt hedefi doğrulandı. Bilinmeyen birleşimler temsil ediliyor. |
| Katman kapsamı | Headroom protokol rotası ve terminal filtresi ölçümü ayrı. RTK-OSS 1.1.0 için kabul edilmiş entegrasyon kaydı yok. |

H005–H011 teknik kanıtları ve H050/H051 hesap/sohbet kararları kendi hash'leriyle incelemeye bağlandı. H010'un yeni ana kabulü mevcut OpenCode kapsamını kesinleştirdi. H006/H009 ve yayın görevlerinin dış ortam/hesap eksikleri, destek durumlarında açık kalıyor.

Normal kabulde komut ve destek kayıtları eşleşir. Tekrarlı/eksik komut, bilinmeyen durum, kayıp kanıt, yanlış bağımlılık sürümü veya kaynak/katalog drift'i doğrulamayı durdurur. Belgedeki yerel bağlantıların proje içindeki gerçek dosyaları göstermesi kontrol edilir.

Bu tur yalnız belge ve kabul kayıtlarını değiştirdi. Ürün/katalog fingerprint'i son yerel kapının 344 girdisiyle eşleşti. 222 Node/462 Rust, 19 ignored, fmt/frontend/embedded build kanıtı yeniden kullanıldı. Belge değişiminden önceki ve doğrulama sırasındaki hash'ler ayrı kaydedildi.

Kaydedilen kabul komutu aşağıdadır. Doğrulayıcı Türkçe görev dosyasını UTF-8 ile okur. İlk kontrolün özgün doğrulayıcısı ve hash'i saklandı.

```powershell
& 'D:/Projeler/Headroom/work/proofs/packaging/artifacts/headroom-engine/runtime/python.exe' -I -B 'D:/Projeler/Headroom/work/proofs/architecture-contracts/close.py' check acceptance-1794738683856465400
```

H012 kabulü bir arayüz/mimari sözleşmesidir. Yeni canlı sağlayıcı, işletim sistemi, installer veya üretim yayın deneyi yapılmadı. Kullanıcının H049 ve H052–H054 ertelemeleri sürüyor. Geliştirme token tasarrufu ölçülmedi.
