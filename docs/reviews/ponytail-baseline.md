# DEV002 — mevcut kodun ilk Ponytail denetimi

26 Ekim 2026. Kaynak başlangıcı `0a031d2`. İnceleme, `ponytail-audit` ve proje kabul sözleşmesine göre ana ajan ile mevcut tek Luna çalışan tarafından yapıldı. Ürün kaynağı, ayarlar ve testler değiştirilmedi.

## Bu depo ne yapıyor?

Headroom Desktop, yerel bir motorun yaşam döngüsünü ve desteklenen kodlama istemcilerinin proxy bağlantılarını yönetiyor. Yapılandırmayı önizleyip uyguluyor; kendi değişikliğini sahiplik kaydı üzerinden geri alıyor. Varsayılan yük tek bilgisayardaki tek kullanıcıdır; eşzamanlı istemci/alt ajan bağlantıları, birden fazla uygulama süreci ve istemcinin ayarını sonradan değiştirmesi değerlendirmeye dahildir. Relay varsayılanı 32 eşzamanlı TCP bağlantısıdır; bu sınır HTTP istek sayısı değildir.

## Bulgular — düşük öncelikli belge düzeltmesi

### 1. DEV002-D01 — README güncel geliştirme durumunu anlatmıyor

**Kaynak:** `README.md:7`, `README.md:17`; karşılaştırma `TASKS.md` durum satırı ve `package.json` içindeki `scripts.test:app`.

- **Ne yapıyor:** Yeni geliştiriciye mevcut ürün durumunu ve çalıştıracağı kaynak kontrollerini anlatıyor.
- **Sorun:** Current state bölümü 22 Ekim'in tarihsel H018/H019 görüntüsünde kalmış; bugün H021 kabul edilmiş durumda. Ayrıca “all six app test files” diyor, mevcut komut dokuz `.test.mjs` dosyası içeriyor. Eski tarih açıkça yazılı olduğu için tarihsel kayıt yanlış değil; güncel giriş özeti eskimiş.
- **En küçük çözüm:** H022'nin belge güncellemesinde güncel durumu TASKS/checkpoint'e bağla. H019'un sınırlı/amber sonucunu temiz sağlayıcı başarısına dönüştürme. Test sayısını sabitlemek yerine komutun kapsamını anlat. İlk CI koşusunun tarihsel 49-test sonucu ayrı korunmalı.
- **Atlanırsa:** Yeni geliştirici tamamlanan işi başlamamış sanabilir ve kaynak kapısının kapsamını yanlış anlayabilir. Üründe veri kaybı veya çökme kanıtı değildir.

**Güven:** Yüksek; dosya içerikleri ve komuttaki mevcut dokuz dosya doğrudan karşılaştırıldı. **Takip:** H022'ye bağlandı; bu denetimde düzeltilmedi.

İncelenen öncelikli akışlarda yeniden üretilebilir veya kaynakta kesinleşmiş yeni bir çökme, veri kaybı ya da sahiplik ihlali doğrulanmadı. Bu, ürünün hatasız veya yayına hazır olduğu anlamına gelmez.

## İzlenen akışlar ve doğrulama düzeyi

| Alan | Gerçekte incelenen yol | Sonuç / sınır |
| --- | --- | --- |
| Config Apply | `ConnectDialog.applyPlan` → TS action client → Tauri `apply_connection_plan` → `PlanManager` → config + ownership transaction | UI plan kimliği/fingerprint gönderiyor; native önkoşullar ve transaction sahiplik sınırları incelendi. Gerçek kullanıcı config'i açılmadı. |
| Undo ve sahiplik | `undoOwnership` → `undo_connection` → sahiplik revizyon/fingerprint kontrolü → transaction seal/commit | Config ve sahiplik birlikte ele alınıyor. `connection_actions.rs:879` ve `transaction_fs.rs:350` çevresindeki yeniden doğrulamalar okundu; yanlış başarıya giden somut yol bulunmadı. |
| Tercihler | `App.changePreferences` → preference komutu → `PreferenceStore` / `save_document` | Revizyon ve içerik fingerprint'i, eşgüdümlü süreç kilidi, sınırlı dosya okuma ve temp/replace yolu incelendi. Kilide uymayan dış yazıcı için atomik CAS garantisi yok; aşağıdaki sınır geçerli. |
| Runtime | Tauri start/stop/exit → `RuntimeManager` actor → `ManagedIngress` route/drain | Aktif nesil, cleanup-belirsizliği, kapanış ve yeniden başlatma akışları okundu. Windows süreç/Job Object/manifest-launch yardımcılarının tamamı yeniden denetlenmedi. |
| Relay | accept/register → değişmez backend hedefi → peer kontrolü → iki yönlü sınırlı buffer → drain/shutdown | Bağlantı sınırı, backpressure, half-close, zaman aşımı ve payload replay yapılmaması incelendi. Yeni trafik/performans testi çalıştırılmadı. |
| Arayüz | `App`, `Applications`, `ConnectDialog`, `ConnectionStatus` eylem/sequence/mutation yolları → TS köprüleri → native kayıtları | Geciken yanıt ve mutation kilitleri okundu. Görsel tasarım, tüm JSX/copy satırları veya bütün decoder kombinasyonları için eksiksiz UI denetimi iddiası yok. |

CodeGraph `ManagedIngress`, `RuntimeManager.stop_engine` ve çağıranları keşfetmek için kullanıldı. İlişkiler kaynakta okundu; grafın “test yok” özeti test yokluğu kanıtı olarak alınmadı. Bilinen dosyalarda `rg` ve dar kaynak okuması kullanıldı.

## Var olan görevlerde kalan konular

- **H022:** Claude proje klasörünü güvenli oluşturma, proje seçimini hatırlama ve seçici başlangıç/fallback senaryoları zaten planlıdır. Yeni denetim hatası olarak tekrar sayılmadı. DEV002-D01 belge güncellemesi bu göreve bağlandı.
- **H023 / H025 / H026:** Önceki H021 Chat fixture'ındaki `original=2306` ile sağlayıcının sentetik `optimized=12` değeri aynı ölçüm temeli değildir. Metrik normalizasyonu ve gerçek platform/model/ajan gösterimi mevcut görevlerde kalır; bu denetimde tasarruf türetilmedi.
- **H008:** H019'un gerçek cevapla birlikte sınıflandırılmamış ek CLI hatası verdiği tarihsel amber sonuç açık kalır. Burada yeni sağlayıcı denemesi yapılmadı ve sonuç temiz PASS'a çevrilmedi.

## Bulgulaştırılmayan sınırlar

`state/store.rs:547` son fingerprint kontrolü ile `:566` replace arasında kilide katılmayan bir dış yazıcı için teorik bir aralık vardır. Ancak `docs/proofs/preferences-store.md:7` garantiyi açıkça eşgüdümlü süreçlerle sınırlar. Mevcut dış-edit testi değişikliği save başlamadan önce yapar (`store_tests.rs:350`). Bu nedenle normal tema/dil kullanımında doğrulanmış kayıp veya güvenlik açığı diye raporlanmadı. Garanti ileride genişletilirse ayrı tehdit modeli ve gerçek yarış testi gerekir.

Benzer şekilde path/reparse kontrollerinden yalnız genel bir yarış hipotezi çıkarmak, kanıtlanmış proje dışına yazım bulgusu sayılmadı. Güvenlik kontrollerini silerek satır azaltma önerisi yapılmadı. Somut gereksiz bağımlılık veya güvenle silinebilecek kod saptanmadığı için satır/bağımlılık tasarrufu tahmini verilmedi.

## Kanıt ve kontrol kapsamı

Config/state incelemesinin ana dosyaları: `transaction.rs`, `transaction_fs.rs`, `connection_actions.rs`, `config_io.rs`, `config_document.rs`, `state/ownership.rs`, `state/store.rs`. İlgili transaction/action/config/ownership testlerinin sözleşmeleri ve tercih deposunun ilgili test bölümleri incelendi; tüm test gövdeleri satır satır yeniden okunmadı.

Ana ajan runtime tarafında özellikle `ingress_relay.rs` accept/forward/drain yollarını ve `manager.rs` içindeki ManagedIngress/RuntimeManager ve actor bölümlerini (2963–3314, 3338–3632, 3722–4111); UI tarafında bileşenlerin eylem ve effect bölümlerini okudu. Manager ownership/cleanup, relay generation/half-close/drain ve TS sequence testlerinden ilgili bölümler kontrol edildi. Kaynak okumaları yeni test çalıştırması değildir.

Yerel metadata: `work/proofs/ponytail-baseline/audit.json`; kaynak/test/config fingerprint'leri, README karşılaştırması ve önceki gate girdisi eşleşmeleri burada kayıtlıdır. Ürün kodu değişmediğinden önceki başarılı testler ve tam ürün gate'i tekrarlanmadı. Yeni native/UI/sağlayıcı başarısı iddia edilmiyor.

**Karar:** DEV002 denetim raporu tamamlandı. Mevcut H008/H022/H023 takipleri korunarak H022'ye geçilebilir; DEV002-D01 küçük belge düzeltmesi H022'dedir. Görev sonrası durma kuralı geçerlidir.

**Kontrol edilmedi:** Yeni canlı OAuth/API-key sağlayıcı çağrısı, gerçek başka sohbet/alt ajan trafiği, yeni native GUI oturumu, temiz kurulum ve installer/uninstaller, Windows 11/macOS/Linux, tüm paketleme/CLI helper'ları, üçüncü taraf motor/bağımlılık kaynaklarının tamamı veya güncel güvenlik duyuruları. İnceleme öncelikli yerel akışların kaynak denetimidir; bütün bu alanların kabulü değildir. Geliştirme token tasarrufu ölçülmedi.
