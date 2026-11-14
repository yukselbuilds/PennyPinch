# OpenCode V2 Windows teknik ön kontrolü

14 Kasım 2026. **H010-e yerel kabul edildi; ana H010 açık, toplam 42/54.** [Programatik kabul](opencode-v2.json), [plan](../decisions/009-opencode-provider-routes.md#h010-e--v2-windows-ön-kontrolü), [inceleme](../reviews/H010-e.md).

Resmî [v2.3.0 yayını](https://github.com/anomalyco/opencode/releases/tag/v2.3.0) Windows x64 baseline ZIP'i yalnız `work/toolchains/opencode/v2.3.0` içine alındı. Release API digest'i, SHA256SUMS ve yerel archive hash'i eşleşti; executable ayrıca sabitlendi. `receipt.json` indirme/çıkarma anını kaydeder ve o anda `executed:false` der; sonraki gerçek yürütme ayrı runtime kanıtındadır. V1 kurulumu veya global PATH değişmedi. Kaynaklar `a6b15d33daf6ab7d35569f6d711fdb630b9f04e7` commit'ine bağlı 34 dar dosyadır; URL/hash'leri kanonik JSON'da bulunur.

## Gerçek yol ve sonuç

Windows 10 22H2 x64 build19045 üzerinde her protokol yeni HOME/XDG/AppData/ProgramData/temp kullanır. Çalışma dizini kendi global config köküdür. Pinned ConfigDiscovery bu durumda ancestor/proje aramasını atlar. CLI yalnız açık allowlist ortamını alır; caller auth/config/server-password aktarılmaz. V2 `providers/settings`, izin/policy ve agent tanımı kullanılır. Güncelleme, share, warming, snapshot, compaction, araçlar ve title agent kapalıdır.

Her profilde sürüm/help/debug paths ve `run --standalone --format json` gerçek binary ile çalıştı. Help private server bayrağını, debug paths on yolun tamamının fixture altında olduğunu gösterdi. `--standalone`, normal background service keşfi yerine stdin lease'i olan özel sunucu açar. Normal CLI/Desktop servisinin ayarları benimsemesi bu yolla sınanmış sayılmaz. [Bağlantı seçimi kaynağı](https://github.com/anomalyco/opencode/blob/a6b15d33daf6ab7d35569f6d711fdb630b9f04e7/packages/cli/src/services/server-connection.ts), [private server kaynağı](https://github.com/anomalyco/opencode/blob/a6b15d33daf6ab7d35569f6d711fdb630b9f04e7/packages/cli/src/services/standalone.ts).

| Kontrol | Chat | Responses |
| --- | --- | --- |
| Seçici → upstream model | lab/slot → headroom-opencode-tool-fixture | openai/slot → gpt-4.1 |
| Model isteği | 1 | 1 |
| Model ve sentetik Bearer | Korundu | Korundu |
| SSE final + terminal step | Exact final / 1 step | Exact final / 1 step |
| Dört CLI komutu | exit0 / stderr0 | exit0 / stderr0 |
| Her komut sonrası owned job | süreç0 | süreç0 |
| Config ve ortam | Aynı | Aynı |
| Motor kapanışı | graceful / exit0 / port kapalı / force=false | graceful / exit0 / port kapalı / force=false |

İlk gerçek model koşusu `f89faa722bae`, `provider.no-route` ile reddedildi ve başarısız tutuldu; motor normal kapandı. Teşhis özel profilde yapıldı. V2 policy yalnız `provider.use` iznine bakmaz: sağlayıcının kendi integration kaydı da kullanılabilir olmalıdır. Son config deny-all ardından yalnız seçili sağlayıcının `provider.use` ve `integration.use` izinlerini açar. Diğer entegrasyonlar ve araç izinleri kapalıdır. Bu test config'i kullanıcı ayarına yazılmaz. [Policy kaynağı](https://github.com/anomalyco/opencode/blob/a6b15d33daf6ab7d35569f6d711fdb630b9f04e7/packages/core/src/config/plugin/policy.ts).

## Doğrulama ve tekrar

Nihai kayıt `work/proofs/opencode-v2/runs/1dbf43fb31b2/result.json` passed:true. Altı test; bölünmüş text/final, eksik/çift terminal step ve hata olayı, yanlış rota/auth/model/stream/prompt, V2 deny-by-default config, profil dışı yol/caller env izolasyonunu ve job ataması hatasında owned CLI temizliğini kapsar. `focused-72dd3c285b.json` aynı son benchmark kaynağıyla geçti. Runtime'ın 645 girdisi, case config/ortamı ve 34 pinned kaynak eşleşir. Eski 332 ürün girdisi, araç sürümleri ve OS/Python eşleştiğinden 222 Node/448 Rust (18 ignored)/fmt/frontend/embedded build yeniden çalıştırılmadı. Yeni test bu eski PASS'in kapsamına dahil edilmedi. CodeGraph sync exit0/stderr0.

Proje kökünde, doğrulanmış V2 toolchain ve mevcut pinned engine payload hazırken:

```powershell
& .\work\proofs\packaging\artifacts\headroom-engine\runtime\python.exe -I -B -m unittest discover -s tests/fixtures/opencode -p test_v2.py
& .\work\proofs\packaging\artifacts\headroom-engine\runtime\python.exe -I -B .\bench\opencode\v2.py
```

Her tekrar yeni UUID profili/engine kullanır; önceki kayıt üzerine yazılmaz. CLI komutu120s/4MiB, HTTP4MiB, motor startup100s ile sınırlıdır. Owned job zorunlu kapanış fallback'idir; PASS, job kapatılmadan önce süreç0 ister. Token/usage sabit sentetik fixture sayılarıdır; gerçek kullanım, tasarruf, maliyet veya ilk-token süresi ölçülmedi. Kapalı loopback proxy ve offline motor profili yapılandırma kontrolleridir; firewall/packet capture kanıtı yoktur.

İlk olumlu `42d4a3f93ce4` runtime kaydı korunur. İncelemede job atama hatasında henüz job'a alınmamış kendi CLI handle'ı için kill/wait fallback'i ve negatif testi eklendi. Kaynak fingerprint'i değiştiğinden bu eski PASS son adayda yeniden kullanılmadı; yukarıdaki altı test ve iki protokol yeni adayla yeniden doğrulandı. Önceki kanonik kabul JSON'unun özgün baytları da proje proof kökünde korundu.

## Desktop ve açık kapsam

V2 Desktop ayrı `opencode-desktop-win-x64.exe` (267.406.912 bayt) artefaktıdır. Pinned Electron builder paketli `opencode-cli` kaynağını taşır; ana giriş sidecar probe'u başlatır. Bu installer indirilmedi veya çalıştırılmadı. CLI ön kontrolü Desktop'ın service/process/config adoption, auth veya gerçek GUI kabulü sayılmaz. [Paket kaynağı](https://github.com/anomalyco/opencode/blob/a6b15d33daf6ab7d35569f6d711fdb630b9f04e7/packages/desktop/electron-builder.config.ts).

Mevcut ürün V2 blocker'ı ve `not_tested` katalog kaydı korunur. Sonraki dilim keşif/trust, atomik Plan/Apply/Undo ve V2 `mcp.servers` retrieval sözleşmesidir. V2 MCP/CCR/alt ajan, normal servis/GUI, canlı provider/auth, Windows 11 ve diğer OS açık. Hesap, ücretli çağrı, Actions veya yayın yapılmadı; geliştirme sohbeti tasarrufu ölçülmedi.
