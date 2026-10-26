# Proje içi CodeGraph

25 Ekim2026. CodeGraph bu checkout için geliştirme aracıdır; Headroom Desktop ürününe/paketine yeni runtime bağımlılığı eklenmedi. Ana görev sırası ve18/51 sayacı değişmedi; H022 başlamadı.

## Kurulum ve sınır

- Resmî [colbymchenry/codegraph v1.6.2](https://github.com/colbymchenry/codegraph/releases/tag/v1.6.2) Windows x64 paketi: `codegraph-win32-x64.zip`,54,260,850 byte. Yayıncı `SHA256SUMS` ile eşleşen SHA256: `7455e817ee8032666867a1badeb799b8a884c5f4fea9b444e088a02069b6deea`.
- Runtime yalnız `work/toolchains/codegraph/1.6.2/codegraph-win32-x64` altında. Paketin kendi Node v24.16.0 ortamı kullanılır. Sistem Node/PATH/npm ayarları değiştirilmedi. Paket dışarıdan eksikse launcher otomatik indirme veya global kurulum yapmaz.
- `scripts/codegraph.ps1` sürümü sabit runtime'a gider, reparse/path kontrolü yapar ve çalışma dizinini bu repo köküne bağlar. Vendor Node bayraklarını koruyarak bundled node.exe ve JS entry noktasını doğrudan çalıştırır; sorgular cmd.exe üzerinden yorumlanmaz. Argümanları/çıkış kodunu aktarır; önceki dizin ve dört ortam değeri finally ile geri döner. Başka projeye ait state sahiplenilmez.
- İlk çalıştırmadan itibaren süreç bazında `DO_NOT_TRACK=1`, `CODEGRAPH_TELEMETRY=0`, `CODEGRAPH_NO_UPDATE_CHECK=1`, `CODEGRAPH_DIR=.codegraph` uygulanır. Global kullanıcı tercih dosyası yazılmaz. Bu opt-out, sabitlenen upstream kaynak üzerinden doğrulandı; paket indirmenin kendisi normal HTTPS erişimidir. Packet-level ağ izolasyonu iddiası yoktur.
- `codegraph.json` resmî exclude formatını kullanır. Kaynak/test/script kapsamı korunur; work, caches/toolchains/proofs/app-state, docs, .codex/.codegraph, bağımlılıklar, dist/target, kilit/binary ve .env dosyaları hariçtir. `.codegraph/` DB/cache ve makineye özel `.codex/config.toml` Git'e girmez. Gerçek auth/client config veya global projeler okunarak indekse kopyalanmaz.

## Günlük kullanım

26 Ekim güncellemesi: `.agents/` dışlaması eklendi; sync/status/files kapsam kontrolünde 76 kaynak girdisi ve sıfır skill klasörü girdisi görüldü. Bu sohbetin gerçek CodeGraph MCP aracı da sembol/kaynak sorgusuyla doğrulandı. Tek dosyalık bu sorgu 15.331 karakter döndürdü; dosya sınırı yanıtın kısa olacağı garantisi değildir. Günlük araç seçimi ve kabul düzeni [WORKFLOW.md](WORKFLOW.md), güncel kanıt [DEV001](../proofs/dev-tools.md) içindedir.

PowerShell, bu repo kökünde:

```powershell
.\scripts\codegraph.ps1 status
.\scripts\codegraph.ps1 query --json --limit 5 undo_owned_endpoint
.\scripts\codegraph.ps1 query --json --limit 5 createConnectionActionClient
.\scripts\codegraph.ps1 query --json --limit 5 apply_connection_plan
```

`status` durum/istatistik gösterir. Kaynaklar değişince `sync` ile indeksi yenile. `init` ilk kurulumu ve indekslemeyi yapar; `index` tam yeniden indeksleme içindir. Bu sürümde `build` adlı bağımsız komut yoktur. Ek seçenekler için aynı launcher ile `--help` veya komutun `--help` çıktısını kullan; eski README sürümlerinden komut/config varsayma. Geçerli küçük sorgu sonucunu kaynak değişmedikçe gereksiz yere tekrarlama.

Önce soruya göre araç seç: bilinen küçük dosya/satır için `rg`/doğrudan okuma; çağıranlar, bağımlılıklar, etki veya bilinmeyen sembol için dar graph sorgusu. Büyük context/source dökümleri varsayılan olmasın. Graf yanlış/eski/bozuksa kaynak ve `rg` ile ilerle. AGENTS.md, ana ajan ve tek çalışan için aynı kısa kuralları içerir; belirli ajan/model adına bağımlılık yoktur.

## Codex MCP

Yalnız bu trusted checkout'ın yerel `.codex/config.toml` dosyası sunucu kaydını taşır. Komut `powershell.exe -NoLogo -NoProfile -NonInteractive -File <bu-checkout>/scripts/codegraph.ps1 serve --mcp --path <bu-checkout>`, çalışma dizini bu repodur. Global Codex/MCP/model/reasoning/agent ayarları değişmedi. Makineye özel yol içeren kayıt ignored'dır; başka clone ayrıca kurulum gerektirir.

MCP ortamında ayrıca `CODEGRAPH_NO_DAEMON=1` ve `CODEGRAPH_MCP_TOOLS=codegraph_explore` uygulanır. Sabitlenen sürümün varsayılan paylaşımlı servis modu, istemci kapandıktan sonra normalde 300 saniye bekleyebilir. Burada doğrudan mod seçildi; test istemcisi kapanınca motorun da kapandığı doğrulandı. `--path` varsayılan projeyi belirler; dosya sistemi sandbox'ı değildir. Araç şemasındaki isteğe bağlı `projectPath` başka dizin gösterebilir. Bu kurulumda varsayılan bu repo kullanılır; başka projeleri taramak için yetki vermez.

Yeni bir bu-proje sohbetinde veya proje MCP yeniden yüklendiğinde `codegraph_explore` aracı kullanılabilir. Bu mevcut sohbetin araç listesinde görünmezse CLI hazırdır; diğer sohbetleri/uygulamayı durdurmak gerekmez. Tek MCP aracı açıkça izin listesine alındı; ek araç şemaları açılmadı. Sorguda `maxFiles` değerini genellikle 1–3 tut, gereken hedefi belirt. Örnek gerçek kabul çağrısı:

```json
{"name":"codegraph_explore","arguments":{"query":"undo_owned_endpoint callers","maxFiles":2}}
```

Bu2-dosyalık çağrı bile17,291 karakter döndürdü. Bu sebeple basit sembol aramada küçük CLI JSON sorgusu daha uygun olabilir. Bu bir token tasarruf benchmarkı değildir; kullanım maliyeti/gerçek token ölçümü olmadan yüzde raporlanmaz.

## Doğrulama

- Yayıncı checksum eşleşmesi ve bundled runtime sürümü doğrulandı. İlk `init` launcher üzerinden exit0, yaklaşık10.8 saniye.
- `status`:76 indexed files,3,539 nodes,11,456 edges; yaklaşık14.41 MB. Dosya listesi incelemesi work/docs/.codex/.codegraph, kilit ve binary kayıtlarının bulunmadığını doğruladı.
- Rust `undo_owned_endpoint` → `src-tauri/src/integrations/transaction.rs`; TS `createConnectionActionClient` → `src/lib/connectionActions.ts`; native `apply_connection_plan` sorguları ilgili sembolleri buldu. CLI ham sorgu çıktıları tutulmadı.
- Gerçek proje MCP stdio initialize→tools/list→tools/call PASS: server 1.6.2, protocol 2024-11-05, tek `codegraph_explore`; hedef sembol bulundu. JSON dışı veya aşırı büyük stdout satırı yok; stderr yalnız byte sayısı olarak kaydedildi (172 byte). Exit 0; istemci kapanınca bu kuruluma ait Node süreç sayısı 0. Ham kaynak/araç çıktısı saklanmadı, yalnız metadata.
- Windows PowerShell 5.1'de önceki dört ortam değeri ve farklı çalışma dizininin geri yüklenmesi geçti. PowerShell 7.6.5'te boş değer ile tanımsız değişken ayrımı da geçti. Son launcher fingerprint'i kurulum kanıtında kayıtlıdır.
- Güncel kurulu Codex CLI `mcp get codegraph --json` bu proje sunucusunu enabled ve doğru cwd/launcher ile okudu. Önceki kayıtlı CLI yolu uygulama güncellemesiyle kalkmıştı; güncel kurulu yol kullanıldı. Kimlik doğrulama/model/provider çağrısı yapılmadı.
- Ürün kaynakları, bağımlılıkları ve derleme ayarları değişmediği için Headroom'un tam ürün testleri tekrar çalıştırılmadı. Kurulum/CLI/MCP odaklı kontroller uygulandı. Ana ve çalışan erişimi aynı launcher/indeks üzerinden sağlanır; model özelinde koşul yoktur.

Kanıtlar `work/proofs/codegraph-setup/` altında: `installation-v1.6.2.json` ve son launcher inceleme metadata'sı, `project-config.json`, `codex-registration.json`, son doğrudan mod için `direct-mcp-discovery.json`, `direct-mcp-acceptance.json` ve toplu `acceptance.json`. Ham hata/HTTP/araç/kaynak dökümü yoktur. Önceki paylaşımlı mod denemelerinin yalnız bu teste ait kalan süreçleri kimlik doğrulamasıyla kapatıldı. Son doğrudan mod denemesi zorla kapatma gerektirmedi; kalıcı watcher/web sunucusu kurulmadı.

Graph bir keşif aracıdır. Dinamik Tauri invoke→Rust handler köprüleri, gerçek istemci/sağlayıcı akışı, subagent attribution veya çalışma zamanı/test başarısı kendi başına bu graf üzerinden doğrulanmış sayılmaz. Resmî metinlerdeki tasarruf oranları bu depoda ölçülmüş kazanç değildir. Native/protokol ve mevcut kanıt disiplini geçerlidir.
