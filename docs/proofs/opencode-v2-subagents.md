# OpenCode V2 alt ajan ve model rotası

15 Kasım 2026. Windows 10 22H2 x64, OpenCode 2.3.0 Windows x64 baseline ve Headroom 0.41.0 ile yerel doğrulama. [Kabul planı](../decisions/009-opencode-provider-routes.md#h010-i--v2-alt-ajan-ve-model-rotası), [programatik kayıt](opencode-v2-subagents.json), [inceleme](../reviews/H010-i.md).

Headroom Önizle ve Uygula, ayrı V2 profilindeki iki sağlayıcı rotasını ve paketli MCP köprüsünü yazdı. Bir normal OpenCode servisi başlatıldı ve iki `opencode run` turu bu servisi kullandı. Sağlayıcı, yalnız servise verilen sentetik anahtarı kabul etti. PID, loopback dinleyicisi ve servis kaydı iki tur boyunca eşleşti.

| Tur | Ana ajan | Gerçek alt ajan | Sonuç |
| --- | --- | --- | --- |
| Model mirası | `lab/slot`, Chat Completions | `lab/slot`, Chat Completions | Ajan model argümanı vermeden ana modeli miras aldı. |
| Açık model seçimi | `openai/slot`, Responses | `lab/child`, Chat Completions | `subagent` aracında seçilen farklı alt model ve protokol korundu. |

Her turda CLI'nin gerçek `subagent` araç kaydı, veritabanındaki ayrı alt oturumun kimliği ve `parent_id` ilişkisiyle eşleşti. Araç önce arka plan başlangıcını verdi. Ana ajana gelen tamamlanma bildirimi aynı kimliği ve alt ajanın son metnini taşıdı. Ana ajanların altışar, alt ajanların dörder model isteği, kayıtlı model/ajan seçimi, üçer MCP çağrısı ve son yanıt doğrulandı. [Sabit araç kaynağı](https://github.com/anomalyco/opencode/blob/a6b15d33daf6ab7d35569f6d711fdb630b9f04e7/packages/core/src/tool/plugin/subagent.ts), [tamamlanma kaynağı](https://github.com/anomalyco/opencode/blob/a6b15d33daf6ab7d35569f6d711fdb630b9f04e7/packages/core/src/session/subagent-completion.ts).

Dört rol/tur örneğinde log bloğu **14.999 → 103 bayt** oldu ve özgün içerik birebir geri çağrıldı. Yerel veri, gerçek token veya fatura tasarrufunu ölçmez. Model ve izin tanımları Uygula sonrasında aynı kaldı. Normal servis durdu ve kayıt/port/süreçler kapandı. Native Kaldır ve test motor ayarı özgün baytlarına döndü.

İlk denemede adaptör, `experimental` içeren profili önizlemede engelledi. Son aday OpenCode'un varsayılan tek seviye alt ajan derinliğini kullandı. İkinci denemede `skill` araç listesinde görünürken doğrulayıcı listeyi fazla kısıtladı. Nihai kabul çağrılan araçları doğruladı. `skill` çağrısı olmadı ve izinler korunarak iki koşu tamamlandı. İki önceki koşunun kayıtları ayrı, temizliği doğrulandı.

Nihai kayıt `work/proofs/opencode-v2-service/native-agent-0cc2093b4d/result.json`. 22.714 native girdi önce/sonra eşleşti. Aynı oturumda model değiştirme, nested alt ajan, V2 TUI/desktop içinden alt ajan, canlı hesap ve diğer işletim sistemleri ayrı kabul bekliyor.

Uyumluluk kataloğunun son adayında 222 Node ve 462 Rust testi geçti, 19 test ignored kaldı. Rustfmt, frontend derlemesi ve Tauri embedded derlemesi geçti. Son yerel kapının 344 girdisi önce/sonra eşleşti.
