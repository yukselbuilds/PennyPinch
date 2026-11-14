# OpenCode V2 salt okunur sürüm tanıma

14 Kasım 2026. **H010-f yerel kabul; ana H010 açık, toplam 42/54.** [Kanonik JSON](opencode-v2-discovery.json), [karar](../decisions/009-opencode-provider-routes.md#h010-f--v2-sürüm-tanıma-ve-güvenlik-sınırı), [inceleme](../reviews/H010-f.md).

Ürün keşfi artık H010-e'de doğrulanmış resmî Windows x64 baseline 2.3.0 executable'ını boyut/SHA256 çiftiyle tanır. Yeni indirme veya sistem kurulumu yapılmadı. V1 1.19.3 aynı matcher'da korunur. PATH, kullanıcı-local ve manuel dosya okuyucuları iki boyuttan birini kabul eder; hash, bounded read, ilk/son handle ve path identity kontrolleri aynı kalır. Keşif `--version` çalıştırmaz. Retained file pin doğru sürümü ayrıca doğrular ve normal yazma/replacement yarışına karşı read handle'ını tutar.

## Gerçek dosya ve backend sonucu

Windows 10 22H2 x64 build19045 üzerinde, yalnız proje içindeki açıkça seçilmiş iki CLI artefaktı okundu. V2 opt-in testi kendi UUID fixture'ında geçici user-local hardlink kurar; cleanup yalnız bu kökü/linki kaldırır. Kaynak binary'nin baytları değişmez. Normal dosya seçici yerine aynı DiscoveryManager metoduna fixture seçimi verilir; fiziksel pencere/file-picker yeniden denenmiş sayılmaz.

| Kabul | Gözlenen |
| --- | --- |
| Gerçek V2 PATH/manual/user-local | Üçü de 2.3.0; bağlantı not_verified |
| Manual rescan | Seçili dosya ve sürümü kaldı |
| Doğru sürüm pin | Alındı; write-open reddedildi |
| Yanlış expected version | Pin reddedildi |
| Gerçek V1 dosyası | PATH keşfinde 1.19.3 korundu |
| Backend V2 preview | Bilinen sürüm; blocked, after_headroom=false |
| Backend Apply | config_conflict; fixture JSONC aynı; ownership root yok |
| Hesap kabiliyeti | 2.3.0 için kapalı |

Yanlış boyut/digest, iki binary'nin çapraz boyut/hash eşleşmesi ve aynı boyutta farklı digest unknown kalır. Plan testi hem V1 hem V2 biçimindeki JSON'un 2.3.0 sürümle V1 writer'a açılamadığını doğrular. Hesap parser/login kapısı ayrıca kapalıdır. UI'nin select_client_file → DiscoveryManager ve preview/apply komutları → PlanManager yolu incelendi; backend testi gerçek dosya/gerçek transaction engelini kullanır. Mock model/başarılı bağlantı sunulmaz.

Katalog yalnız eski standalone CLI Chat/Responses preflight'ını local_fixture_partial olarak günceller. V2 config yazma/GUI/alt ajan kabiliyetleri not_tested, auth/platform/surface/version matching kapsamı aynı kalır. Bu kayıt otomatik V2 ürün desteği veya giriş yapılmış hesap göstermez.

## Kanıt ve tekrar

- Son tam kapı `work/proofs/opencode-discovery/checks-b3151e09f2/final.json`: **222 Node, 450 Rust PASS; 19 ignored; fmt/frontend/embedded build PASS.**
- Ayrı gerçek dosya testi `checks-296b2e1367/real.json`: **1 PASS**; tam kapıdaki ignored testin gerçek çağrısıdır. Her iki kayıt aynı 341 kaynak/test/config/CLI/bağımlılık/araç/OS girdisini önce/sonra bağlar.
- H010-e'nin 645 protokol girdisi değişmedi; başarılı yerel model turu tekrarlanmadı ve kapsamı genişletilmedi. Debug exe ve dist hash'leri kanonik kayıtta ayrıca sabittir.
- 59 Rust/5 frontend focused PASS, son ignored test fixture'ındaki DOS-path düzeltmesinden önce alındı. Son adayın normal testleri tam kapıyla, gerçek dosya testi ayrı güncel kayıtla kabul edilir. Eski focused kayıt güncel dosya fingerprint'i gibi sunulmaz.
- CodeGraph son dört Rust dosyasını sync etti; yeni ortak digest matcher'ı bir sonuçlu sorguyla bulundu. Graph çıktısı runtime PASS değildir.

İlk derlemede test private sibling fonksiyonuna erişti. Üretim API görünürlüğü genişletilmeden gerçek public discovery/pin yolu kullanıldı. İlk gerçek koşuda V2/plan/pin adımları geçti; V1 fixture'ının canonical `\\?\` PATH girdisi namespace guard'ına takıldı. Yalnız test normal DOS proje yoluna düzeltildi; güvenlik guard'ı değiştirilmedi. İki başarısız kayıt ve önceki focused PASS hash'leri kanonik JSON'da korunur.

Pinned proje toolchain ve önceki yerel hazırlıklar hazırken repo kökünde:

```powershell
$env:HEADROOM_TEST_OPENCODE_V2_CLI = 'D:\Projeler\Headroom\work\toolchains\opencode\v2.3.0\runtime\opencode.exe'
$env:CARGO_HOME = 'D:\Projeler\Headroom\work\caches\cargo-home'
$env:CARGO_TARGET_DIR = 'D:\Projeler\Headroom\work\caches\cargo-target'
cargo test --locked --offline --manifest-path src-tauri/Cargo.toml opencode_v2_project_binary_is_discovered_retained_and_pinned_without_execution -- --ignored --nocapture --test-threads=1
```

Kanonik kayıttaki `verify.py real/final` aynı komutları yalnız alt sürece ait ortamla, yeni proof dizininde çalıştırır. Gerçek kullanıcı ayarı/auth, kurulu istemci veya aktif chat değiştirilmez. Client/model/account açma, Actions, ücretli API veya yayın yoktur.

V2 writer/receipt/MCP retrieval, normal CLI/Desktop servis adoption, gerçek auth/alt ajan ve Windows 11/macOS/Linux kabulü açık. Gerçek token/fatura veya geliştirme sohbeti tasarrufu ölçülmedi.
