# 014 — Model, istemci ve ajan ataması

5 Kasım 2026. H026 etkin;23/51 aynı. Ana ajan veri/provenance, origin eşleme, kalıcı uyumluluk ve kabul sahibidir. Sabit sözleşmeli sunum işi mevcut tek Luna'ya devredilebilir. H025 toplamı ve güven sınırları korunur.

## Kaynak ve kapsam

Pinned0.41.0 `/stats?cached=1` recent_requests/request_logs satırında request_id, provider ve model vardır. `server.py::_classify_agent_from_log`, tags/model/provider üzerinden uygulama tahmin eder; bu sonuç kimlik veya rota kanıtı değildir ve alınmaz. Raw tags, headers, prompt_cache_key, sohbet dosyası veya hesap/auth okunmaz. Raw agent/client/role alanları güvenilir witness sayılmaz. Mevcut relay byte akışını incelemez; bir açık bağlantı bütün isteklerin istemcisini kanıtlamaz.

Model/provider yalnız sınırlı gözlenen etikettir. Native kaynağın aynı runtime+proxy origin için sahipli rota kanıtı varsa istemci, opaque upstream ve mevcutsa opaque ajan/ebeveyn kimliği ve açık rol taşınabilir. Model adından role/istemci/sağlayıcı URL'si çıkarılmaz. Pinned adaptör bu route witness'ını üretmediğinden gerçek istemci/ajan alanları unknown kalır; bu eksiklik optimizasyon filtresi değildir. Farklı upstream aynı model olsa bile ayrı kalır. H051 bu istek-kökeni/opaque kimlik sözleşmesini kullanabilir; sohbet/hesap listeleme burada başlamaz.

## Uygulama sözleşmesi

- H023 observation/event'e varsayılan unknown attribution eklenir; schema1 eski payload'lar okunur. SQL tablosu/DB sürümü değişmez. Mevcut4KiB event/128MiB depo ve bütün pin/transaction korumaları sürer. Renderer yazma IPC'si açılmaz.
- Rota kanıtı origin_id'ye bağlıdır; ham upstream/ajan kimliği saklanmaz. Native witness içindeki kimlikler runtime kapsamında hash edilir. Etiket boyutu/karakteri ve kimlik korelasyonu doğrulanır. Hata/payload/secret geri yansıtılmaz.
- Attribution zenginleşmesi veya çelişkisi token sayacını bozmaz. Çelişen metadata absorbs unknown olur; bir sonraki replay ile yeniden doğrulanmış sayılmaz. Eski unknown kayıt aynı origin'in yeni gerçek metadata'sıyla zenginleşebilir; sayaçlar ve ilk izleme oturumu değişmez.
- Dönem sorgusu aynı event kümesinden client/provider/model/upstream/agent grupları çıkarır. Aynı origin'in katman event'leri bir istek kimliği olarak gruplanır; rol modelden türetilmez. En çok64 görünür grup; fazla gruplar tek açık diğer satırında toplanır, veri sessiz düşmez. Miktar/% H024 toplam sözleşmesiyle aynı temeldedir; unknown sayaç temeli yükseltilmez.
- Kapsam durumları yalnız gözlenen alt kümede unknown/partial/verified; tüm bilgisayar/hesap/ajan trafiği kapsandı anlamına gelmez. Proxy dışında kalan upstream için sıfır istek veya optimizasyon başarısı üretilmez. İstemci bağlantı kaydı attribution'a dönüştürülmez.
- Graphite'da kısa model/kapsam tablosu, bilinmeyen atama ve açık kaynak notu; hesap, sohbet, maliyet veya model seçici eklenmez. Veri yoksa ikinci boş panel üretilmez.

## Kabul ve doğrulama

1. Aynı modeli kullanan iki farklı kanıtlı ajan aynı kimliğe düşmez; ayrı upstream ve eksik rol korunur. Sadece model/tags/client iddiası native proof üretmez. Metadata çatışması token toplamını değiştirmez.
2. H025 owned HTTP → pinned adaptör → SQLite → aynı dönem/attribution → readonly IPC/strict TS/UI çalışır. Eski payload, replay/reopen, dönem, sınır/diğer satırı, yanlış origin, canary/safe-label kabulü test edilir. Native/proxy empty kaynak kanıtı, sayısal fixture veya gerçek sağlayıcı testi gibi sunulmaz.
3. UI unknown/partial/verified ve iki aynı model/ayrı ajan durumları; TR/EN, dar pencere ve klavye kabulü. Odaklı RED/GREEN, tek birleşik root inceleme, son adayda tam yerel kapı ve native kontrol.123 H025 girdisinin geçerli PASS'i compact/yeni gün nedeniyle tekrarlanmaz; değişen girdiler yeni adayda doğrulanır. H026 kapanınca durulur; H027 başlamaz.
