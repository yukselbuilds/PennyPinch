# H043 — RTK-OSS yerel teknik değerlendirmesi

10 Kasım 2026. H043 tamamlandı. Windows 10 22H2 x64 üzerinde RTK-OSS 1.1.0 ile denenen sekiz komut yolunun hiçbiri H044 için kabul edilmedi. H044, çıktı bütünlüğünü koruyarak ölçülebilir fayda sağlayan bir aday bekliyor. [Ölçüm kaydı](rtk.json) · [İstemci yüzeyleri](rtk-surfaces.md)

Yalnız Windows x64 OSS ZIP’i proje içindeki `work/proofs/rtk/toolchain/1.1.0` dizinine alındı. GitHub etiketi `753a3b0e939f2e4490fc24e3a1984010f40a103c` commit’iyle eşleşti. Arşiv, sürüm kaydındaki ve yayımlanan checksum dosyasındaki SHA-256 ile eşleşti. Çalıştırılan exe arşivdeki exe ile aynıydı. Apache-2.0 OSS derlemesi RTK engine özelliğini dışarıda bırakır. Standart RTK paketi ve ayrı proxy bu değerlendirmenin dışındaydı. [Sürüm varlıkları](https://github.com/rtk-ai/rtk/releases/tag/v1.1.0) · [Derleme özellikleri](https://github.com/rtk-ai/rtk/blob/753a3b0e939f2e4490fc24e3a1984010f40a103c/Cargo.toml) · [Apache-2.0 lisansı](https://github.com/rtk-ai/rtk/blob/753a3b0e939f2e4490fc24e3a1984010f40a103c/LICENSE)

## Eşlenmiş kıyas

Üç yeni Headroom 0.41.0 sürecinde, her komut üç tur çalıştı. 72 eşlenmiş komut çıktısı 288 Responses HTTP/SSE isteğine dönüştü. Kontrol aynı olayın ham çıktısını, Headroom aynı çıktının motor üzerinden iletilen biçimini aldı. RTK kolu filtrelenen çıktıyı doğrudan gönderdi. Birleşik kol bu çıktıyı Headroom’dan geçirdi. H041’in son Responses kanıtında 592 girdi eşleşti. Komut çıktısı ve istek gövdesi farklı olduğundan kontrol ve Headroom burada yeniden ölçüldü.

Komut baytı, stdout ve stderr toplamıdır. RTK için bayt/4 tahmini ham satırlarda ayrı tutulur. Girdi tokenı, eklenen araç şemaları dahil tüm isteğin aynı `o200k_base` tokenizer ile sayımıdır. Kontrol her örnekte %0 referanstır. Birleşik oran aynı olayın kontrolüne göre hesaplandı. Negatif oran girdinin büyüdüğünü gösterir.

| Örnek | RTK komut baytı azalması | Headroom girdi azalması | RTK girdi azalması | Birleşik girdi azalması | Kabul dışı kalma nedeni |
| --- | ---: | ---: | ---: | ---: | --- |
| `git status` | %5,62 | %85,10 | %10,71 | %85,29 | %10 bayt eşiğinin altında, tam özgün çıktı geri alınamadı |
| `git diff` | %1,20 | %0 | −%0,35 | −%0,35 | Diff baytları değişti, tam özgün çıktı geri alınamadı |
| Başarılı test | %93,09 | %85,13 | %80,49 | %80,49 | Test sayısı korundu, ayrıntılı özgün çıktı için recall yok |
| Başarısız test | %89,61 | %83,15 | %77,84 | %77,84 | stderr 80 bayttan 0’a indi, recall iki akışı birleştirdi |
| Başarısız derleme | %96,33 | %88,12 | %87,91 | %87,91 | stderr ve çıkış kodu korundu, recall akışları ayrı döndürmedi |
| Kesin sayılı JSON | %97,02 | %0 | %91,26 | %91,26 | Filtre çıktısı geçerli JSON değildi, özgün içerik için recall yok |
| Kaynak okuma | %0 | %0 | %0 | %0 | Türkçe/Unicode ve CRLF kaynak baytları korundu, kazanç yok |
| Özel stderr ve çıkış 23 | %99,23 | %95,29 | %95,21 | %95,21 | stderr ve çıkış kodu korundu, recall akışları ayrı döndürmedi |

72 olayın hepsinde RTK çıkış kodu korundu. `rtk bypass` özgün komutu yeniden çalıştırdı ve bu deterministik örneklerde üç alanı da bire bir verdi. Bu yol daha önceki çalışmanın kaydını geri getirmez. Başarısız test, derleme ve çıkış 23 örneklerinde `rtk recall --full` 27 kez birleşik akışı verdi. Recall çağrısı özgün stdout ve stderr’i ayrı sunmadı. Başarısız testte anlık stderr alanı da kayboldu. Headroom’da 54 CCR geri alımı motorun aldığı içerikle tam eşleşti. Birleşik kolda bu içerik RTK’nın zaten filtrelediği çıktıdır. [RTK geri alma kodu](https://github.com/rtk-ai/rtk/blob/753a3b0e939f2e4490fc24e3a1984010f40a103c/src/core/retriever.rs) · [RTK komut yürütme kodu](https://github.com/rtk-ai/rtk/blob/753a3b0e939f2e4490fc24e3a1984010f40a103c/src/core/runner.rs)

Isınmış turlarda sekiz örnek içindeki en yüksek ek p95 süreleri komut için 87,14 ms, komut ve loopback isteği birlikte 95,55 ms oldu. Önceden belirlenen eşikler 250 ve 1.000 ms idi. İlk turlar ayrı kayıtlıdır. OS disk cache’i sıfırlanmadı. Sayaçlar ilk fixture yanıtını, toplam süreyi ve sahip olunan Headroom süreçlerinin CPU/belleğini kapsıyor. RTK süreç ağacının CPU/belleği, sağlayıcı cache’i, fatura ve model kalitesi ölçülmedi. Sağlayıcı çağrısı yapılmadı.

## Yüzey ve kabul

Windows PowerShell 5.1’de profili yüklemeden açık `rtk git status` çağrısı ve `rtk bypass` denendi. Codex ve Claude için gerçek RTK binary’sine sentetik `PreToolUse` girdileri verildi. OpenCode için sabit TypeScript plugin’inin erken geçiş koşulları Node’da çalıştırıldı. Bunlar otomatik istemci entegrasyonunu, Desktop’ı, alt ajanları veya diğer işletim sistemlerini doğrulamaz. Ayrıntılar [yüzey kanıtında](rtk-surfaces.md).

İlk koşu CCR alanını bütün araç çıktısıyla karşılaştıran doğrulama hatasıyla durdu. Kısa tanı koşusu alanı geri getirip zarfı eksiksiz kurdu. Son aday her CCR sonucunu doğru kapsamda kontrol etti ve iki motor kolunun oturum kimliklerini ayırdı. 661 kıyas girdisi ile 12 yüzey girdisi kendi koşularında eşleşti. Üç motor da temiz kapandı. Gerçek RTK kullanıcı dizinleri koşulardan önce ve sonra yoktu. Yalnız benchmark, izole fixture ve kanıt dosyaları değişti. Ürün kalite kapısı yeniden çalıştırılmadı. İstemci kataloguna eklenecek doğrulanmış bir RTK bağlantısı çıkmadı.

## Tekrar üretme

Sabit Python 3.13.13 x64 ve mevcut Headroom payload’ı gerekir. İlk komut yalnız sabit OSS paketini ve ilgili kaynakları proje içinde hazırlar. Windows’taki RTK kullanıcı ayar dizini için bağımsız override bulunmadığından, yerel koşucu Roaming veya Local AppData altında mevcut RTK dizini varsa durur. Telemetri kapalıdır. DB, recall ve geçici dosyalar koşu dizininde tutulur. [Windows dizin çözümlemesi](https://github.com/rtk-ai/rtk/blob/753a3b0e939f2e4490fc24e3a1984010f40a103c/src/core/user_dirs.rs)

```powershell
$rtkProofPython = 'work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe'
& $rtkProofPython -I -B bench/rtk/prepare.py --run
& $rtkProofPython -I -B bench/rtk/replay.py --self-check
$rtkProofRun = & $rtkProofPython -I -B bench/rtk/replay.py --run | ConvertFrom-Json
if (-not $rtkProofRun.passed) { throw 'RTK kıyası başarısız.' }
& $rtkProofPython -I -B bench/rtk/surfaces.py --benchmark-result (Join-Path $rtkProofRun.run 'result.json')
```

## RTK-OSS 1.2.0 — H044 ön elemesi

15 Kasım 2026. Resmî kararlı `v1.2.0` Windows x64 OSS ZIP'i, etiket commit'i ve iki yayımlanan SHA-256 kaydıyla doğrulandı. Proje içindeki izole çalışmada mevcut H043 komutları ve aynı bütünlük/asgari %10 bayt faydası ölçütleri kullanıldı. `retriever.rs` ve `runner.rs` dosyaları 1.1.0 ile bayt düzeyinde aynıydı. [Sürüm varlıkları](https://github.com/rtk-ai/rtk/releases/tag/v1.2.0) · [Sabit geri alma kaynağı](https://github.com/rtk-ai/rtk/blob/e288a18c04452fd26192f59132e5324a8bb6924d/src/core/retriever.rs).

Sekiz komutun tamamı elendi. Git status %5,62, git diff %1,20 bayt azalmasıyla eşiğin altında kaldı. Kaynak okuma baytları korudu ve %0 fayda verdi. Sayısal JSON filtrelenince geçerli JSON yapısı bozuldu. Başarılı testin özgün çıktısı geri alınamadı. Başarısız testte stderr 80 bayttan 0'a indi. Başarısız test, derleme ve özel stderr örneklerinde `recall --full` özgün akışları birleşik verdi. Sekiz bypass, özgün komutu yeniden çalıştırarak aynı çıkış kodunu ve ayrı akışları üretti. [Komut sonuçları ve fingerprint'ler](rtk-1.2.0-preflight.json).

Bu tek turluk komut ön elemesi uygun aday çıkmadığında durur. H043'ün dört kollu motor ve istemci kanıtı kendi sürümüyle kalır. Sonuç bayt ölçümüdür. Başlangıç ve bitişte 127 kaynak, fixture, araç ve ortam girdisi eşleşti. RTK veri dosyaları çalışma dizinindeydi. H044 kurulum ve ürün kabulü açık.
