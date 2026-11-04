# H008 — gerçek Desktop hesabı ve aynı sohbet

4 Kasım 2026. Windows 10 Pro 22H2 x64. Ana ajan auth, rota ve süreç sahipliği nedeniyle bu doğrulamayı yürüttü. Ürün kaynağı değişmedi.

## Gözlenen davranış

Kurulu ve OpenAI imzası doğrulanan Desktop `26.1027.6604.0`, backend `0.166.0-alpha.5.1` ve pinli Headroom `0.41.0` kullanıldı. Kullanıcı ayrı uygulama state profilinde resmî ChatGPT girişini tamamladı ve ilk kullanım ekranını kendisi atladı. Profilin varsayılan modeli ve eforu korundu. İki turda `gpt-6-luna` / `medium` gözlendi. Bu, ürün için model koşulu değildir.

| Senaryo | Sonuç |
| --- | --- |
| Profilde kayıtlı Headroom uç noktasıyla Desktop açılışı ve kısa istek | Koşuya özel tam yanıt görünür arayüzde doğrulandı. |
| Yeni sohbet ekranına geçiş | Test sohbetinin seçimi ve ilk yanıtın görünürlüğü kapandı. Önceki sohbetin gizli DOM'da kalması başarı ölçütü yapılmadı. |
| Kenar çubuğundan aynı sohbeti açıp ikinci istek gönderme | İlk yanıt geri geldi, native sohbet kimliği eşleşti ve ikinci tam yanıt görünür oldu. |

Sohbet kaydında aynı kimliğe ait iki kullanıcı girdisi, iki tam asistan yanıtı ve iki model/efor kaydı ayrıca eşleşti. İnceleme yalnız bu deneyin kimliğiyle sınırlandı. Kanıta metin içeriği veya auth dosyası eklenmedi.

Motor beş yerel istek satırı, bir cache kaydı ve sıfır başarısız istek bildirdi. İstek amaçları ayrı sınıflandırılmadığı için bu sayılar iki kullanıcı turuna bire bir eşitlenmez. WebSocket üzerinden gerçek yanıt tamamlandı. Üç satırdaki ham yerel token tahminleri `22628→22212`, `15082→14885`, `23440→23413` idi. Bu alanlar sağlayıcı faturalandırması veya doğrulanmış kota tasarrufu değildir. [Ölçüm sözleşmesi](../decisions/010-metrics-normalization.md) korunur.

## Sahiplik ve doğrulama

- Microsoft paket içi tanı başlatması ve yalnız test sürecine ait loopback CDP kullanıldı. Bu, standart launcher veya kurulum paketi kabulü değildir.
- Test penceresi sahipli Job ile kapatıldı. Bu kapanış normal GUI kapanış testi sayılmadı. Motor graceful/exit 0 ile durdu, test süreçleri ve iki listener kapandı.
- Normal Codex config hash'i eşleşti. Ayrı test profilindeki sahipli endpoint alanı geri kaldırıldı; Desktop'ın eklediği diğer profil ayarları korundu. Kimlik bilgisi uygulama state kökünde kaldı.
- Desktop/backend dosyaları önce/sonra eşleşti. Yardımcı ve motor kaynak fingerprint'leri eşleşti. Önceki tam kalite kapısının 101 ürün girdisi aynı kaldı; tam kapı tekrarlanmadı.
- Kurulu Desktop paketi ve backend dosyası önceki opt-in testlerden sonra değiştiği için gerçek keşif/pin ve CLI+Desktop izole Apply/Undo testleri yenilendi. İkisi de PASS verdi. Bu kontroller model çağrısı yapmadı.

Yerel kanıt kökü `work/proofs/codex-current/desktop-live-runs/dd004ad6-2551-403a-b07e-8a5a236f80ce/`. `desktop-live.json`, `finish.json`, `native-chat-metadata.json`, `installed-check.json` ve `verification.json` içeriksiz sonuç ve fingerprint'leri tutar.

Önceki `9de4c852-5008-4870-b03a-4c184decc742` koşusu boş arayüzle süre sonunda kapandı, model isteği gözlenmedi. İki koşu arasında paket ve test ortamı değiştiğinden önceki boş ekranın kök nedeni bu PASS ile açıklanmış sayılmaz.

## Çalışan uygulamada ayar ve tam yeniden açılış

Aynı gün ayrı `6d0d6209-1743-43b0-84bb-4df18dac61f0` koşusu mevcut test hesabı ve sohbetiyle yapıldı. İlk Desktop süreci Headroom ayarı olmadan başlatıldı. Aynı native sohbet görünür olduktan sonra yalnız bu test profilinin endpoint alanı atomik eklendi.

İlk ek yanıt aynı GUI süreci açıkken tamamlandı, Headroom istek ve WebSocket sayaçları sıfır kaldı. Bu sürüm, çalışan süreçte haricen eklenen ayarı o turda kullanmadı. Test penceresi sahipli Job ile kapatılıp kayıtlı ayarla yeniden açıldı. Önceki sohbet ve yanıt geri geldi. İkinci ek yanıt aynı native kimlikle Headroom üzerinden tamamlandı. İkinci istek öncesi ve sonrası farklı motor satırı ayrıca gözlendi. Bu sonuç mevcut UI'daki yeniden açma uyarısıyla uyumlu; ürün kopyası değiştirilmedi.

Frontend'in ilk süreçteki geçici `client-new-thread` kimliği yeniden açılışta native kimliğe dönüştü. Eşleme aynı native sohbet üzerinden yapıldı. Bu, sohbet DB'sini yeniden etiketleme işlemi değildir.

İki test GUI süreci ve üç loopback listener kapandı. Motor graceful/exit 0 verdi. Normal config ve kaynak/paket fingerprint'leri eşleşti. Test profilindeki sahipli endpoint kaldırıldı. Yerel kanıtlar ikinci run altındaki `before-apply.json`, `after-change.json`, `before-second.json`, `finish.json`, `adoption.json` ve `verification.json` dosyalarıdır.

## Kabul sınırı

H008 ana görevi açık. Bu sürümde yeni ayarın o açık turda kullanılmadığı ve tam yeniden açılışla mevcut sohbetin Headroom'a yönlendiği ölçüldü. Canlı API-key sağlayıcı yolu henüz doğrulanmadı; API-key HTTP/SSE ve yerel Desktop WebSocket fixture kanıtları ayrı tutulur. CLI'nin farklı model koşusundaki wait çağrısı dispatch öncesi iptal olduğu için o birleşik yaşam döngüsü sonucu kısmi kalır. Windows 11, macOS, Linux, bütün model özellikleri, aktif stream sırasında ayar değiştirme ve uzun süreli kararlılık bu kısa koşunun kapsamı değildir.
