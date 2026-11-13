# H050 — Hesap bağlantıları kabulü

13 Kasım 2026. Windows 10 22H2 x64 yerel geliştirme kabulü tamamlandı. [Makine kanıtı](accounts.json) 317 kaynak/config girdisini, araç sürümlerini, komut özetlerini ve native ikiliyi programatik bağlıyor. [Karar 030](../decisions/030-account-connections.md) hesap, yönlendirme ve trafik durumlarının ayrımını tanımlar.

## Kullanıcıya sunulan davranış

Uygulamalar ekranındaki Hesaplar bölümü bulunan istemcileri otomatik seçer; birden çok kurulum varsa seçim değiştirilebilir. Yerel kayıt var/yok/bilinmiyor durumları, kaydın türü ve desteklenen OpenCode provider kayıtları gösterilir. Desktop ve başka CLI profilleri bu kayıtla doğrulanmış sayılmaz. Hesap okunması bağlantı ayarı yazmaz veya model/alt ajan seçmez.

Resmî giriş önce seçilen CLI sürümünü ve mevcut oturumu değiştirme riskini gösterir. Codex ChatGPT, Claude Code Claude hesabını açar; OpenCode yöntem/sağlayıcıyı kendi konsolunda seçtirir. Parola/kod Headroom'da alınmaz. Tam çıkış ve süre sınırı yalnız sahip olunan CLI kökünü durdurur; tarayıcı başka süreçlerle birlikte kapatılmaz. İptal önceki auth'u geri yükleme vaadi vermez. Kapanış doğrulanamazsa sahiplik korunur ve yeniden durdurulabilir.

## Doğrulamalar

| Katman | Sonuç ve sınır |
| --- | --- |
| Frontend sözleşmesi | 216 Node testi geçti; account korelasyonu, açık onay, fazla/secret alan reddi ve akış kimliği dahil. |
| Rust | 432 geçti, 16 doğrudan ignored. Hesap parser'ları, pin/discovery regresyonları, stale seçim ve native konsol sahibi testleri kapsamda. Ignored çocuk süreç yardımcısı ilgili ebeveyn testinden çağrılır; diğer opt-in testler çalıştırılmış sayılmaz. |
| Derleme | TypeScript/Vite, Rust fmt, kilitli/offline native debug build ve ayrı kimlikli QA release build geçti. Rust 1.92.0, Node 25.2.1, pnpm 11.25.0. |
| İzole UI | 7 Edge senaryosu, 100 erişilebilirlik denetimi; sıfır ihlal. TR/EN, açık/koyu, dar görünüm, odak, onaydan vazgeçme, pending akışla yeniden açılış, cleanup retry, kayıp başlangıç yanıtı ve stale okuma test edildi. Navigasyon ölçümü yalnız tarayıcı fixture'ıdır; OS DPI veya sağlayıcı gecikmesi değildir. |
| Native Windows 10 | Ayrı release kimliğiyle gerçek Uygulamalar → AccountManager/IPC → hesap satırları → Codex kapsam onayı → Vazgeç → Etkinlik → geri dönüş → normal onaylı çıkış. Gerçek Codex 0.167.0-alpha.4.1 varsayılan CLI ChatGPT kaydı okundu; OpenCode 1.19.3 verisi sahipli sentetik XDG kaydıydı. Claude status komutu çalıştırılmadı. Çıkış kodu 0, kaynak/ikili fingerprint ve kontrol edilen gerçek auth/ayar dosyalarının boyut/mtime/kimlik metadata'sı değişmedi. Auth içerikleri kopyalanmadı veya hash'lenmedi. |

İlk Sky ekran yakalama/tıklama yolu zaman aşımı ve koordinat hatası verdi; klavye odağı da değişmedi. Kabul, kurulu open-computer-use aracının erişilebilirlik Invoke/ScrollIntoView eylemleri ve gerçek native görüntüleriyle yapıldı. Kullanıcıdan bu koşuda tıklama yardımı istenmedi. QA kopyasında motor payload'ı yoktu; görülen motor-kullanılamıyor bandı hesap kabulünün dışında ve beklenen durumdu. Bu koşu installer/motor teslimi kabulü değildir.

## Pasif okuma ve inceleme bulguları

Claude Code 2.1.158 resmî status komutu, yalnız sahipli sentetik profilde ayar ve yedek oluşturdu. OpenCode V1 auth-list, fetch kapatılmış izole deneyde bile global DB/WAL/SHM oluşturdu. Bu komutlar ürünün otomatik hesap okumasında kullanılmaz: Claude bilinmiyor, OpenCode salt okunur sabit V1 metadata adapter'ı. Proxy'de sıfır erişim gözlemi tam ağ izolasyonu olarak sunulmaz.

Ana ajan UI devrinde ekrana dönüşün akışı iptal ettiğini buldu ve düzeltti. Kapanışı doğrulanmayan sürecin değiştirilmesi, hatalı süreç durumu sonrası erken kaynak bırakma ve launch yanıtı kaybolduğunda toparlanamama da düzeltildi. [İnceleme](../reviews/H050.md).

İlk tam kapıda Node testleri geçti, TypeScript durum daraltması düzeltildi. Geçerli Node girdileri yeniden kullanılabildi. Sonraki ürün kapısında Rust/fmt/build geçti; eski UI testi katalogda 12 kayıt beklerken mevcut 13 kaydı gördü. Sayı beklentisi güncel kataloğun bütün kayıtlarının görünmesini doğrulayacak biçimde düzeltildi ve tüm UI senaryoları yenilendi. Başarısız raporlar korunur; başarılı ürün kontrolleri yalnız ilgili girdileri programatik eşleştiğinde yeniden kullanıldı.

## Doğrulanmayan kapsam

Yeni gerçek sağlayıcı girişinin tamamlanması, canlı API-key erişimi, Desktop hesapları, yeni hesap adapter'ının Windows 11/macOS/Linux çalışması doğrulanmadı. Desteksiz sürüm/yüzey bilinmiyor döner. Yerel kaydın bulunması uzaktaki erişim, abonelik, sohbet veya Headroom trafiği kanıtı değildir. Yeni login/model isteği ve CI başlatılmadı; Linux ertelemesi korundu. Geliştirme token tasarrufu ölçülmedi.
