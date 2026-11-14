# H010-d — Windows 10 tam OpenCode TUI kabulü

14 Kasım 2026. **Yerel dilim kabul edildi; ana H010 ve toplam 42/54 açık.** Windows 10 22H2 x64/build19045, gerçek OpenCode 1.19.3 binary'si ve Headroom 0.41.0. [Programatik kabul](opencode-tui.json), [plan](../decisions/009-opencode-provider-routes.md#h010-d--tam-tui-kabulü), [inceleme](../reviews/H010-d.md).

## Gerçek terminal ve ürün yolu

H010-b3'ün aynı hash'li release'i/paket manifesti ve H010-c'nin native config/metadata yardımcıları kullanıldı. Native Başlat → OpenCode Önizle → Uygula, fixture kullanıcı JSONC'sine iki provider ve ürün retrieval köprüsünü yazdı. Her terminal normal kaydedilmiş dosyayı okudu; inline config veya endpoint override yoktu. Standart ürün kodu/paket yeniden derlenmedi.

Varsayılan tam TUI komutu gerçek Windows PTY'ye bağlandı; stdin/stdout/stderr TTY'dir. `run --format json`, `--mini` veya UI yerine provider çağıran bir test kullanılmadı. Terminale `/models`, arama metni, Enter, kullanıcı mesajı ve Enter gönderildi. Final gerçek arayüzde gözlendi; ardından `/exit` ve Enter ile çıkıldı. Ayrı kontrollü metadata CLI'si yalnız izole profilin session list/export'unu okudu; transcript ve terminal gövdesi rapora taşınmadı.

| Tur | Açık TUI model seçimi | Gözlenen |
| --- | --- | --- |
| Aynı model | `Headroom base fixture` → `lab/slot` | Parent ve modeli miras alan gerçek child; araç/CCR/tanık/final tamamlandı. |
| Aynı sohbette değişim | `/models` → `Headroom switch fixture` → `lab/other` | Parent session ID aynı; farklı child açıldı ve yeni modeli miras aldı; bağlantı yeniden kurulmadı. |
| İki protokol | `Headroom Responses fixture` → `openai/slot`; child `lab/child` | Responses parent ve Chat child ayrı gerçek kimliklerle iki kayıtlı rotayı kullandı. |

İki parent ve üç farklı foreground child, en fazla bir eşzamanlı child vardır. Task metadata'sı ile child export parentID/agent/provider/model alanları eşleşti. Toplam 27 upstream mesajında iki rolün altı log bloğu 14.999→103 bayta indi ve özgün içerik hash/bayt düzeyinde aynı geri geldi. Chat ürün MCP retrieval'ı, Responses parent motorun server-side continuation yolunu kullandı. Call ID, model ve sentetik auth korundu; parent/child araçları completed, final'ler doğru.

Üç terminalin gerçek CLI exit0, source/config/owner fingerprint'leri aynı ve forced=false; süreçler kendilerine ait Windows Job'a bağlıdır. Bu, beklenmedik yardımcı kapanışında orphan engeli sağlar; bütün çocukların ayrı graceful shutdown ölçümü değildir. GUI onaylı normal exit0 ile kapandı. Aynı native exclusive gate'li `--headroom-uninstall keep` bağlantıyı geri aldı: byte-exact özgün JSONC, released owner, kapalı listener ve korunmuş state. Küçük, içerik doğrulanmış fixture settings başlangıç kaydı da birebir geri yüklendi. Fiziksel fare/klavye kullanıcının ön plandaki uygulamasına gönderilmedi; native GUI erişilebilirlik eylemleri arka planda çalıştı.

## Model seçimi ve önceki denemeler

Olumsuz resume denemesinde `--model lab/other` ipucuna rağmen önceki TUI modeli korundu. Bu koşu kabul edilmedi; doğru model seçilmiş gibi raporlanmadı. Kabul, kullanıcının model menüsünden yaptığı açık seçimi kapsar. Pinned local state'te agent/model store CLI fallback'inden önce gelir; model dialog'u aramayı title/category üzerinden yapar. Bu nedenle fixture model adları ayırt edilebilir tutuldu. [Model state kaynağı](https://github.com/anomalyco/opencode/blob/e3c4eb242585dcd77be1a867391b7c25fd47ce52/packages/tui/src/context/local.tsx), [model menüsü kaynağı](https://github.com/anomalyco/opencode/blob/e3c4eb242585dcd77be1a867391b7c25fd47ce52/packages/tui/src/component/dialog-model.tsx). Kaynak okumak runtime PASS değildir; doğru gerçek seçim son export'ta ayrıca doğrulandı.

İlk salt başlangıç probe'unda CLI exit0/TTY vardı, dış wrapper exit1 idi; model turu yoktu ve tam kabul sayılmadı. `5acf73fd324c`, süreç job koruması tamamlanmadan model trafiği başlamadan normal kapatıldı. `e1617ea4930e` aynı model turunu geçti, resume modeli nedeniyle switch başarısız oldu; terminal kendi 180s sınırında sonlandı. Controller kesildiği için normal GUI/settings kapanışı gözlenmedi; sonuç tamamlanmış sayılmadı. Devamda süreç/listener olmadığı kontrol edildi, native Undo özgün istemci ayarını birebir döndürdü. Önceki fixture settings baytları bellekte kaybolduğu için yalnız QA state'i kapalı loopback9'a sıfırlandı. [Kanonik JSON](opencode-tui.json) eski partial/recovery dosyalarını hash'leriyle bağlar.

Son aday, kendi ayarı için durable başlangıç kaydı ekler. Yalnız uygulamaya ait proof köküne, tek loopback ayarı içerdiği doğrulanan sınırlı fixture JSON'u yazılır; gerçek kullanıcı/auth yedeği alınmaz. Negatif test yanlış içerik/uzak URL için kayıt oluşmadığını doğrular.

## Doğrulama ve tekrar

Nihai native kayıt `work/proofs/opencode-tui/runs/a13dae9bba7a/result.json` passed:true. 19 odaklı kontrol geçti: beş TUI export/izin/fixture testi, on subagent/fixture backup kontrolü ve dört mevcut OpenCode proof helper testi. `focused-selection.json` ve `focused-selection-extra.json` kaynak/araç önce-sonra eşleşmesini taşır. 363 native girdi değişmedi; kaynak, CLI/runtime/binary, settings/config ve terminal parmak izleri programatik bağlandı. Sekiz TUI kaynağı iki manifestte pinlidir.

Aynı 332 ürün girdisi ve araçlar eşleştiğinden 222 Node/448 Rust (18 ignored)/fmt/frontend/embedded build kanıtı yeniden kullanıldı. Değişen benchmark ve yeni helper bu eski PASS'in içinde sayılmadı; odaklı ve gerçek TUI akışı ayrıca doğrulandı. Kaynak değişmeden başarılı kontrol tekrarlanmaz.

Ön koşul: H010-b3 QA release/paket ve pinned Python/OpenCode hazır, aynı QA state'te aktif bağlantı olmamalıdır. Repo kökünde ilk komut controller'ı açar. Native arayüzde Başlat → OpenCode Önizle → Uygula; yazdırılan koşu dizininde `run-all` dosyası oluşturulur. Ayrı gerçek terminalde ikinci komut `same`, ardından `switch`, ardından `mixed` ile çalıştırılır. Her tur yukarıdaki görünür model adı menüden seçilir, `HEADROOM_PARENT_<tur>` yazılıp Enter gönderilir; final sonrası `/exit`. Controller geçerli `same.json` yazdıktan sonra switch açılır. En sonda GUI normal onayla kapatılır; native Undo controller tarafından yürür.

```powershell
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B bench/opencode/tui.py --run
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B bench/opencode/tui.py --case same
```

Her terminal 180s, metadata komutları 30s/4MiB, GUI koşusu 900s sınırındadır. Title agent yalnız fixture'da kapalı; shell/read/write/network, nested/background ve auto/yolo kapalıdır. İzinler sabit MCP/veri/retrieval ve iki adlandırılmış test child'ıdır. Açıkça test edilmemiş başka hesap/model özelliği veya etkin config katmanı kapsanmaz.

**Sınır:** Gerçek CLI/worker/TUI kullanıldı; model sunucusu yerel ve sentetiktir. Byte azalması toplam token/cache/fatura veya bu geliştirme sohbetinin tasarrufu değildir. Canlı sağlayıcı/auth, gerçek model kalitesi/TTFT, V2/Desktop ve diğer OS açık. Hesap girişi, Actions, ücretli çağrı veya yayın yapılmadı; Linux/Cloudflare ertelemeleri korundu.
