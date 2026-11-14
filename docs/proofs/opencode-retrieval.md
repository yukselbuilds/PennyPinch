# H010-b3 — OpenCode ürün retrieval kabulü

14 Kasım 2026. **Windows 10 22H2 x64, build 19045; OpenCode 1.19.3; Headroom motor 0.41.0.** Bu yerel ürün dilimi kabul edildi; ana H010 ve toplam 42/54 değişmedi. [Programatik özet](opencode-retrieval.json), [inceleme](../reviews/H010-b3.md).

## Gerçek ürün yolu

Native release penceresinde OpenCode Önizle → Uygula, kullanıcı JSONC'sindeki uygun iki sağlayıcı endpoint'i ile `mcp.headroom` tanımını mevcut atomik writer/ownership işlemiyle kaydetti. CLI, bu dosyayı normal kullanıcı ayarı olarak okudu; `OPENCODE_CONFIG_CONTENT`/özel endpoint override veya test retrieval bridge bayrağı verilmedi. Auth/HOME/XDG/workspace test için ayrıdır; gerçek hesap ve kullanıcı sohbetleri okunmadı. Veri MCP aracı yalnız sabit sentetik log/tanık döndürür. İlgili iki MCP aracı açıkça izinlidir; yerleşik shell/görev araçları bu kabulde açılmadı.

| Senaryo | Gözlenen |
| --- | --- |
| Preview ve kapatma | Config ve ownership byte/hash düzeyinde aynı; henüz bağlantı yazılmadı. |
| Chat Completions, `@ai-sdk/openai-compatible` | Veri → gerçek `headroom_retrieve` MCP çağrısı → tanık; üç araç completed, dört upstream mesajı, streaming korundu. |
| Responses, `@ai-sdk/openai` | Veri ve tanık completed; dört upstream mesajında motor server-side retrieval kullandı, upstream buffered kaldı. |
| İki protokolün içeriği | Özgün 14.999 bayt blok 103 bayta indi; geri alınan içerik aynı bayt/SHA'da, model/sentetik auth/call_id ve final doğru; CLI exit0/stderr0. |
| Duraklatma / tam durdurma | Duraklatılmış motorda mevcut CCR içeriği birebir alındı; tam durdurmada köprü sabit hata döndürdü. |
| Normal kapanış ve yeniden açılış | Yeni GUI PID'si, aynı endpoint kimliği/port/config/owner; süresi dolmamış CCR içeriği yeniden birebir alındı. Her iki GUI exit0, force fallback yok. |
| Undo iptali / çatışması | İptal yazmadı. Sahipli MCP timeout'u değiştirildiğinde `config_conflict`; dosya ve owner değişmedi. Yalnız testin kendi değişikliği geri alındı. |
| Normal Undo | Özgün yorumlu JSONC byte-exact geri geldi; owner released ve sahipli listener kapalı. |
| Farklı volume sınırı | Mevcut atomik writer `unsupported_filesystem` döndürdü; config/owner değişmedi. Başarılı akış aynı volume'da koşuldu. |

Model selector'leri Chat'te `lab/slot`, Responses'ta `openai/slot`; wire isimleri ayrı fixture girdileridir. Model adı filtresi veya otomatik reasoning/model değişimi yoktur. Reasoning seçimi sınanmadı. Tek blok byte azalması toplam token/cache/fatura tasarrufu, model kalitesi veya uzun süreli kararlılık ölçümü değildir; CCR özgün içeriği yeniden taşır.

## Köprü ve kaldırma sınırı

Paketli, hash'i native tarafta doğrulanan stdlib Python köprüsü OpenCode tarafından stdio ile başlatılır. Renderer komut/path/port vermez. Araç yalnız 12–24 hex karakterli hash alır; URL/path/shell/auth girdisi kabul etmez. Sınırlı endpoint kaydı, sabit endpoint UUID'si ve gerçek kurulmuş soketin PID/image/same-user SID sahipliği HTTP uygulama baytlarından önce doğrulanır. Sabit loopback `/v1/retrieve`, proxy ortamından bağımsız soket, 64KiB MCP giriş, 8MiB yanıt ve 10 saniye toplam deadline sınırı vardır. Eksik/kesilmiş içerik başarı sayılmaz; stdout MCP dışı log/secret taşımaz.

Release köprüsü mevcut `.desktop-use` kilidini tutar. Gerçek release `--headroom-uninstall keep` girişi ayrıca çalıştırıldı: açık köprü sırasında exclusive gate Windows sharing violation32 ile engellendi; stdin kapanınca köprü exit0/stderr0 ve kilit serbest kaldı. Kullanıcının MCP değişikliğini temsil eden çatışmada kaldırma exit10/config+owner aynı; normal durumda exit0/byte-exact JSONC/released owner ve korunmuş state geçti. Tam NSIS kurulum-kaldırma, updater dağıtımı ve üretim yayını bu dar giriş testiyle yeniden kabul edilmiş sayılmaz.

## Doğrulama ve kaynak eşleşmesi

Son adayda **222 Node, 448 Rust (18 ignored), fmt, TypeScript/frontend ve embedded custom-protocol build** başarılı. Yedi Python köprü testi gerçek Windows socket sahipliği/gate, EOF, sınırlar, hata ve Unicode'u kapsar; ayrıca yedi Rust retrieval ve on Node plan testi geçti. Son kapı ve native koşuların 332 kaynak/test/config girdisi, bağımlılık/araç fingerprint'leri önce-sonra aynı. Hash'ler elle taşınmadı; kanonik JSON kaynak, executable, paket manifesti ve sonuç dosyalarını programatik bağlar.

- Tam yerel kapı: `work/proofs/opencode-retrieval/final-74f47538b0.json`.
- Odaklı doğrulama: `work/proofs/opencode-retrieval/focused-7d784ff844.json`.
- Gerçek GUI/CLI: `work/proofs/opencode-retrieval/native/ce8ea443ed/same-volume/result.json`.
- Release kaldırma girişi: aynı adayın `removal/result.json` dosyası.
- Preview iptali ve volume reddi: aynı adayın `rerun/result.json` dosyasında ayrı hash kontrolleri; bu genel koşu başarılı sayılmadı.

Staging 22.325 payload dosyası/533.082.296 baytı doğruladı; köprü ve ek kaynaklarla 22.334 resource vardır. Native release executable ve immutable staged kaynakları aynı manifestten doğrulandı. Test profili, uygulamaya ait ayrı state kökü altında tutuldu. Cache/coding profili ve CCR TTL7200; yerel ölçüm açık, beacon/router/output-shaper/memory/learn kapalıdır. Sentetik testte protected-tools listesi boş; kaynak/sayısal korumanın tüm gerçek araçlarla kabulü bu koşudan türetilmez.

İlk koşularda fixture MCP receipt sınırı nedeniyle eksik araç bildirimi, release state ortamı nedeniyle erken exit12 ve farklı volume yazım reddi görüldü; bu başarısız kayıtlar silinmedi veya PASS'e çevrilmedi. Önceki yedi argümanlı test bağlantısı kendi doğrulanmış eski binary'sinin native Undo'suyla temizlendi: `work/proofs/opencode-retrieval/cleanup-old/result.json` exit0/MCP kaldırma/canonical endpoint/released owner geçti. Bu profil kullanıcı bağlantısı değildir.

İlgili girdiler değişmedikçe başarılı kapılar tekrarlanmaz. Gerekirse mevcut komutlar:

```powershell
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B -m unittest discover -s tests/runtime -p test_retrieval_bridge.py
```

Diğer yerel kapılar [WORKFLOW.md](../development/WORKFLOW.md) içindedir. Native kanıt için paketli release ve gerçek OpenCode gerekir; yalnız unit test veya eski test-only MCP köprüsü bu kabulü yeniden üretmez.

## Açık kabul

Canlı sağlayıcı/auth ve gerçek model çıkarımı, gerçek ana/alt ajan kimliği/ayrı yolları, model değişimi, TUI/V2/Desktop, temiz Windows/Windows 11 ve diğer OS ayrı açık kalır. Seçilen kullanıcı dosyası ve eski aynı-kullanıcı config katmanları kontrol edilir; başka sürecin proje/remote/managed/plugin katmanları etkin ayarı değiştirebilir. Tarihsel endpoint-only katalog profili bu farklı fixture birleşimini kapsıyormuş gibi yükseltilmedi. Actions, yeni hesap/kurulum, yayın veya ücretli model çağrısı yapılmadı; Linux ve Cloudflare ertelemeleri korunur.
