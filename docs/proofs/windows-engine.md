# H006-a — Portable Windows engine proof

Status: local proof PASS on the attached Windows 10 host, 2026-10-19. This covers a relocated portable runtime and hidden-helper lifecycle only. It is not a Windows 11, clean-machine, Tauri integration, or full H006 result.

Güncel takip: 14 Kasım H006-b, hazır durumdaki motor için gerçek Rust başlatma yolunda Job Object kaynaklı ani kapanış temizliğini ayrıca doğruladı. Tarihsel H006-a sonucu ve aşağıdaki hash'leri değişmez; yeni kapsam ve kaynaklar bu belgenin sonundadır. Ana H006 temiz Windows/Windows 11 nedeniyle açık kalır.

The package is a copyable directory, not a zip. The pinned standalone CPython 3.13.13 base is 61.1 MiB and the locked environment's `site-packages` is 438.0 MiB; the resulting payload is 22,225 files / 523,394,231 bytes (499.1 MiB), excluding the small manifest. Keeping native-extension files extracted avoids adding an archive extraction layer to this first proof. The manifest records the engine/Python pins, source hashes, dependency-record and installed-distribution counts, and payload size. The packager's reuse path rejects reparse points and stale paths; its post-copy inventory must exactly match the pinned Python tree, locked `site-packages`, runner, and manifest.

The integration proof copies the package to a separate path containing spaces and starts it with a controlled environment whose `PATH` contains only Windows `System32`. The runtime invokes its adjacent Python by absolute path and does not need a developer Python, uv, or Node entry on `PATH`. Temporary state, workspace, config, HOME, and TEMP paths are also under a separate path containing spaces. The engine environment binds only `127.0.0.1` on a dynamically selected port, enables local telemetry, disables anonymous beacon shipping, disables update checks, and does not log message contents.

The proof launches an outer controller with `CREATE_NO_WINDOW`; that process uses a typed `GetConsoleWindow` call and confirms it has no console. It starts the runtime manager as a native hidden-console helper with `CREATE_NEW_CONSOLE` and `STARTF_USESHOWWINDOW/SW_HIDE`, redirecting stdin and stdout through pipes. The helper verifies its console window is not visible and accepts only a bounded stdin JSON command. The engine shares that hidden console while running in its own `CREATE_NEW_PROCESS_GROUP`. The manager accepts readiness only when `/readyz` reports version `0.41.0` and the exact engine PID owns the loopback listener. `/stats` reports local telemetry enabled and anonymous shipping disabled. On `{"command":"stop"}`, the manager sends `CTRL_BREAK_EVENT` only to the owned engine process group. Python's documented Windows process-group event behavior is described in [`GenerateConsoleCtrlEvent`](https://learn.microsoft.com/en-us/windows/console/generateconsolectrlevent), [console process groups](https://learn.microsoft.com/en-us/windows/console/console-process-groups), and Python's [`SIGBREAK`](https://docs.python.org/3/library/signal.html#signal.SIGBREAK) handling. The observed result is exit code 0, Headroom's proxy summary and Uvicorn lifespan-shutdown markers, and a closed owned port; the forced termination fallback is not used on this PASS.

A console-less manager that tried to attach to the engine's separate hidden console did not provide a working shutdown path in the earlier focused attempt. The passing path keeps the outer parent console-less and gives only the helper a hidden console shared with the engine process group. This test uses a small Python `CREATE_NO_WINDOW` fixture as the GUI-like parent; the Tauri GUI's native process creation and stdio wiring remain untested. H006/H014 must verify that Tauri can start this helper with the tested hidden-window flags and preserve the stdin control channel. The Job Object is configured with `KILL_ON_JOB_CLOSE` as an orphan-prevention fallback, but this test does not simulate an abrupt manager crash and does not claim that fallback was exercised. Windows 11 and a clean machine remain untested.

## Reproduction

Run from `D:\Projeler\Headroom` in PowerShell:

```powershell
powershell.exe -NoLogo -NoProfile -NonInteractive -File scripts/package-engine.ps1 -ReuseExisting
& work\proofs\engine\toolchain\cpython-3.13.13-windows-x86_64-none\python.exe -I work\proofs\packaging\test_portable_runtime.py PortableRuntimeTests.test_manifest_describes_a_relocatable_pinned_runtime
& work\proofs\engine\toolchain\cpython-3.13.13-windows-x86_64-none\python.exe -I work\proofs\packaging\test_portable_runtime.py PortableRuntimeTests.test_relocated_runtime_is_ready_and_stops_through_a_consoleless_controller
```

The manifest check reports `Ran 1 test in 0.000s — OK`; the focused integration test reports `Ran 1 test in 33.669s — OK`. The package command reports 22,225 payload files, 499.1 MiB, and 102 lock records. The manifest and an independent read-only inventory check agree on 22,225 payload files and 523,394,231 bytes, with 0 missing and 0 unexpected paths. The integration test confirms the `CREATE_NO_WINDOW` parent has no console, the helper window is hidden, relocation works, readiness and listener ownership match, telemetry flags are correct, graceful exit is 0, and the owned port closes. No model/API request was made and no client configuration was changed.

## Fingerprints

- Host: Microsoft Windows 10 Pro 22H2, 10.0.19045, x64. Windows 11 and clean-machine behavior are unknown.
- Engine and Python: [`headroom-ai==0.41.0`](https://pypi.org/project/headroom-ai/0.41.0/), CPython 3.13.13 x64. The manifest counts 102 lock records and 96 installed `.dist-info` distributions. The payload includes the standalone base runtime and project-installed locked environment.
- Environment: helper `PATH` is only `%SystemRoot%\System32`; engine `PATH` is the copied runtime root plus `%SystemRoot%\System32`. `HEADROOM_HOST=127.0.0.1`, dynamically allocated port, `HEADROOM_TELEMETRY=on`, `HEADROOM_BEACON=off`, `HEADROOM_UPDATE_CHECK=off`, `HEADROOM_STATELESS=false`, and `HEADROOM_LOG_MESSAGES=off`.
- Command fingerprint: the three reproduction commands above; Python runs with `-I`, and test paths are copied under `work\proofs\packaging\runs` with spaces in the relocated runtime and state directories.

SHA-256 of the final proof inputs:

| File | SHA-256 |
| --- | --- |
| `work/proofs/engine/pyproject.toml` | `7d9802162b931a2c2b8ceb08852f44c587668290d1763eaec49786d089aeeaa9` |
| `work/proofs/engine/uv.lock` | `161d3b1543b1f6620c1ce40e38d9a2fba26d5c64e3e103a41dcd81dff18ced3f` |
| project CPython `python.exe` | `282c2635fbd49b4aeefda4a965d84a745508c87e6e5eb95947efd0b5a1c1fb07` |
| `scripts/package-engine.ps1` | `b154493968e0ce0f374521c74189582c036833b5a6b941fdbe5d2b0664bdd152` |
| `work/proofs/packaging/headroom_runtime.py` | `bf887b6f783f4385debc84836353340f39e3773433782305376a3247be0f4ac1` |
| `work/proofs/packaging/test_portable_runtime.py` | `68156d35bc3b5d3460dd56c4b1a7c4ef448e24bba5815febc4696b8400c9e623` |
| packaged `runtime-manifest.json` | `8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908` |

## H006-b — Ani sahip süreç kaybı (14 Kasım 2026)

Yerel Windows 10 build 19045 x64 kabulü geçti. Test, `ControllerSession::launch` üretim kodunu, güncel staged Windows paketini ve UUID ile ayrılmış state'i kullanır. Konsolsuz fixture sahibi `TerminateProcess` yoluyla zorla kapatılır; Rust `Drop`, stop isteği veya `TerminateJobObject` çağrısı kullanılmaz. Bu deney Tauri penceresinden başlatılmış bir GUI çökmesi değildir.

Windows'ta `KILL_ON_JOB_CLOSE`, son Job handle'ı kapanınca gruptaki süreçleri sonlandırır. [Microsoft Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects). Test gözlemcisi kontrol borusunun yazıcı handle'ını kendi sürecinde açık tutar; boru EOF'u ile normal kapanış, Job korumasının yerine geçemez. [DuplicateHandle sözleşmesi](https://learn.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-duplicatehandle). Üç çekirdek süreç için canlı, aynı Job'a ait process handle'ları; Job'ın inherit edilmediği ve fixture sahibini içermediği doğrulanır. Kimlikler yalnız test metadata'sındadır.

| Senaryo | Önceden belirlenen kabul | Gözlenen |
| --- | --- | --- |
| Tek Job sahibi zorla kapanır; kontrol borusu açık | Üç çekirdek süreç ve motor listener'ı en çok 5000ms'de kapanır | Süreçler 53ms; portun kapalı olduğu gözlem 357ms |
| Gözlemci ek Job handle'ını tutar | Sahip kapansa da üç süreç/port en az 1000ms yaşar | Süreç handle'ları canlı, readiness ve port sahibi aynı |
| Gözlemci son Job handle'ını kapatır; kontrol borusu hâlâ açık | Üç süreç ve listener en çok 5000ms'de kapanır | Süreçler 54ms; port gözlemi 370ms |
| Bağımsız test listener'ı | İki kolda da korunur | TCP bağlantısı ve 13 baytlık içerik eşitliği geçti |

Süreler ilgili tetik çağrısının hemen öncesinden, gözlem sonuna kadar ölçülür; kernelin gerçek sonlanma anı veya ilk boş port anı değildir. Her kol bir kez çalıştı; uzun süreli kararlılık, hız/kalite veya token tasarrufu benchmark'ı değildir. Yalnız üç çekirdek süreç handle'ı gözlendi; bütün işletim sistemi süreçlerinin sayımı yapılmadı.

Paket **22.333 dosya / 536.626.939 bayt**. Paketleyicinin tam içerik doğrulaması, aynı paket girdileri/komut/araçlarla başlangıç kanıtı olarak yeniden kullanıldı. Test sonrası bütün paket dosyaları bağımsız hash'lendi; envanter ve kaynak/araç/ortam önce-sonra eşit. Son adayda opt-in test, 441 Rust testi / 18 ignored, fmt ve embedded build geçti. Önceki 221 Node ve frontend build kanıtı, ilgili girdiler ve Node/pnpm sürümleri eşleşerek yeniden kullanıldı. [Makine tarafından üretilen kabul](windows-engine-crash.json), [inceleme](../reviews/H006-b.md).

İlk deneme eski `work/build/windows/resources` helper pin'i uyuşmadığı için reddedildi. Eski paket korunarak ayrı güncel stage üretildi. İkinci deneme testin kendi Job mutex'ini yeniden alması nedeniyle zaman aşımına uğradı; testte liveness kontrolü kilitten önceye taşındı. Bu iki koşu başarısızdır; nihai geçer kanıt `verification-6fdcd6e65d`, güncel yerel kapı `quality-28623d163c` altındadır. Kayıtlar `work/proofs/windows-engine-crash/` içindedir; ham hata dökümü saklanmadı.

### Tekrarlama

Pinned payload ön koşulları için [Windows paketleme kaydı](../validation/windows-install.md) geçerlidir. Başarılı kanıtı yalnız ilgili girdiler değiştiğinde yenile. Paketleyici eski hedef için `existing_stage_conflict` bildirirse eski artefaktı proje içinde koruyup boş hedefte güncel paketi üret; pin/guard gevşetilmez.

```powershell
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B scripts/package-windows.py --output work/build/windows/h006-crash-resources
$env:CARGO_HOME = 'D:\Projeler\Headroom\work\caches\cargo-home'
$env:CARGO_TARGET_DIR = 'D:\Projeler\Headroom\work\caches\cargo-target'
cargo +stable-x86_64-pc-windows-msvc test --locked --offline --manifest-path src-tauri/Cargo.toml runtime::manager::windows_crash_tests::packaged_controller_job_closes_after_owner_crash -- --exact --ignored --test-threads=1
```

Ana H006 açık: temiz Windows 10/Windows 11, başlangıçta Job ataması öncesindeki çökme yarışı ve gerçek GUI çökmesi bu dilimle doğrulanmaz. Canlı sağlayıcı veya istemci ayarı değiştirilmedi; yayın ve Actions çalıştırılmadı.

## H006-c — Gerçek paketli GUI kaybı (15 Kasım 2026)

Yerel alt dilim kabul edildi. Hazır Tauri release GUI'sinin zorla kaybında üç motor process handle'ı 57,429ms, motor/sabit ingress portları 58,299ms içinde kapandı. Dış gözlemci Job'ı bu ölçüm boyunca açıktı. Aynı test kimliği/profili Stopped açıldı, endpoint korundu ve Başlat/Ready/Durdur/normal Çıkış geçti. Yabancı listener korundu, model isteği 0. Bu, H006-b'nin controller fixture'ından ayrı gerçek GUI kanıtıdır. [Kapsam ve tekrar üretim](windows-gui-crash.md), [programatik sonuç](windows-gui-crash.json), [inceleme](../reviews/H006-c.md).

H006 ana kutusu açık kalır. Temiz Windows 10, ayrı Windows 11, Job ataması öncesi başlangıç yarışı ve aktif stream/CCR verisi bu yeni deneyle doğrulanmadı. Toplam 45/54 değişmez.

## H006-d — Job atamasından önce sahip kaybı (15 Kasım 2026)

Windows 10 22H2 x64 build 19045 üzerinde gerçek paketli controller'ın başlangıç davranışı doğrulandı. Test, üretim başlatma işlevini `spawn` döndükten sonra ve henüz oluşturulan üretim Job'ına atama yapılmadan önce bekletti. Bu noktada `start` iletisi henüz gönderilmemişti. [Kabul koşulları](../decisions/002-runtime.md#h006-d--job-atamasından-önce-sahip-kaybı).

| Senaryo | Gözlem |
| --- | --- |
| Sahip zorla kapatıldı, borunun yazma ucu kapandı | Controller 147 ms içinde exit 2 ile sonlandı. |
| Gözlemci yazma ucunu tuttu | Sahip kaybından sonra controller bir saniyelik gözlemde canlıydı. Yazma ucu kapatıldıktan sonra 8 ms içinde exit 2 ile sonlandı. |
| Süreç sahipliği ve protokol | Dış gözlemci Job'ı açık kaldı. Başlangıçtaki test sahibi, controller ve iki `conhost.exe` dışında süreç oluşmadı. İki kolda son canlı süreç ve controller stdout bayt sayısı 0. |

İlk ön kontrol geçmiş stage'deki `windows-hooks.nsh` pinini reddetti. H006-c'nin güncel stage'iyle devam edildi. İlk native test, Windows'un iki `conhost.exe` sürecini sayımda ayırt edemedi. Süreç türleri ayrı gözlendi. Nihai test başlangıç üyelerini tanıyor ve kümülatif Job sayacındaki her artışı hata kabul ediyor. Beş saniyelik kapanış sınırı korundu. Başarısız kayıtların referansları kanonik JSON'dadır.

Paket kaydı dahil 22.334 dosya, kaynaklar, doğrulama komutları ve araç/OS fingerprint'leri önce/sonra eşleşti. Son adayda 462 Rust testi, biçim ve embedded build geçti. 20 opt-in test normal koşuda atlandı. H006-d ayrı opt-in koşuda geçti. Aynı Node/pnpm ile değişmeyen frontend girdilerinin 222 uygulama testi ve frontend derlemesi yeniden kullanıldı. [Programatik kanıt](windows-engine-startup-crash.json) · [İnceleme](../reviews/H006-d.md).

Doğrulama için mevcut `work/build/windows/h010-retrieval-resources` stage'i ve proje içi Cargo cache kullanılır. Komut [kanıt kaydında](windows-engine-startup-crash.json) yer alır. Bu yerel test duraklatılmış bir spawn sonrası kontrol noktasını kapsar. Ayrı temiz Windows 10/11 x64 ortamları ana H006 için bekliyor.

## Temiz Windows ortamı için taşınabilir test paketi

`work/proofs/windows-engine-crash/clean-test-a8e23e494d/Headroom_0.1.0_Windows_x64_H006_test.zip` daha önce gerçek pencere ve motorla sınanan H006-c adayını, aynı motor dosyalarını, lisans bildirimini ve kısa test yönergesini içerir. 197.348.721 baytlık arşivin 22.335 dosyası tam SHA-256 karşılaştırmasından geçti. Windows'un `tar.exe` aracı girişleri okudu ve PowerShell arşiv hash'ini doğruladı. [Paket ve hash kaydı](windows-engine-clean-test-package.json).

Test eden kişi ayrı temiz Windows 10 22H2 x64 veya Windows 11 x64 bilgisayarda ZIP'i kısa bir klasöre çıkarır ve `headroom-desktop.exe` dosyasını açar. Arşivdeki `README.txt` açılış, motor hazır, durdurma ve normal çıkış adımlarını listeler. Ön koşul Microsoft Edge WebView2 Runtime'dır. Sonuçla birlikte Windows yapı numarası ve mimarisi kaydedilir. Arşiv SHA-256 değeri, aynı dizindeki `SHA256SUMS.txt` ile karşılaştırılır.

```powershell
Get-FileHash -Algorithm SHA256 -LiteralPath .\Headroom_0.1.0_Windows_x64_H006_test.zip
```

Paket, H006-c'deki byte-eşleşen imzasız ve ayrı kimlikli executable'ı kullanır. Bağımsız temiz sistem kabulü, arşivdeki motor testinin iki hedef işletim sisteminde yapılmasını bekliyor. NSIS ve eksik WebView2 giderme deneyi H037 kapsamındadır.
