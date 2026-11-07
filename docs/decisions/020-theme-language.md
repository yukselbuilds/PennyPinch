# H032 — tema ve iki dil son kabulü

7 Kasım2026, tamamlandı;31/51. Ana ajan kalıcı kayıt eşzamanlılığı, uygulama/inceleme ve kabul sahibi; tek mevcut Luna yalnız sınırlı salt okunur metin/locale denetimi yaptı. Ponytail full, DESIGN.md ve Emil rehberi uygulandı. H033 başlamadı; görev sonunda duruldu.

## Kapsam ve korunacak davranış

Dört ana ekranın TR/EN ve açık/koyu birleşimleri, sistem temasını izleme ile manuel seçimin önceliği ve yeniden açılışta hatırlanması doğrulanır. Tarih/sayı/işaretli yüzde/duration, bilinmiyor ile0 ayrımı, uzun metin ve dialog/odak/taşma incelenir. Mevcut bileşen içi iki-dilli sözlükler veya tema token'ları sırf görev dosya önerileri için taşınmaz; yeni i18n/framework/dependency eklenmez. Yalnız somut eksik varsa en küçük bağlı düzeltme yapılır. Model/auth/ölçüm semantiği, OS görünüm ayarı, başlangıç kaydı ve H031 çalışma tercihleri değiştirilmez.

Native kabulde somut açılış yarışı bulundu: Overview/Activity ölçüm okumasının retention tercihi ile App'in dil/tema okuması aynı PreferenceStore klonlarında named mutex'i eşzamanlı ister; sıfır beklemeli OS kilidi birine busy verir. Aynı store'un read/save çağrıları paylaşılan std Mutex ile sıraya alınır; Windows cross-process kilidi, CAS/atomic replace ve hata sözleşmesi değiştirilmez. Store'dan runtime/metrics'e geri çağrı yoktur; kilit guard'ı reply dönmeden bırakılır. Kayıt varsayılanla ezilmez, retry veya süre sınırı gevşetilmez. Ana ajan düzeltmeyi ve4×16 paralel okuma regresyonunu üstlendi.

## Kabul

1. Gerçek App + izole transport: her tema/dilde Genel bakış/Uygulamalar/Etkinlik/Ayarlar, veri/boş/hata ve unknown durumları;1280/1024 ve dar819/683/507 viewport. Locale sayısı/yüzdesi/tarihi gerçek Intl çıktısıyla karşılaştırılır; negatif ve0 korunur. Ana kontroller ve uzun metin taşmaz. Kapsam fixture'dır, gerçek trafik/savings sayılmaz.
2. Sistem açık→koyu değişince system seçimi izler; manuel dark/light seçimi değişmez. Başarısız/stale tercih cevabında kalıcı başarı iddiası olmaz. Root lang/theme doğru güncellenir. Headless media taklidi OS ayarı değişikliği değildir.
3. Native son derlemede gerçek tema/dil seçimi, ana ekran gezinmesi, kapanış/reopen ve kaydedilmiş tercihle eşleşme; normal preferences/runtime/metrics state'i izole alandan sonra geri yüklenir. Gerçek kullanıcı startup/client/auth ayarı yazılmaz. UI input yalnız Sky; IPC okuması gereken en az metadata'yı kaydeder. Windows10 yerel, Windows11 önceki ücretsiz istisnayla not_tested. Gerçek OS DPI/diğer OS/installer sonucu çıkarılmaz.

Kanıt `work/proofs/appearance/`, rapor `docs/proofs/theme-language.md`, inceleme `docs/reviews/H032.md`. Sonuç+güncel kaynak/config/derleme+ortam fingerprint'i aynı doğrulama adımında programatik tutulur. Değişmeyen H031 kaynak/test/config, araç ve artefact eşleşiyorsa PASS yeniden kullanılır; somut ürün değişikliği varsa etkilenen odaklı kontroller ve son aday WORKFLOW.md kapısı yenilenir. Kabul bitince TASKS/checkpoint güncellenir, kullanıcıya bildirilir ve durulur. Ölçülmeyen geliştirme tasarrufuna yüzde verilmez.

Sonuç:48 UI ekran/durum ×5 genişlik ve locale/system/manual/failure PASS; üç native oturum ve iki yeniden açılış PASS. Yarışın kırmızı testi düzeltme öncesi FAIL,12 store testi ve son tam kapı153 frontend/363 Rust PASS (8 opt-in ignored).26 kontrol171 kaynak/config ile son artefact/ortamı bağladı. Ayrıntı [kabul raporunda](../proofs/theme-language.md); gerçek OS tema değişimi, ikinci OS ve installer kanıtı değildir.
