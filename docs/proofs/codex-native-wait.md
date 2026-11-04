# H008 — native bekleme ve MCP kontrolünün ayrılması

4 Kasım 2026, Windows 10 22H2 x64. Ana ajan auth/rota ve kabul semantiği nedeniyle deneyi yürüttü. CLI `0.166.0` ve pinli Headroom `0.41.0` kullanıldı. Ana model/efor mevcut `gpt-6-astra` / `max` olarak korundu; ayrı deneye özel tek alt ajan `gpt-6.1-sol` / `medium`, `fork_turns=none` kullandı. Bu seçimler ürünün model koşulu değildir.

## Doğrulanan native akış

`bf2d5c8e-ce03-4f42-b0f5-7f34fc4621ba` koşusunda native kayıt gerçek bir spawn, bir wait çağrısı ve eşleşen iki tool dönüşü gösterdi. Bekleme runtime olayı completed, dönüş JSON'unda `timed_out=false` idi. Tek alt ajan başladı ve tamamlandı; parent ve child son işaretleri eşleşti. İki farklı sohbet kimliği ayrı Responses girişlerinde eşleşti. Dokuz bağlantının iki ucu sahiplik kontrolünü geçti. CLI exit 0 verdi ve normal config hash'i korundu.

Bu, önceki koşunun dispatch öncesi iptal edilmiş wait kaydından ayrı, gerçekleşmiş bir native bekleme kanıtıdır. Eski koşunun kısmi sonucu tarihsel olarak korunur. Legacy stdout okuyucusunun kısmi sonucu native dosya kanıtının yerine kullanılmadı.

## Başarı sayılmayan kontrollü araç senaryosu

Aynı koşunun ayrıca bir yerel metadata MCP kapısını çağırması amaçlanmıştı. Kapı isteği sayısı sıfır kaldı. Modele önceden verilen işaretin geri gelmesi araç çağrısının kanıtı yapılmadı; bu senaryo geçmedi.

İkinci `1e5b1874-1dca-4cd8-b111-c7763f33669d` koşusunda beklenen işaret prompttan çıkarıldı. Araç keşfine izin verildi. Alt ajan ve parent `HEADROOM_GATE_UNAVAILABLE` yanıtını verdi; native wait yine eşleşen completed olayı ve `timed_out=false` dönüşü üretti. Kapı isteği yine sıfırdı. Kontrollü alt ajan MCP senaryosu tamamlanmadı; nedenin Headroom olduğu çıkarılmadı.

İkinci koşuda motor 7 yerel istek, 0 başarısız istek ve 0 başarısız WebSocket sıkıştırma frame'i bildirdi. İstek amaçları tek tek sınıflandırılmadı. QA relay iki `ConnectionResetError / 10054` gözledi; kaynak yönü ve terminal yanıtla zaman ilişkisi kaydedilmedi. Bu sayaçlardan genel transport sağlığı veya sıfır hata iddiası çıkarılmadı.

## Model çağrısı yapmayan doğrudan kontrol

`4c322df7-d294-413f-ae6b-1f05c47427fa` koşusu ayrı profil ve sentetik API anahtarıyla yalnız initialize, ephemeral thread/start, mcpServerStatus/list ve mcpServer/tool/call kullandı. `turn/start` çağrılmadı. Gerçek CLI, test aracını listeledi ve çağırdı; yerel kapı bir isteği gördü, doğru işaret döndü ve stdin kapanışında exit 0 gerçekleşti. Bu kontrol fixture'ın gerçek istemci MCP protokolüyle çalıştığını kanıtlar; modelin/alt ajanın aynı aracı kullanabildiğinin kanıtı değildir.

`tests/fixtures/codex/native_wait_gate.py` yalnız stdio JSON-RPC ve sabit loopback metadata uç noktasını kullanır. Üç odaklı test geçerli çağrıyı, yabancı argüman reddini ve hata/yanlış boolean ile sahte başarı verilmemesini doğruladı. Önceki reddedilmiş OS/file-read eylemi tekrarlanmadı; bu fixture OS aracı testi değildir.

## Kanıt ve kalan kapsam

Yerel raporlar `work/proofs/codex-current/gated-agent-runs/<yukarıdaki-UUID>/` içindedir. Toplu yeniden inceleme `work/proofs/codex-current/native-wait-verification.json` dosyasındadır. Yalnız deneye ait parent/child kayıtları kimlik hash'iyle bulundu; kaynak/artefakt ve kayıt fingerprint'leri önce/sonra eşleşti. Yeniden inceleme sağlayıcı isteği tekrarlamadı. Test süreçleri ve kontrol edilen listener'lar kapandı; motor graceful/exit 0 verdi. İçerik, auth ve ham hata metni rapora alınmadı.

H008 ana görevi açık. Native mesaj/bekleme ve ayrı rota dilimi doğrulandı; kontrollü alt ajan MCP araç erişimi, relay reset sınıflandırması ve canlı API-key/geniş auth kapsamı ayrıca bekliyor. UI/runtime kodu ve global model/ajan/hook ayarları değişmedi; iki Python test fixture'ı eklendi. Tasarruf veya bütün modeller için kalite garantisi ölçülmedi.
