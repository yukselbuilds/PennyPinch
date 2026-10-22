# H016 — Yerel keşif kabulü

20 Ekim2026. Windows10 Pro22H2 x64 üzerinde H016 yerel geliştirme kabulü tamamlandı. Windows11, temiz kurulum, bağlantı/config değişimi veya canlı model/alt ajan uyumluluğu bu görevle doğrulanmış sayılmaz.

## Son aday ve doğrulama

- Backend: son focused discovery suite15 PASS/1 opt-in ignored; fmt ve offline check PASS. Ayrı ignored gerçek bounded environment taraması önceki eşdeğer davranış üzerinde1 PASS, aynı ortam ve yalnız debug trace farkı nedeniyle tekrar edilmedi. [Tanı eki](discovery-native-diagnosis.md) kaynak/komut/toolchain hash'lerini tutar.
- Arayüz:14 focused test/typecheck/build PASS; root32 görünüm/9 memory-fixture akışı, JS hata ve dış istek0. Production CSP ile yerel dist üzerinden, sunucu açılmadan çalıştı. Bu test gerçek IPC sayılmaz. `work/ui-review-h016/summary.json` ve [UI kanıtı](discovery-picker-ui.md). Son debug trace değişikliği UI/dist kaynaklarını değiştirmedi; geçerli kanıt tekrar edilmedi.
- Yeni embedded build:10,66s PASS. EXE14.098.432B, SHA256 `d2f33a9eef9adb07764d3b3a7ac788320f154991fc4fa159b0e0e179dac9eaae`; discovery source `df5a3137d0b96c7b8884b829a162fc2a187d581aa4e0e976669d7b6d13e60c11`, tests `7e306509471ca4bb49756460c960ef0e7bcc5359bc48fdd00d5273936c62e02c`. Diğer kaynak/config hash'leri alt proof'larda.

## Gerçek pencere yolu

PID1204/session17716/window1247184, gömülü yerel Tauri WebView. Kullanıcı Applications → OpenCode dosya seçimi → aynı0B test dosyası → Scan again akışında adayın kaldığını doğruladı. Root fixed metadata stderr kaydı ve gerçek AT metnini karşılaştırdı:

| Olay | Nesil | Manuel aday | Ret | Görünen toplam |
| --- | ---: | ---: | ---: | ---: |
| İlk gerçek otomatik tarama | 1 | 0 | 0 | 2 |
| Dosya seçimi kabulü | 2 | 1 | — | — |
| İlk yeniden tarama | 3 | 1 korundu | 0 | 3 |
| İkinci yeniden tarama | 4 | 1 korundu | 0 | 3 |

AT'de Codex PATH, Claude Code user-local ve OpenCode Selected file satırı görüldü; bütün bağlantılar doğrulanmadı, sürümler bilinmiyor. OpenCode satırı gerçek kurulumu değil, yalnız seçici için oluşturulan boş test dosyasını temsil ediyordu. Test dosyası veya istemci çalıştırılmadı. Kullanıcının ilk native denemesi seçici iptalini de içeren akıştı; bildirdiği tek sorun yeniden taramadaki kayıptı.

Root son pencereyi normal Alt+F4 ile kapattı; retained process handle exit0, son process sorgusunda Headroom yok. Önceki PID25548 adayı da normal kapanış exit0 idi. Tercih dosyası SHA256 başlangıç/son aynı `04ba8cfd197a03cf18ac2e1870b890ba4c90d060e0ba7d766e495ca61b43e7ac`; model/auth/ajan ayarı veya başka oturum değişmedi.

İzole test UUID5b415e24-b96b-46fb-8a26-6ceb7a68b5e4, pencere kapandıktan sonra original ancestor/reparse/absolute containment/exact tek çocuk/0B SHA kontrolüyle temizlendi. Önce yalnız opencode.cmd, ardından boş UUID dizini silindi; rekürsif silme yok, eski frozen hedefler değişmedi. `work/ui-review-h016/native-fixture.json` tarihsel manifesttir; dosya artık mevcut değil.

## Pratik sınır ve yayın kapısı

Önceki adayda kullanıcı yeniden taramayla satırın kaybolduğunu bildirdi; root sonrasındaki yokluğu gördü. Bu eski kaybın kök nedeni açıklanmadı. Yeni0B cmd/default-input ve gerçek bounded environment testleri geçti; yalnız debug metadata gözlemi eklendi. Son adayda iki ardışık gerçek yeniden tarama ve AT doğrulaması başarılıdır; “kök neden bulunup düzeltildi” iddiası yok. H039'a debug trace kapalı paketlenmiş adayda tekrarlı manuel seçim/rescan/ekran değişimi kontrolü eklendi; tekrar kayıp olursa yayın kabulü kapanmaz.

Computer Use capture ve indexed click hataları Headroom ile boş Notepad'de tekrarlandı; otomatik native mouse veya screenshot başarısı iddia edilmez. [Dış kayıt incelemesi](../research/computer-use-2026-10-20.md) yalnız bilgi kaynağıdır; helper yaması uygulanmadı. Kaynak/statik/native kanıtları [H016-c2 incelemesinde](../reviews/H016-c2.md) ayrı tutulur.

H016 yerel kapsamı tamamlandı; kullanıcı isteği gereği burada duruldu. Sonraki H040 başlamadı.
