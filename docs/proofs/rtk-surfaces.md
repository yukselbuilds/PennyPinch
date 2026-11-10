# RTK-OSS 1.1.0: istemci hook yüzeyleri

Tablo `work/proofs/rtk/toolchain/1.1.0/receipt.json` içinde sabitlenen `753a3b0e939f2e4490fc24e3a1984010f40a103c` kaynaklarına dayanır. Hook README, plugin ve Rust dosyalarının yerel kopyaları kaydedilen SHA-256 değerleriyle eşleşti. Alt bölümde Windows üzerinde çalıştırılan fixture’ların kapsamı bulunur.

| İstemci / önkoşul | Hook girdisi ve kapsamı | Yeniden yazma ve izin davranışı |
|---|---|---|
| **Codex CLI**. README bir Codex sürüm alt sınırı belirtmiyor. Hook, proje `.codex/hooks.json` veya `$CODEX_HOME/hooks.json` içine `rtk init --codex` ile eklenebilir. | `PreToolUse` olayı, `Bash`/`bash` aracı ve `tool_input.command`; izin kipleri yalnız `default`, `acceptEdits`, `plan`, `dontAsk`, `bypassPermissions`. Bash dışı araçlar, bilinmeyen kipler ve güvenle çözümlenemeyen shell yapıları olduğu gibi bırakılır. | `hookSpecificOutput.updatedInput` içinde mevcut `tool_input` korunup yalnız `command` değişir; `permissionDecision: "allow"` RTK yanıtında bulunur. Codex değiştirilmiş komut için kendi normal onay ve sandbox denetimlerini tekrar yapar. RTK diğer ajanların izin dosyalarını yorumlamaz. |
| **Claude Code**. Sabit shell hook’u Bash, `jq` ve RTK `0.23.0` veya üstünü ister. Ayrı `rtk hook claude` yolu native JSON işler. | Her iki yol `tool_input.command` alanını okur. Shell scripti `rtk rewrite` çağırır. Native yol girdide varsa `cwd` alanını da alır. | İzinli rewrite `updatedInput` ve `permissionDecision: "allow"` döndürür. Onay isteyen durumda `permissionDecision` alanı eklenmez. Eşleşme yoksa veya kural reddediyorsa orijinal komut geçer. Shell scripti eksik bağımlılıkta da rewrite vermez. |
| **OpenCode**. 1.x için en az `1.18.29` gerekir. README 2.0 plugin yolunu da listeler. RTK PATH’te bulunmalı ve `rtk hook opencode --help` yetenek kontrolünden geçmeli. | TypeScript plugin yalnız `bash` veya `shell` aracının `command` metnini ele alır. 2.x `execute.before` girdisinde isteğe bağlı `event.agent` alınır. 1.x `tool.execute.before` biçiminde ajan alanı yoktur. Windows’ta PATH araması `PATHEXT` kullanır. | Plugin komutu `execFile` argv’siyle `rtk hook opencode [--agent NAME] COMMAND` çağrısına verir. Rust tarafı yorumlayabildiği OpenCode kurallarıyla rewrite öncesi ve sonrası izin sonucunu karşılaştırır. Sonuç değişirse boş cevap verir. Geçerli rewrite v2 `event.input.command`, v1 `output.args.command` içine yazılır. Hata, timeout veya geçersiz yanıt orijinal komutu korur. |

## Windows 10 fixture sonuçları

| Yüzey | Gözlenen | Kanıt kapsamı |
| --- | --- | --- |
| PowerShell 5.1 | Ayrı, profilsiz shell’de `git status` 2.279 bayt, açık `rtk git status` 2.151 bayt çıktı verdi. `rtk bypass` stdout, stderr ve çıkış kodunu bire bir korudu. | Açık komut çağrısı. İstemci tarafından otomatik yakalama denenmedi. |
| Codex | RTK’nın native hook’u sentetik `Bash` girdisinde `rtk git status` döndürdü. `shell_command`, `Read`, `Grep` ve bilinmeyen izin kipi boş yanıt aldı. | RTK’nın girdi ayrımı. JSON cevabı istemciye verilmedi ve cevapla komut çalıştırılmadı. |
| Claude Code | Native RTK hook’u geçici Claude config diziniyle `Bash` girdisinde `rtk git status` döndürdü, `permissionDecision` alanı boş kaldı. `command` alanı taşımayan `Read` girdisi boş yanıt aldı. | Native RTK hook fixture’ı. Bash/jq scripti ve gerçek istemci oturumu denenmedi. |
| OpenCode | Gerçek RTK plugin fonksiyonu `read` ve `grep` için dönüşüm vermedi. `RTK_DISABLED=1` ile `bash` ve `shell` olduğu gibi geçti. | Node’da erken geçiş koşulları. Gerçek OpenCode yükleyicisi ve rewrite sonrası komut çalışması denenmedi. |

12 girdinin önce/sonra SHA-256 değerleri eşleşti. Bu dört fixture çalışması istemci ayarına yazmadı. Windows 11, macOS, Linux, Codex Desktop, gerçek OpenCode/Claude hook kurulumu ve alt ajan kapsamı `not_tested` olarak kaldı. [Makinece okunan kayıt](rtk.json)

## Kapsam ve sınırlar

- Bu üç entegrasyon shell komutunu çalıştırmadan önce yeniden yazar. Bunlar model sağlayıcısına giden API isteklerini gözleyen bir proxy yüzeyi değildir; bu kaynaklar tüm model trafiğini, token tasarrufunu veya her model/rotayı kanıtlamaz.
- Codex ve Claude kayıtları `Bash`/shell komutlarıyla sınırlıdır. Yerel `Read`, `Grep`, MCP veya başka native araçların çağrılarını kapsadıkları bu kaynaklarda gösterilmiyor. OpenCode plugin’i de yalnız `bash`/`shell` adlarını kabul ediyor.
- Windows için OpenCode plugin README’si Node `execFile`, Windows PATH/PATHEXT ve OpenCode Desktop/Electron uyumluluğunu beyan ediyor. Bu, Headroom içindeki Windows runtime veya gerçek Desktop kullanımının testi değildir. Codex hook kaydı Bash matcher’ıdır; bu kaynaklar PowerShell komutlarının yakalandığını göstermiyor. Claude hook’u Bash scriptidir; Windows/PowerShell çalışması belgelenmiyor. Dolayısıyla bu yüzeylerden Windows PowerShell kapsamı çıkarılamaz.
- Codex README’si hook’un diğer ajanların izin dosyalarını yeniden kullanmadığını açıkça söyler. OpenCode 2.x `agent` bilgisini yalnız ajan izin kurallarını değerlendirirken aktarır; 1.x’te bu alan yoktur. Bu ayrıntılar, tüm ana/alt ajan çağrılarının yakalandığını kanıtlamaz. Claude için de burada alt ajan kapsamı gösterilmemiştir.
- Hook’un kurulması veya komut eşleşmesi gerçek entegrasyon, rota, hesap doğrulaması ya da sağlayıcı ölçümü yerine geçmez. RTK proxy’si veya diğer araçlarla birlikte çalışma bu kaynak incelemesinde denenmemiştir.

## Kaynaklar ve fingerprint

Satır numaraları `work/proofs/rtk/toolchain/1.1.0/source/` altındadır.

- Codex: `hooks/codex/README.md:7-19`; `src/hooks/hook_cmd.rs:681-700,983-1025,1027-1066`; CLI hook komutu `src/lib.rs:1274-1303`.
- Claude: `hooks/claude/README.md:7-10`; `hooks/claude/rtk-rewrite.sh:1-4,16-27,46-60,62-105`; JSON işleme/yanıt `src/hooks/hook_cmd.rs:703-777,824-878`.
- OpenCode: `hooks/opencode/README.md:7-20`; `hooks/opencode/rtk.ts:137-172,177-225`; izin karşılaştırması/yanıt `src/hooks/hook_cmd.rs:1278-1335`; CLI komutu `src/lib.rs:1294-1303`.
- Receipt’teki commit: `753a3b0e939f2e4490fc24e3a1984010f40a103c`. İncelenen yedi dosyanın SHA-256’sı receipt kayıtlarıyla eşleşti: Codex README `c1f3d218…ff0489ca`, Claude README `424e28ef…ab4e82c`, Claude script `d4cd1e04…893267d`, OpenCode README `a289a50b…fb1c8b`, OpenCode plugin `989e9231…d4898aa4`, Rust hook `ce9965fa…2308ad7`, CLI definitions `c737ed90…f4e372`.
