# H051 — Oturumlar: yerel kanıt ve açık kabul

13 Kasım 2026. **H051 tamamlandı — Windows 10 22H2 x64 yerel geliştirme.** [Makine kaydı](sessions.json), [karar](../decisions/031-session-metadata.md), [inceleme](../reviews/H051.md).

Oturumlar → Sohbetler / Hesaplar hazır. Desteklenen mevcut Codex sunucusunda açık/çalışıyor/bekliyor/kayıtlı/sona erdi/bilinmiyor ayrılır. Platform, model, proje basename'i ve varsa üst/alt ajan ilişkisi gösterilir. Son trafik ve sohbet tasarrufu, istek bağı doğrulanmadığı için bilinmiyor kalır. Sohbet başlığı, geçmişi ve auth kaydı tutulmaz; metadata API'sindeki fazladan alanlar parser'da atılır. Açma, resmî Codex sohbet merkezinde kullanıcı seçimiyle devam eder; Headroom gizli resume veya turn başlatmaz.

| Senaryo | Gözlenen | Kanıt / kapsam |
| --- | --- | --- |
| Strict istemci/IPC, hata ve modelden bağımsız metadata | 221 Node testi geçti | `work/proofs/sessions/final-reviewed.json`; sonraki React görünürlük yardımcı değişikliği Node testleri tarafından içe aktarılmaz. Diğer girdiler eşleşir. |
| Runtime, süreç güveni, parser/manager ve mevcut ürün | 441 Rust testi geçti, 17 opt-in ignored; fmt, frontend ve embedded debug derlemesi geçti | `final-candidate.json`; sonradan yalnız ignore edilen native assertion ve UI test harness'i değişti. Üretim kodu aynı; yeni assertion gerçek opt-in native koşuda derlendi ve geçti. |
| Gerçek App fixture: TR/EN, açık/koyu, boş/hata, görünmezlik, focus, onay ve launch hatası | 6 sohbet, 3 hesap ve 4 genel ekran senaryosu; 122 Axe denetimi, 0 ihlal | `ui-stable-theme.json` ve makine kaydındaki üç UI raporu. Native IPC/sağlayıcı kanıtı değildir. |
| Gezinme | 60 örnekte ikinci çizim karesine p95 16.9 ms, eşik 250 ms | İzole browser click ölçümü; test sürücüsü bekleme zamanı, native DPI ve uzun süreli kararlılık hariç. |
| Gerçek Codex metadata, iki aynı model, sessiz açık sohbet, sunucu kaybı | 3 boş sohbet, 2 aynı model, mevcut user-agent korundu; kayıp sunucu canlı/ended sayılmadı | `work/proofs/s51/7aa331aa/result.json`; 0 inference POST, 0 dış ağ denemesi. |
| Resmî Codex merkezi | Hedef izole sunucuda gerçek `codex-tui` initialize tanığı, üç sohbetin model/provider/reasoning korunumu ve Headroom çıktıktan sonra açık kalma | `work/proofs/s51/12e3ea28/result.json`; fixed-command/sahip-parent receipt'i `ui-center.json` içinde. Public TUI duyuru CONNECT'i TLS başlamadan guard ile engellendi; inference POST 0. |
| Gerçek Headroom penceresinde son onay/iptal/açma/çıkış | **Geçti** | Üç açık kayıt/iki aynı model, Hesaplar ve Sohbetler arasında gidip dönme, odaklı onay penceresinde Vazgeç ile istemci başlatmama, yeniden onayla resmî merkezi açma ve başarı mesajı gözlendi. Headroom normal kod 0 ile çıktı; açılan istemci hem Headroom hem test sohbetlerinin sahibi çıktıktan sonra yaşadı. Yalnız sahipli test süreçleri temizlendi. |

Sohbet/hesap bileşenlerinin geçerli UI kanıtı yeniden kullanılabilir: üretim kaynakları aynı, yalnız içe aktarılmayan genel erişilebilirlik testi ve `preserveOutput` artefakt politikası değişti. Eski config baytları programatik yeniden oluşturulup hash ile doğrulandı. Genel ekran kontrast koşusunda tema geçişinin iki farklı paleti bir ölçümde yakaladığı görüldü; font ve iki çizim karesi beklenerek aynı kontrast kurallarıyla 100 denetim yeniden geçti. Hiçbir kontrast kuralı veya performans eşiği düşürülmedi. Ham Playwright hata bağlamı korunmaz.

Önce/sonra kaynak/config, bağımlılık ve araç fingerprint'leri JSON kaydındadır. Cargo/rustc 1.92.0, Node 25.2.1, pnpm 11.25.0 ve Edge 157.0.4341.46 kullanıldı. Gerçek Codex `0.167.0-alpha.4.1` imza/sürüm ve sabit kamu kaynaklarıyla doğrulandı; bu sınır başka sürümlere taşınmaz.

Koyu/TR ve açık/EN render'ları `work/proofs/sessions/tr-dark.png` ile `en-light.png` içinde görsel olarak incelendi. Bunlar açıkça izole örnek verilerdir; gerçek sohbet/hesap veya native başarı resmi değildir.

## Son kabul ve sınır

Kullanıcı kilidi açtıktan sonra WTS unlock durumu doğrulandı. Sky capture zaman aşımı nedeniyle mevcut open-computer-use erişilebilirlik eylemleriyle ana ajan gerçek pencereyi kontrol etti; native görüntü de gözle incelendi. İlk kilitli koşu `e782f555` ile boş test sohbetlerinin sahibi erken kapandığı için 0 yüklü gösteren `4ba2fa29` koşusu başarısız olarak korunur. Son fixture, boş sohbetlerin sahibi olan bağlantıyı UI bitene kadar tutar. Bu yalnız ignore edilen test kurulumu değişikliğidir; üretim kodu değişmedi. Yeni opt-in test derlenip çalıştı, güncel fmt kontrolü geçti; önceki geçerli Node/Rust/default build ve browser kontrolleri tekrar edilmedi.

`12e3ea28/observations.json` gerçek UI gözlemlerini içerik toplamadan sayılar/boolean'larla kaydeder. Aynı koşu kaynak ve fixture config fingerprint'leri önce/sonra eşleşir. H051 kutusu kapandı, toplam **42/54**. Sonraki ana görev kullanıcı devam mesajını bekler.

Linux H049 kullanıcının kalan yaklaşık $0.10 Actions kotası nedeniyle ertelenmiştir. Bu görevde CI, gerçek giriş veya model isteği başlatılmadı. Yeni özelliğin Windows 11, macOS/Linux, gerçek sağlayıcı/alt ajan turn'ü, başka profil/sürüm ve yayın paketi kabulü yoktur. Claude/OpenCode sohbet durumunun bilinmemesi, sıkıştırma motorunun model desteği hakkında hüküm değildir. Geliştirme token tasarrufu ölçülmedi.
