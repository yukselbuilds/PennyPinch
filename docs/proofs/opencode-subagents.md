# H010-c — Native OpenCode alt ajan ve model değişimi

14 Kasım 2026. **Yerel dilim kabul edildi; ana H010 ve toplam 42/54 açık.** Windows 10 22H2 x64/build19045, gerçek OpenCode 1.19.3, Headroom 0.41.0. [Programatik kabul](opencode-subagents.json), [plan](../decisions/009-opencode-provider-routes.md#h010-c--yerel-alt-ajan-kabulü), [inceleme](../reviews/H010-c.md).

OpenCode sistem geneline kurulu kabul edilmedi. Proje içindeki resmî `work/toolchains/opencode/v1.19.3/runtime/opencode.exe` binary'sinin hash'i sabit sürümle eşleşti. CLI, gerçek task aracını ve gerçek MCP süreçlerini çalıştırdı; model yanıtlarını yalnız yerel, sentetik sağlayıcı verdi. Gerçek model çıkarımı veya fatura ölçülmedi.

## Ürün yolu ve sonuç

H010-b3'te kabul edilen aynı release executable/paket manifesti kullanıldı; ürün kaynağı yeniden derlenmedi veya değiştirilmedi. Native Başlat → OpenCode Önizle → Uygula iki uygun provider'ı ve ürün retrieval köprüsünü tek kez kaydetti. CLI kaydedilmiş kullanıcı JSONC'sini normal biçimde okudu; inline config veya endpoint override verilmedi. Test agent/model tanımları Apply öncesinde fixture dosyasındaydı ve Headroom tarafından değiştirilmedi.

| Senaryo | Parent / child selector | Gerçek kimlik kontrolü |
| --- | --- | --- |
| Aynı model | `lab/slot` / `lab/slot` | Yeni parent ve ayrı child; child modeli parent'tan miras aldı. |
| Aynı sohbette açık model değişimi | `lab/other` / `lab/other` | Önceki parent session ID'si korundu; farklı child açıldı, yeni modeli miras aldı; yeniden bağlanılmadı. |
| Farklı provider/protokol | `openai/slot` Responses / `lab/child` Chat | Yeni parent ve ayrı child; açık child modeli/agent'i korundu; iki kaydedilmiş rota kullanıldı. |

Üç CLI sıralı çalıştı; her biri tek foreground child başlattı. Toplam iki parent ve üç farklı child kimliği vardır. Task'ın `sessionId`, `parentSessionId`, model metadata'sı ile gerçek child export'unun `info.parentID` ve assistant agent/provider/model alanları eşleşti. Aynı model adı tek başına ajan kanıtı sayılmadı. Export stdout'u yalnız bellekte çözümlendi; rapora transcript veya HTTP gövdesi taşınmadı.

Her rol veri → CCR retrieval → tanık; parent ayrıca task sonucunu bekleme → final yolunu tamamladı. Toplam 27 upstream mesajı; altı log bloğunun her birinde 14.999→103 bayt ve özgün içerikle hash/bayt eşitliği doğrulandı. Chat gerçek ürün MCP retrieval çağırdı; Responses parent'ında retrieval motor tarafından server-side işlendi. Parent/child araçları completed, sentetik auth ve wire modeller doğru, CLI exit0/stderr0. Model/efor değiştiren ürün kuralı veya model allowlist'i eklenmedi; fixture selector/wire isimleri gerçek model desteği ilanı değildir. Reasoning seçimi bu senaryoda yapılmadı.

Her CLI boyunca kaydedilmiş config ve owner fingerprint'leri değişmedi. GUI normal onaylı çıkışla kapandı; aynı release'in mevcut exclusive gate'li `--headroom-uninstall keep` girişi bağlantıyı geri aldı. Exit0, byte-exact özgün JSONC, released owner, korunmuş state ve kapalı sahip listener doğrulandı; zorla kapatma fallback'i yoktu. Son uygulama ayarı fixture'ın kendi önceki baytlarına döndü. H010-b3'te UI Undo ayrıca doğrulanmıştı; bu dilimde arka planda açılmayan kaldırma popup'ı için kullanıcının ön plandaki uygulamasına fiziksel girdi gönderilmedi.

## Kanıt ve tekrar

Nihai kayıt `work/proofs/opencode-subagents/runs/5efdb40e1eea/result.json` passed:true. Dokuz odaklı test; yanlış parent/model/agent, kısmi retrieval, tamamlanmamış task, kullanıcı işareti ile tool prompt'unun karışması ve Responses kullanıcı/CCR biçim farkları dahil geçti. `focused-responses.json` ve `focused-responses-extra.json` kaynak/araç önce-sonra eşleşmesini taşır. Native input ve sonuç referanslarının hash'leri kanonik JSON'a programatik yazıldı.

Eski 332 ürün girdisi ve araç sürümleri aynı olduğundan 222 Node/448 Rust (18 ignored)/fmt/frontend/embedded build kanıtı yeniden kullanıldı. Yeni benchmark/fixture ayrı sınandı; eski ürün kapısının test etmediği yeni dosyalar eski PASS'e dahilmiş gibi sunulmadı. Pinned OpenCode task/config/agent/run/export kaynaklarının altı hash'i `work/research/h010-subagents/manifest.json` içinde; motorun gerçek paketli CCR response handler'ı da native fingerprint'e dahil.

Üç önceki başarısız deneme korunur: `e2d4db02d026` GUI açılmadan app state namespace kontrolünde durdu; `55d8ed1099e3` Responses `input_text` biçiminde; `6dcbb93aa03d` server-side CCR'nin ek `original_item_count` alanında durdu. Son ikisinde aynı model/model değişimi adımları geçti fakat genel koşu kabul edilmedi. Son fixture gerçek kaynak zarfını doğrular; içerik/hash eşitliği aynen zorunludur. Native koşular normal kapandı ve sahipli bağlantıları geri alındı; ilk native koşunun ayrı temizliği `cleanup.json` içinde kayıtlıdır.

Ön koşul: H010-b3'ün kabul edilen yerel QA release'i/paketi ve sabit OpenCode/Python hazır olmalı; aynı QA state'te aktif bağlantı bulunmamalı. Aşağıdaki opt-in komut kendi ayrı client state/workspace'ini açar. Native UI'da Başlat/Önizle/Uygula gerekir; koşu dizinindeki `run-all` tetikleyicisinden sonra üç senaryo çalışır. Sonunda GUI normal onayla kapatılır; benchmark aynı native Undo girişiyle temizler.

```powershell
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B -m unittest discover -s tests/fixtures/opencode -p test_subagent_fixture.py
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B bench/opencode/subagents.py --run
```

İlgili fingerprint değişmedikçe başarılı kontroller tekrarlanmaz. Bu benchmark otomatik CI veya ücretli çağrı başlatmaz. İzinler yalnız sabit veri MCP'si, retrieval ve adı belirtilmiş iki test child türüdür; shell/dosya/ağ araçları, nested/background ajan ve auto/yolo izin bayrağı kapalıdır.

## Tasarruf ve açık sınır

Bu koşunun byte azalması toplam oturum token/fatura kazancı değildir; geri alma özgün içeriği yeniden taşır. H041'in ayrı yerel `o200k_base`/tam canonical istek JSON'u kıyasında tekrarlı log için %98,15–98,23; kaynak/sayısal/kısa JSON için %0 ölçüldü. Bu, canlı sağlayıcı usage/cache veya günlük kullanım ortalaması değildir. [H041 ölçüm temeli](../validation/optimization-benefit.md), [yerel özet](optimization-local-acceptance.json).

RTK-OSS 1.1.0'ın sekiz örneği H043 koruma/geri alma/asgari fayda kapısını geçmedi; H044'te yönetilen etkinleştirme beklemede. RTK bu sohbet veya ürün trafiğine sessizce kurulmadı. [RTK sonuçları](rtk.md). Bu geliştirme sohbetinin Headroom rotası/tasarrufu doğrulanmadı; yüzde atanmaz.

Canlı sağlayıcı/auth ve gerçek model kalitesi/TTFT/fatura, TUI/V2/Desktop, temiz Windows/Windows 11 ve diğer OS açık. Başka kullanıcı/config katmanları ve tüm modeller için kapsama garantisi yoktur. Normal kullanım ölçümü ancak gerçekten bağlı oturumdan geçen, aynı temelde sayılabilen isteklerle oluşur; fatura ve cache ayrıca değerlendirilir. Actions/hesap girişi/yayın/ücretli çağrı yapılmadı; Linux/Cloudflare ertelemeleri korundu.
