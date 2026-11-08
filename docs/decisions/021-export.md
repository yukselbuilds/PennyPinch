# H033 — istatistik ve tanı dışa aktarma

7 Kasım 2026 başladı, 8 Kasım tamamlandı; 32/51. H034 başlamadı. Ana ajan ölçüm anı, allowlist, dosya/süreç sahipliği, native entegrasyon ve kabul sahibi; mevcut tek Luna xhigh, dondurulmuş IPC sözleşmesinin frontend/client/test dilimini tamamladı. Root incelemesi tek düzeltme paketi ve son entegrasyon/kabulü kapsadı. Ponytail full, Emil ve DESIGN.md korundu.

## Kapsam ve sözleşme

Genel bakışta tek Dışa aktar menüsü, istatistik JSON/CSV ve tanı JSON sunar. İstatistik yalnız seçili dönemin içeriksiz toplam/kapsam özetidir; istek/model/sohbet listesi veya maliyet tahmini eklenmez. Her başarılı gerçek overview yanıtının yalnız güvenli aggregate projeksiyonu, mevcut request UUID'siyle RAM'de en çok8 kayıt tutulur. Export bu anı seçer; yeni ölçüm sorgusu yapmaz. Kayıt picker açılmadan alınır, dialog sırasında yeni trafik toplamları değiştirmez. Eksik/evicted kimlikte açık snapshot_expired, hata ekranında snapshot olmadan tanı mümkündür. Frontend path, dosya içeriği veya ölçüm değeri göndermez.

JSON signed saved_tokens/saved_percent, before/after, period/UTC sınırları, status/coverage ve sabit yerel kaynak açıklamasını korur; unknown null kalır. CSV UTF-8 BOM, CRLF, TR noktalı virgül/ondalık virgül ve EN virgül/ondalık nokta kullanır; metadata text hücreleri quote/formül başlangıcına karşı korunur. Güvenli toplam projeksiyonuna raw basis/tokenizer, model etiketi, auth, config, kişisel yol, HTTP/gövde/log veya sohbet kimliği girmez. Tanı yalnız allowlist sabit uygulama/OS mimarisi, kamuya açık durum/hata kategorileri ve varsa aynı aggregate anını içerir; yeni model/sağlayıcı çağrısı yoktur.

Native Save As mevcut COM/Windows bağımlılığıyla kullanılır; UI'ya seçilen yol/isim dönmez. Yalnız yeni dosya kaydedilir: mevcut dosya, link/reparse/unsafe path reddedilir. Parent dizin sahipliği/pin korunur; aynı parent'ta staging dosyası flush/sync sonrası replace etmeyen atomik move ile hedeflenir. İptal dosya üretmez, hata başarı değildir. Aynı anda tek export; exit sırasında yeni export yoktur, açık export için çıkış onayı busy olmalıdır. Yeni dependency/global dosya erişimi/OS tercihi eklenmez.

IPC `export_report({request:{request_id,kind,language,snapshot_request_id}})`, kind statistics_json/statistics_csv/diagnostics_json, language tr/en; statistics kimliği zorunlu, diagnostics nullable. Sabit saved/cancelled zarfında kind ve bytes_written; hata invalid_input/snapshot_expired/busy/unavailable/unsafe_path/file_exists/write_failed. Sırlar ve ham hata/path zarf dışıdır. Ürün yalnız gerçekten doğrulanan OS sınırıyla raporlanır; mevcut ücretsiz Windows11 istisnası korunur.

## Üç kabul senaryosu

1. Gerçek depodaki seçili dönem → UI'dan JSON/CSV Kaydet → dosya aynı capture'ın signed toplam/yüzde, before/after, period/source/coverage değerlerini taşır; sonradan gelen ölçüm veya dönem değişimi dosyayı değiştirmez. TR/EN,0/negatif/unknown/mixed ve CSV formül/canary sınırı doğrulanır.
2. İptal/meşgul/expired/disk veya unsafe-existing hedef → başarı yok, eski dosya/state korunur, sabit geri bildirim ve tekrar deneme. Picker açıkken yeniden export/exit yarışı; guard'lar bırakılır. Auth/model/client ayarları okunup rapora kopyalanmaz.
3. Gerçek native son derlemede Save As ile JSON, CSV ve tanı dosyası + Cancel; dosyalar backend/UI anıyla karşılaştırılır. Tüm tema/dillerde dar görünüm ve klavye/focus/pending/error; son adayın WORKFLOW.md yerel kapısı, programatik önce/sonra kaynak/araç/OS/artefact fingerprint'i. Normal state geri yüklenir.

Odaklı doğrulama: varlığı eklendikten sonra `node --experimental-strip-types --test tests/app/export-report.test.mjs`; Rust `export::` filtreli testler. Tam yerel kapı son adayda bir kez. Kanıt `work/proofs/export/`, rapor `docs/proofs/export.md`, inceleme `docs/reviews/H033.md`; TASKS/checkpoint her dilimle tutarlı güncellenir. Ölçülmemiş tasarruf oranı verilmez; H033 tamamlanınca durulur.

Sonuç:161 frontend/371 Rust PASS (8 opt-in ignored), aynı aday build ve fmt PASS.20 UI birleşimi +6 davranış grubu, native üç dosya ve Cancel için18 dosya kontrolü,180 kaynak/config'e bağlı16 toplu kontrol PASS. İlk native kesintisi ve son helper SQLite cleanup hatası ürün başarısı gibi sunulmadı; orijinal state ayrı fingerprint kayıtlarıyla geri yüklendi, kapalı process/port gözlendi. Helper düzeltilip dizin taşıma smoke'u doğrulandı. Native exit code kaydedilemedi; diğer OS/canlı sağlayıcı/installer sonucu çıkarılmaz. [Kabul raporu](../proofs/export.md).

Win32 davranışı pinned windows-sys0.61.2 kaynağı ve [Microsoft FILE_RENAME_INFO belgesi](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_rename_info) üzerinden kontrol edildi. `ReplaceIfExists=false`, var olan hedefi değiştirmez; rename açık handle üzerinden yapılır. Parent pin ve writer sınırları ayrıca gerçek Windows dosya sistemi testleriyle doğrulandı; güç kesintisi dayanıklılığı iddiası yoktur.
