# İstemci ön envanteri

19 Ekim 2026 — Salt okunur `Get-Command`, PE sürüm metadatası ve SHA-256 kontrolü. Bu belge H008–H010 gerçek trafik veya destek kanıtı değildir.

| Yüzey | Yerel bulgu | Sürüm / durum |
| --- | --- | --- |
| Codex CLI | `%LOCALAPPDATA%\OpenAI\Codex\bin\896a0166b774037a\codex.exe` bulundu. | `--version`: `codex-cli 0.163.0-alpha.10.2`; login-status mevcut girişi `chatgpt` olarak raporladı. Gerçek proxy yönlendirmesi H008'de ayrıca doğrulanacak. |
| Claude Code CLI | `%USERPROFILE%\.local\bin\claude.exe` bulundu. | `--version`: `2.1.158 (Claude Code)`; PE `2.1.158.0`; auth-status `loggedIn=true`, `authMethod=oauth_token`. Proxy/auth forwarding henüz doğrulanmadı. |
| OpenCode CLI/TUI | Mevcut PATH'te `opencode` bulunmadı. | Tam sistem yokluğu veya protokol uyumsuzluğu sonucu değildir; bilinen kurulum yolları H010/H016'da ele alınır. |

Binary SHA-256:

- Codex: `f6dbf48cb70f62649c645f26c87dc9bf84cc36986202dfad8e12fb5b8ca70312`.
- Claude: `10fa305545b20baf6e074a086762bd252b89dfe7035848a5d41385503b1a6c74`.

İstemci ayarı/auth dosyası değiştirilmedi veya içerikleri rapora kopyalanmadı. Bulunmuş olmak gerçek proxy trafiğini, aktif auth yöntemini veya ana/alt ajan kapsamını kanıtlamaz. Bu envanter tekrar kullanılabilir; rota doğrulama aşamasında değişen binary/config fingerprint'i eski uyumluluk kanıtını geçersiz kılar.

Salt okunur durum komutları: `codex login status` ve `claude auth status --json`, ikisi de exit=0. Çıktılar bellekte ayrıştırıldı; yalnız yukarıdaki auth türü/giriş durumu kaydedildi. Token, API anahtarı, e-posta veya organizasyon bilgisi rapora yazılmadı. Bu snapshot, mevcut auth türünü söyler; proxy üzerinden başarılı inference, hesap kotası veya girişin gelecekte geçerliliğini kanıtlamaz. Auth yöntemi/ortam değişirse canlı rota aşamasında yeniden doğrulanır.

## H008 için güncel resmî kaynak notları

19 Ekim 2026'da resmî OpenAI Docs sayfaları arandı, açıldı ve ilgili bölümleri okundu:

- Proje `.codex/config.toml` dosyasındaki provider/base URL anahtarları etkili yönlendirme yöntemi olarak kullanılamaz. Süreç veya desteklenen kullanıcı kapsamı gerçek istemcide ayrı doğrulanmalıdır. [Yapılandırma referansı](https://learn.chatgpt.com/docs/config-file/config-reference).
- Özel sağlayıcıda `requires_openai_auth = true`, mevcut OpenAI/ChatGPT girişini kullanabilen aday auth yoludur; bu durumda `env_key` dikkate alınmaz. Bu belge davranışı, yerel Headroom upstream uyumluluğunu kanıtlamaz; H008 gerçek rota/model/reasoning kontrolü yine gereklidir. [Authentication — Alternative model providers](https://learn.chatgpt.com/docs/auth#alternative-model-providers).

Yalnız kaynak araştırması yapıldı; bu ayarlar kullanıcı profilinde uygulanmadı. Mevcut seçili model/eforun veya alt ajan sayısının değiştirilmesi testin koşulu değildir.

Devam oturumunda aynı binary'nin `--version` ve `exec --help` salt okunur çıktısı alındı. Bu sürüm süreçlik `-c` override, `--ephemeral`, `--ignore-user-config` ve `--sandbox read-only` seçeneklerini sunuyor. `--ignore-user-config` yardımına göre auth yine `CODEX_HOME` kullanır. Bunlar izole deney için aday araçlardır; bütün kullanıcının ayarını devre dışı bırakmak ürün entegrasyonu olarak sunulmaz.

ChatGPT girişini taşıyan gateway, API-key `/v1` yoluyla eş tutulamaz: resmî belge `/models` ve `/responses` isteklerini `https://chatgpt.com/backend-api/codex` hedefine yönlendirmeyi ve mevcut auth başlıklarını korumayı tarif eder. Headroom'un bu route'u taşıdığı ayrıca sınanacak. [Sign in with ChatGPT through a gateway](https://learn.chatgpt.com/docs/enterprise/sign-in-with-chatgpt-through-a-gateway).

H008'de yalnız HTTP 200 yeterli değildir: artımlı SSE, tam tool-call → eşleşen sonuç → son yanıt döngüsü ve takip turu doğrulanır. Seçilen transport WebSocket/incremental continuation kullanıyorsa ilgili devam davranışı da ayrıca gerekir. [Gateway compatibility requirements](https://learn.chatgpt.com/docs/enterprise/gateway-compatibility).

Pinned Headroom 0.41.0 kaynak incelemesinde `providers/codex/runtime.py`, ChatGPT account başlığı/OAuth claim'ini routing hint olarak tanıyor; yetkilendirmeyi upstream'e bırakıyor. `proxy/handlers/openai.py` bu dalda sabit Codex backend hedefini seçiyor. Dolayısıyla `OPENAI_TARGET_API_URL` ayarlamak tek başına ChatGPT-auth deneyini fixture'a hapsetmez. H008 yerel ve canlı adımlarını bu davranışa göre ayrı kurmalı; fixture kontrolü gerçek hesap credential'ı kullanmamalıdır. Bu kaynak bulgusu başarılı canlı auth/rota kanıtı değildir.

Aynı pinned handler'ın `_should_buffer_openai_responses_stream_ccr` dalı, API-auth isteğinde `headroom_retrieve` aracı etkinse buffered JSON → SSE yolunu seçebilir; ChatGPT auth bu koşuldan muaftır. Ordinary araç döngüsündeki artımlı SSE başarısı, CCR etkin her API isteği için aynı gecikme davranışını kanıtlamaz. Gerçek retrieval döngüsü, transport ve latency kapsamı sonraki istemci/uyum testinde ayrı tutulur; core CCR'ı kapatarak bu fark gizlenmez.

## H009 için yerel CLI hazırlığı

Claude 2.1.158'in `--version` ve `--help` çıktıları salt okunur incelendi. Yerel API-fixture deneyi için `--bare`, `--print`, `--no-session-persistence`, `--output-format stream-json`, `--include-partial-messages`, `--setting-sources`, `--settings`, `--tools` ve `--strict-mcp-config` seçenekleri mevcut. Permission modları ayrı destekleniyor; mevcut kullanıcı ayarı değiştirilmez ve reddedilen Codex/PowerShell eylemi Claude üzerinden tekrarlanmaz.

`--bare` yardımına göre hooks/LSP/plugin sync/auto-memory/keychain ve CLAUDE.md keşfini atlar; Anthropic auth'u API key veya explicit apiKeyHelper ile sınırlar, OAuth/keychain okumaz. Bu yüzden bare+sentinel deneyi mevcut OAuth girişinin uyumluluk kanıtı değildir. Canlı OAuth yolu ayrı ve mevcut seçili model/efor korunarak sınanmalıdır. Burada yalnız CLI yardım metadata'sı toplandı; model çağrısı yapılmadı.

## H010 için resmî sürüm ve izolasyon notları

19 Ekim 2026'da [resmî giriş](https://opencode.ai/docs/) ile [V2 girişi](https://opencode.ai/v2/docs/) incelendi. V1/V2 aynı varsayılan komutu kullanıyor; otomatik bir global kurulum veya major yükseltme yapılmayacak. V2 sağlayıcı şeması V1 ile aynı kabul edilmeyecek. İlk H010-a adayı, proje içine alınabilecek sabit V1 CLI'dır; V2/desktop/TUI destekleri ayrıca açık kalır.

[Resmî GitHub v1.19.3 sürümü](https://github.com/anomalyco/opencode/releases/tag/v1.19.3), 16 Ekim 2026'da yayımlanmış, prerelease=false. GitHub release API'si Windows x64 ZIP için `62.204.138` bayt ve SHA-256 `e4ce9117f55fc068414513789f8c7d1049f855d7518bb4e350b9170a3a20d8c3` bildiriyor; baseline ZIP aynı boyutta, hash `0d59113484bfe8d6dc146fc981f5c33faa8318303c183711b7a9387060b289e8`. Asset adı `opencode-windows-x64.zip` / `opencode-windows-x64-baseline.zip`. Bu metadata indirme/çalışma başarısı değildir; indirilirse hash yerelde eşleşmeli ve gerçek exe sürümü kaydedilmelidir.

[V1 yapılandırma belgesi](https://opencode.ai/docs/config/) JSON/JSONC ve ayarların birleştirildiğini açıklıyor. `OPENCODE_CONFIG` tek başına kullanıcı/proje ayarlarını izole etmez; inline `OPENCODE_CONFIG_CONTENT` ayrı süreçlik override'dır. Yeni geçici HOME/XDG/workspace, izole auth/config ve gerçek binary'nin salt okunur help/debug çıktısıyla kapsam doğrulanmalı. İstemcinin kendi tool/MCP/agent çağrılarını kapatma ve loopback dışına model isteği göndermeme koşulları fixture deneyi için ayrıca gerekir. Burada kurulmuş OpenCode veya gerçek trafik PASS'i yok.

Pinned [v1.19.3 run.ts](https://github.com/anomalyco/opencode/blob/v1.19.3/packages/opencode/src/cli/cmd/run.ts#L710) kaynağında `--format json` yolu text event'ini `part.time.end` oluşunca yayımlıyor. Dolayısıyla H010 fixture'ının ilk tokenı CLI stdout'unda bekleyerek upstream'i kilitlemesi hatalı bir oracle olabilir. Tamamlanmış text part'ı ile terminal mesaj arasındaki gate ayrı sınanabilir; bu sonuç token bazında ilk delta gecikmesi diye sunulmaz. CLI JSON çıkışında event olmaması tek başına ağda incremental SSE olmadığı anlamına gelmez.
