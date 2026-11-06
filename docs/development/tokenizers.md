# Yerel tokenizer sözlükleri

H029 sırasında gerçek motorun ilk token sayımında eksik tiktoken sözlüklerini beklediği görüldü. Sabitlenmiş Headroom yükleyicisi her kodlamada10s bekleyebiliyor; tek soğuk tanı isteği20,080s sürdü. Bu sırada mevcut5s sağlık gözlemi eskidiği için aktif motor düşebiliyordu. Bu bir hız karşılaştırması veya genel gecikme garantisi değildir.

Çözüm süreleri artırmak yerine dağıtılan motorun veri bağımlılığını tamamlar. `runtime/desktop_controller.py` içindeki altı public URL/SHA256 çifti, sabitlenmiş tiktoken paketinin `tiktoken_ext/openai_public.py` dosyasından alınır. Cache dosya adı tiktoken'ın SHA1(URL) kuralını izler; içerik bütünlüğünü SHA256 belirler. Veri `work/proofs/packaging/artifacts/headroom-engine/runtime/tiktoken-cache` altında kalır, Git'e veya kullanıcı profilindeki cache'e yazılmaz.

Proje kökünde geliştirici/paketleme hazırlığı:

```text
python scripts/prepare-tokenizers.py
```

İlk hazırlık yalnız sabit public veri dosyalarını HTTPS üzerinden indirir; mevcut doğrulanmış dosyayı yeniden indirmez, bozuk mevcut dosyayı sessizce değiştirmez. Her dosya10MiB ile sınırlıdır ve doğrulanmış geçici dosya atomik taşınır. Sonunda paketli Python ile yedi yerleşik kodlama, socket bağlantıları yasaklanmışken gerçekten yüklenip bir metni tokenize eder. Toplam altı kaynak dosya8.465.407bayttır; bunlar tokenizer verisidir, model ağırlıkları değildir.

Hazırlanmış cache'i ağ indirmeden kontrol etmek için:

```text
python scripts/prepare-tokenizers.py --check
```

Her yeni engine sürecinden önce helper bütün dosyaları doğrular ve yalnız o sürecin `TIKTOKEN_CACHE_DIR` değerini bu pakete bağlar. Eksik/bozuk veri hazır motor olarak sunulmaz. Kullanıcının model, reasoning veya auth seçimi; coding/cache profili; sağlık/stop bütçeleri değişmez. Bu kabul tiktoken'ın yedi yerleşik kodlaması içindir; bütün sağlayıcıların/model ailelerinin tokenizer desteği iddiası değildir.

İlk hazırlık/çevrimdışı smoke kanıtı `work/proofs/tokenizers/prepare-95e27441-9279-49bc-bb90-c3d5e8c58008.json` içindedir; kaynakların önce/sonra hash'leri ve altı artifact checksum'u kayıtlıdır. Dağıtılan installer/runtime paketine bu alt klasörün ve mevcut üçüncü taraf lisanslarının alınması H038/H041 kapısında ayrıca doğrulanır. Son kullanıcıdan bu komutu çalıştırması beklenen bir kurulum akışı tasarlanmadı; mevcut ürün hâlâ yerel geliştirme adayıdır.
