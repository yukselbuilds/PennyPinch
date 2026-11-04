# H008 — native bekleme ve MCP kontrolünün ayrılması

4 Kasım 2026, Windows 10 22H2 x64. Ana ajan auth/rota ve kabul semantiği nedeniyle deneyi yürüttü. CLI `0.166.0` ve pinli Headroom `0.41.0` kullanıldı. Ana model/efor mevcut `gpt-6-astra` / `max` olarak korundu; ayrı deneye özel tek alt ajan `gpt-6.1-sol` / `medium`, `fork_turns=none` kullandı. Bu seçimler ürünün model koşulu değildir.

## Doğrulanan native akış

`bf2d5c8e-ce03-4f42-b0f5-7f34fc4621ba` koşusunda native kayıt gerçek bir spawn, bir wait çağrısı ve eşleşen iki tool dönüşü gösterdi. Bekleme runtime olayı completed, dönüş JSON'unda `timed_out=false` idi. Tek alt ajan başladı ve tamamlandı; parent ve child son işaretleri eşleşti. İki farklı sohbet kimliği ayrı Responses girişlerinde eşleşti. Dokuz bağlantının iki ucu sahiplik kontrolünü geçti. CLI exit 0 verdi ve normal config hash'i korundu.

Bu, önceki koşunun dispatch öncesi iptal edilmiş wait kaydından ayrı, gerçekleşmiş bir native bekleme kanıtıdır. Eski koşunun kısmi sonucu tarihsel olarak korunur. Legacy stdout okuyucusunun kısmi sonucu native dosya kanıtının yerine kullanılmadı.

## Başarı sayılmayan kontrollü araç senaryosu

Aynı koşunun ayrıca bir yerel metadata MCP kapısını çağırması amaçlanmıştı. Kapı isteği sayısı sıfır kaldı. Modele önceden verilen işaretin geri gelmesi araç çağrısının kanıtı yapılmadı; bu senaryo geçmedi.

İkinci `1e5b1874-1dca-4cd8-b111-c7763f33669d` koşusunda beklenen işaret prompttan çıkarıldı. Araç keşfine izin verildi. Alt ajan ve parent `HEADROOM_GATE_UNAVAILABLE` yanıtını verdi; native wait yine eşleşen completed olayı ve `timed_out=false` dönüşü üretti. Kapı isteği yine sıfırdı. Sonraki salt okunur incelemede aynı child kaydındaki `exec` çağrısı ve dönüşü call_id ile eşleşti: MCP çağrısı istemcinin `approval_policy=never` politikası nedeniyle onay gerektirdiği için engellenmişti. Durum `environment_blocked`, sabit neden kodu `mcp_requires_approval_policy_never` olarak kaydedildi. Araç envanterinde bulunmama varsayımı bu koşul için geçerli değil. İzin politikası değiştirilmedi ve reddedilen çağrı tekrar edilmedi. Bu, Headroom rota hatası kanıtı değildir.

İkinci koşuda motor 7 yerel istek, 0 başarısız istek ve 0 başarısız WebSocket sıkıştırma frame'i bildirdi. İstek amaçları tek tek sınıflandırılmadı. QA relay iki `ConnectionResetError / 10054` gözledi; kaynak yönü ve terminal yanıtla zaman ilişkisi kaydedilmedi. QA kaynağında `relay_duplex.complete` iki TCP EOF ve boş tamponları bekler; modelin terminal olayını ayrıştırmaz. Mevcut kayıt orijinal reset yönünü veya terminalden önce/sonra gerçekleştiğini belirlemeye yetmez. Bu sayaçlardan genel transport sağlığı, hataların zararsızlığı veya sıfır hata iddiası çıkarılmadı; tamamlanmış native yanıt/rol kanıtı ayrı korunur.

## Model çağrısı yapmayan doğrudan kontrol

`4c322df7-d294-413f-ae6b-1f05c47427fa` koşusu ayrı profil ve sentetik API anahtarıyla yalnız initialize, ephemeral thread/start, mcpServerStatus/list ve mcpServer/tool/call kullandı. `turn/start` çağrılmadı. Gerçek CLI, test aracını listeledi ve çağırdı; yerel kapı bir isteği gördü, doğru işaret döndü ve stdin kapanışında exit 0 gerçekleşti. Bu kontrol fixture'ın gerçek istemci MCP protokolüyle çalıştığını kanıtlar; modelin/alt ajanın aynı aracı kullanabildiğinin kanıtı değildir.

`tests/fixtures/codex/native_wait_gate.py` yalnız stdio JSON-RPC ve sabit loopback metadata uç noktasını kullanır. Üç odaklı test geçerli çağrıyı, yabancı argüman reddini ve hata/yanlış boolean ile sahte başarı verilmemesini doğruladı. Önceki reddedilmiş OS/file-read eylemi tekrarlanmadı; bu fixture OS aracı testi değildir.

## Kanıt ve kalan kapsam

Yerel raporlar `work/proofs/codex-current/gated-agent-runs/<yukarıdaki-UUID>/` içindedir. Toplu yeniden inceleme `work/proofs/codex-current/native-wait-verification.json` dosyasındadır. Yalnız deneye ait parent/child kayıtları kimlik hash'iyle bulundu; kaynak/artefakt ve kayıt fingerprint'leri önce/sonra eşleşti. Yeniden inceleme sağlayıcı isteği tekrarlamadı. Test süreçleri ve kontrol edilen listener'lar kapandı; motor graceful/exit 0 verdi. İçerik, auth ve ham hata metni rapora alınmadı.

İzin tanısının içeriksiz raporu `work/proofs/codex-current/approval-classification.json` içindedir. Yalnız aynı koşunun kimliği/fingerprint'i doğrulanan child kaydı incelendi; sağlayıcı veya MCP çağrısı tekrarlanmadı. Ham hata metni, auth ve konuşma içeriği kaydedilmedi.

## H008 kapsam kararı ve kapanış

4 Kasım'da kullanıcı, H008'in ChatGPT kanıtıyla kapanmasını ve canlı API-key testinin H041 yayın öncesi kabulünde açık kalmasını seçti. Bu açık kapsam kararıyla H008 tamamlandı; API-key desteği canlı doğrulandı sayılmadı. Yeni ücretli kaynak kullanımı yetkilendirilmedi.

| Kabul | Kanıt ve sınır |
| --- | --- |
| Gerçek CLI yönlendirmesi; mevcut giriş/model/efor | CLI0.166.0, ChatGPT ve pinli Headroom; [güncel kullanım](codex-current.md), [tanı](codex-current-diagnostics.md). |
| Ana ve tek alt ajan; aynı/farklı model-efor; native bekleme | Ayrı sohbet/giriş kimlikleri ve tamamlanan spawn/wait; yukarıdaki native kayıt. Bu test modelleri ürün allowlist'i değildir. |
| Gerçek Desktop; aynı sohbet ve yeniden açılış | Desktop26.1027.6604.0/backend0.166.0-alpha.5.1, ChatGPT, görünür tamamlanmış yanıt ve aynı native sohbet; [Desktop kanıtı](codex-desktop-live.md). Açık süreçte ayar değişimi sonrası yeniden açma gerekir. Desktop alt ajan senaryosu ayrıca not_tested. |
| Hata/kapsam sınıflandırması | MCP izin engeli environment_blocked; iki QA TCP resetinin yön/zamanı unknown. Genel araç/transport sağlığı iddiası yok. |
| API-key ve diğer ortamlar | API-key HTTP/SSE fixture ayrı; canlı sağlayıcı testi H041'de açık/not_tested. Windows11/macOS/Linux, bütün model özellikleri ve uzun süreli kararlılık bu kabulün kapsamında değil. |
| İnceleme ve kalite | Ana ajan önceki UI→native yönetici→motor/istemci kanıtlarını ve yeni katalog/belge farkını inceledi. Değişmeyen 101 ürün girdisiyle 289 Rust/98 frontend ve build/fmt PASS kanıtı korunur; yeni Python fixture'larının 3 testi ayrıca PASS. |

Son içeriksiz kabul/fingerprint kaydı `work/proofs/codex-current/h008-chatgpt-acceptance.json` içindedir. Sürüm/senaryo durumları [uyumluluk kataloğuna](../../compatibility/clients.json) eklendi; eski kayıtların kanıt kapsamı genişletilmedi. Katalog henüz ürün UI'sına bağlı değildir (H036). UI/runtime ve global model/ajan/hook ayarları değiştirilmedi. İki Python test fixture'ı önceki dilimde eklendi. Tasarruf veya bütün modeller için kalite garantisi ölçülmedi. Sıradaki H024 başlatılmadan duruldu.
