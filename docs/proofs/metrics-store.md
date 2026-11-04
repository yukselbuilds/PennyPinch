# H024 — kalıcı metadata ve dönem sorgusu

4 Kasım 2026, Windows10 22H2 x64. Ana ajan kalıcı kayıt/ölçüm semantiğini, uygulamayı ve kabul incelemesini üstlendi. Mevcut tek Luna xhigh yalnız ağırlıklı toplam test dosyasını yazdı; karışık temellerin taşma için de birleştirilmemesi tek düzeltme paketiydi. Sonra çalışan durdurulup tamamlanmış durumu doğrulandı; ana ajan sınır regresyonlarını ekledi. Geliştirme token tasarrufu/süre ölçülmedi.

## Kapsam

- H023 normalize event → guarded SQLite transaction/upsert → yerel dönem sorgusu → gerçek native IPC. UI, canlı motor polling'i veya yeni model isteği açılmadı. [Karar012](../decisions/012-metrics-store.md), [API](../contracts.md#h024-kayıt-ve-sorgu-apisi).
- Her event_id bir kez; proxy/MCP replay kaynakları birleşir, çelişki ölçülemez kalır. Origin/slot kimliği tekrar doğrulanır. İlk izleme oturumu ve ilk görülme zamanı değişmez. İzleme oturumu hesap/sohbet kimliği değildir.
- Bugün ve bugün dahil7 yerel takvim günü; UTC saklama ve sorgu anında yerel sınır.30 gün metadata retention native yanıtta görünür. Grup ayrıntısı64 ile sınırlı; farklı temel/bileşenler birleşik yüzde olmaz.
- Gerçek sıfır, eksik, negatif, sıfır taban ve JS sınırı taşması ayrıdır. Rolling/aggregate/kimliksiz/zamansız kaynaklar kapsamda görünür; tam trafik varsayılmaz.

## Doğrulama

| Senaryo | Gözlenen |
| --- | --- |
| Ağırlıklı toplam | Sentetik1.000→500 ve9.000→8.100 girdisi1.400/%14 verir. Bu gerçek kullanıcı tasarrufu değildir. |
| Replay/restart/çatışma | Aynı köken bir kayıt; farklı runtime nesilleri ayrı; çelişen tekrar iyileşmiş replay ile tekrar measured olmaz. |
| Ayrı süreçle yeniden açılış | Bir test süreci SQLite'a yazıp exit0 verdi; ikinci süreç aynı dosyadan toplamı okudu. Yeni izleme oturumu0 event, korunmuş dönem toplamı1.400/%14. Test verisi gerçek uygulama state'ine eklenmedi. |
| Yerel sınırlar ve kapsam | UTC+3 gece yarısı/7 takvim günü, gelecekteki fractional timestamp dışlama, UTC yerel bölgesine değişimde yeniden seçim; kısmi toplam açık. |
| Kalıcı kayıt hataları | Yabancı/future schema ve hardlink reddi, değiştirilmiş event_id/batch reddi, busy, ikinci SQL yazımında tetiklenen hata ve tüm transaction rollback; mevcut veri korunur. |
| Retention ve sınırlar |30 günlük eşik, eski replay'in yeniden alınmaması, daha sonraki sorguda temizleme ve128MiB üstü dosyanın SQL'den önce reddi. Bu yük/performance benchmarkı değildir. |
| İçeriksiz legacy adaptörü | Gerçek H023 projeksiyon yolu2306/12 temelini unknown bırakır; prompt/auth canary SQLite'a geçmez. Aggregate lifetime ve kimliksiz satırlar tasarruf olmaz. |

İlk RED12 mevcut PASS/17 yeni FAIL; ardından29 odaklı PASS. İnceleme regresyonları önce başarısız, düzeltmeden sonra34 PASS; ayrı süreç kanıtıyla36 odaklı PASS. Derleme/API ve bir test fixture'ı tür hataları düzeltildi; ham hata çıktısı saklanmadı.

Son kaynak adayında tam yerel Rust kapısı bir kez: **313 PASS/5 opt-in ignored**, rustfmt check ve offline embedded build PASS. `work/proofs/metrics-store/final.json`108 girdinin (107 kaynak/test/config + doğrulama yardımcısı) önce/sonra fingerprint'ini ve araç/ortamı tutar. Frontend kaynak/test/config ve dist değişmedi;98 frontend testi ve önceki build kanıtı ilgili girdi/Node/pnpm fingerprint eşleşmesiyle yeniden kullanılır, tekrarlanmaz.

## Gerçek native sorgu

Yeni built Headroom iki kez ayrı WebView profili ve sahipli Job ile açıldı. Her açılışta gerçek `get_metrics_summary` IPC'sinde session/today/seven_days sorguları doğru DTO, aynı süreç içi izleme kimliği ve retention30 döndürdü. Geçersiz schema sabit hata verdi. İkinci açılışın kimliği farklıydı. İki native pencere normal WM_CLOSE→exit0; Job artıkları ve iki debug listener kapandı. Normal Codex config, uygulama preferences/ownership/onboarding ve metrik DB durumu hash'leri korundu. Motor/model başlatılmadı.

Kanıtlar `work/proofs/metrics-store/` altında `focused4.json`, `final.json`, `native-query.json`, süreç fixture'larının UUID dizinleri ve son `acceptance.json` içindedir. Kaynak/artefakt/ortam fingerprint'leri aynı doğrulama adımlarında programatik kaydedildi. Ham auth, konuşma veya hata dökümü yoktur.

H024 backend kabulü tamamlandı. H025 owned runtime metadata toplayıcısı, polling/gap ve gerçek ekran bağlantısını ayrıca yapacak; bu kabul canlı tasarruf dashboard'u veya güvenilir0.41.0 tokenizer provenance'ı oluşturmaz. Native kontrolde boş depo sorgulandı; dolu geçmişin ayrı süreç kanıtı yukarıdaki üretim depo koduyla ve açıkça sentetik fixture'la sağlandı. Windows11/macOS/Linux, installer, güç kesintisi ve uzun süre/yük ölçümü yapılmadı. [Ana ajan incelemesi](../reviews/H024.md).
