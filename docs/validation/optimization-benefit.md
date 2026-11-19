# H041 — Optimizasyon ve gecikme

10 Kasım 2026. Windows 10 x64 build 19045 yerel kabulü geçti. H041’in canlı API-key ve Windows 11 doğrulamaları açık. [Plan](../decisions/027-optimization-regression.md), [güncel yerel kanıt](../proofs/optimization-local-acceptance.json).

`bench/optimization/replay.py --run` proje içindeki sabitlenmiş CPython 3.13.13 ile çalışır. Geliştirme manifestinin işaret ettiği payload ve hazırlanmış tokenizer verisi gerekir. `--self-check` dört fixture’ın çağrı kimlikleri ve geçmiş ekleme yapısını ağ çağrısı olmadan denetler. `--run`, ölçüm ve sağlık isteklerini loopback fixture’a yönlendirir. Kopyalanan test katmanında Hugging Face ve Transformers dosya indirmeleri kapalıdır. Sıfır model cache yükü ve parmak izi kontrolü kaydedilir. Ağ paketi izlemesi yapılmadı.

Üç yeni controller/helper/engine süreci ve her örnekte üç konuşma turu kullanıldı. Aynı 36 iş yükü doğrudan loopback kontrolüne ve Headroom’a gönderildi. Model, reasoning ve sentetik auth korundu. Kaynak kodu ve kesin sayısal JSON `Read` aracında aynen kaldı. İletilmiş geçmiş prefix’i ve araç şemaları yeni turlarda eşleşti. CCR özgün logu döndürdü.

| Fixture | Headroom isteği | Yerel token azalması |
| --- | --- | --- |
| Kaynak kodu | 9 | %0, içerik korundu |
| Kesin sayısal JSON | 9 | %0, içerik korundu |
| Tekrarlayan sentetik log | 9 | %98,15–98,23 |
| Kısa JSON | 9 | %0 |

Tokenlar iki kolda aynı `o200k_base` kodlamasıyla tam sıralanmış JSON üzerinden sayıldı. Eklenen araç şemaları dahil edildi. Bu örnek sonucu sağlayıcı cache indirimi veya fatura tasarrufu olarak sunulmaz. Gerçek model TTFT’si ve yanıt kalitesi ölçülmedi. İlk fixture içerik delta süresi, toplam süre ve CPU/RAM her normal istek kaydında var.

Güncel koşuda ısınmış ek p95 gecikme 72,45 ms, istek CPU p95 78,125 ms. Gözlenen motor özel belleği en çok 855,67 MiB. Önceden belirlenen 1000 ms, 2000 ms ve 1536 MiB sınırları geçti. Bellek 50 ms aralıkla ve istek sınırlarında gözlendi. CPU kapsamı benchmark/fixture ve ilgili controller/helper/engine süreçleridir. Rust ingress ayrı ölçüldü.

İlk koşu 24 normal isteği tamamladıktan sonra hata testinin tek upstream çağrısı varsayımında durdu. Sabit ProxyConfig 429 için üç deneme tanımlar. Koşucu bunu doğrulayacak ve gecikmesini kaydedecek biçimde düzeltildi. Son koşuda doğrudan kol 1, Headroom kolu 3 denemeyle 429 ve Retry-After değerini korudu. Headroom hata yanıtları yaklaşık 2,02–2,07 s sürdü. Eşikler değiştirilmedi. [İlk Responses ölçümü](../proofs/optimization-benefit.json).

Üç süreç normal/graceful kod 0 ile kapandı, portlar kapandı. Ham istek, auth, kontrol sırrı ve stderr dökümü raporda tutulmadı. Stderr içeriği bilinmiyor olarak işaretlendi. Dış sağlayıcı çağrısı 0. Diğer yerel protokol ve relay sonuçları aşağıdadır. Gerçek API-key ve dış işletim sistemi kontrolleri açık.

## Relay ve okumayan istemci

`bench/optimization/backpressure.py` aynı 4 MiB veriyi doğrudan TCP ve Rust relay kolunda sınadı. İstemci alım tamponu 2048 bayt, okumadığı süre 30 s. Önceki 180 ms delayed-reader testi kendi kapsamıyla korundu. Yeni sayaçlar yalnız test derlemesinde, relay’in istemciye yazdığı noktadaki gerçek WouldBlock ve yazılan baytları kaydetti.

10/20/30 s örneklerinde son iki aralık boyunca WouldBlock arttı ve aktarım bitmemişti. İstemci okumaya başladığında iki kolda da 4 MiB SHA-256 eşitliği ve cleanup geçti. Kaynak, plan ve test binary’si değişmedi. [Backpressure kanıtı](../proofs/relay-backpressure.json).

Native test süreci dışarıdan örneklendi. Son tekrarda relay kolunda 46,875 ms CPU ile yaklaşık 30 s boyunca tek çekirdeğin %0,156’sı ve gözlenen özel bellek artışı 0 ölçüldü. Okumaya başlanmasından EOF’a kadar doğrudan kol 174,97 ms, relay kolu 143,71 ms sürdü. Bu süreler aynı makinedeki tek bir çiftin gözlemidir. Önceden belirlenmiş %10 CPU ve 32 MiB büyüme sınırları geçti. Ölçüm tek bağlantı ve test derlemesi kapsamındadır. [İlk backpressure ölçümü](../proofs/relay-backpressure.json).

## Diğer protokoller

`bench/optimization/surfaces.py --run` üç yeni motor süreciyle aynı dört içerik/üç turu Chat HTTP/SSE, Anthropic HTTP/SSE ve Responses WebSocket üzerinden çalıştırdı. 108 eşlenmiş iş yükünde 216 istek geçti. Kaynak/sayısal içerik, araç kimlikleri, model/reasoning, sentetik auth ve iletilmiş geçmiş prefix’i korundu. Bulunan CCR işaretlerinin özgün çıktıları geri alındı.

| Yüzey | Sentetik log token azalması | Isınmış ek p95 | Isınmış CPU p95 |
| --- | --- | --- | --- |
| Chat HTTP/SSE | %24,64–49,34 | 60,18 ms | 62,5 ms |
| Anthropic HTTP/SSE | %16,24–24,33 | 48,93 ms | 46,875 ms |
| Responses WebSocket | %98,15–98,23 | 13,60 ms | 31,25 ms |

Tablodaki log örnekleri farklı protokol zarflarıyla taşınır; sonuçlar bütün işlere uygulanacak genel tasarruf oranı değildir. Ek araç şemaları tam JSON token hesabına dahildir. Gerçek Anthropic tokenizer’ı/faturası, sağlayıcı cache isabeti, gerçek model TTFT’si ve yanıt kalitesi ölçülmedi. Anthropic kolunda istemcinin açık cache işareti sabit tutuldu. WebSocket konuşması aynı bağlantıda üç tur sürdü; açılış el sıkışması istek zamanlamasına dahil değil. Her yeni motorun başlangıç süresi ayrıca kaydedildi, işletim sistemi dosya cache’i sıfırlanmadı.

Küçük veya korumalı içerikte ek şemaların maliyeti de korundu. Aşağıdaki negatif azalma token artışı demektir; korumalı araç çıktısının değiştiği anlamına gelmez.

| Yüzey | Kaynak kodu | Kesin sayısal JSON | Kısa JSON |
| --- | --- | --- | --- |
| Chat HTTP/SSE | −%0,10…−%0,07 | −%0,07…−%0,05 | −%2,50…−%1,79 |
| Anthropic HTTP/SSE | −%1,30…−%0,87 | −%0,84…−%0,56 | −%27,93…−%20,49 |
| Responses WebSocket | %0 | %0 | %0 |

Kısa Anthropic örneğinde artışın tamamı `headroom_retrieve` araç tanımından geldi. Üç turlu loopback tekrarının girdi ve çıktı parmak izleri önceki dokuz kayıtla eşleşti. İletilen gövdeden bu tanım çıkarıldığında özgün istek birebir elde edildi. `o200k_base` tam JSON sayımı her turda 100 ek token gösterdi. İlk tur 358’den 458’e, son tur 488’den 588’e çıktı. [Alan bazında ölçüm](../proofs/anthropic-request-overhead.json).

Sabit motor varsayılan ayarla, araç listesi bulunan ve optimizasyona açık isteklerde bu aracı ilk turdan itibaren ekler. Sonradan araç listesini değiştirmek Anthropic’in önbelleğe aldığı ortak başlangıcı geçersiz kılabilir. Aracı ihtiyaç oluştuğunda ekleyen `lazy` ayarı kısa konuşmalardaki bu maliyeti azaltabilir. Uzun konuşmadaki önbellek etkisi canlı sağlayıcıyla ölçülmedi.

Üç motor normal kod 0 ile kapandı, portlar kapandı. Güncel koşuda 648 kaynak ve araç girdisi önce ve sonra eşleşti. İlk koşunun doğrudan WebSocket bağlantısı uyarısı, sabit kütüphanenin açık `legacy=True` seçimiyle giderildi. Bağlantı yaşam döngüsü kontrollü biçimde sonlandırıldı. [İlk protokol ölçümü](../proofs/optimization-surfaces.json).

## Anthropic CCR aracının eklenme zamanı

15 Kasım 2026’da `bench/optimization/ccr_policy.py --run` sabit 0.41.0 motorla dört yeni süreç başlattı. İki eşlenmiş tekrarda sıra ters çevrildi. Her süreç üç kısa tur, geç log ve korumalı sayısal Read içeren beş turlu oturum ve ayrı bir kısa istek gönderdi. Aşağıdaki sayılar iki tekrarda da aynıdır. `o200k_base` ile tam kanonik JSON sayımı, CCR tanımını da içerir. [Çalıştırma kaydı](../proofs/anthropic-ccr-policy.json).

| Oturum veya istek | Özgün | `eager` | `lazy` |
| --- | ---: | ---: | ---: |
| Üç kısa turun toplamı | 1.386 | 1.686 | 1.386 |
| Beş uzun turun toplamı | 29.376 | 13.560 | 13.260 |
| Uzun oturumda logun ilk geldiği tur | 10.321 | 2.263 | 2.263 |

Üç kısa turda `lazy` toplam 300 token, `eager` koluna göre %17,79 azalttı. İki ayar da logun ilk geldiği isteği özgüne göre %78,07 küçülttü. Geç log geldikten sonra `lazy` araç dizisi bir kez değişti ve bir sonraki turda sabit kaldı. `eager` dizisi baştan sabitti. Mesaj başlangıcı, sentetik auth, model/efor, araç çağrıları ve korumalı Read içeriği korundu. Ayrı oturum `lazy` aracını devralmadı. Model çağrısı gerektirmeyen sekiz yerel CCR isteği özgün logu birebir döndürdü. Dört motor kod 0 ile kapandı ve sahipli portlar serbest kaldı.

İlk koşuda fixture önceki sentetik cevabı yeni isteğe eklemediği için cache modu logu sıkıştırmadan iletti. İkinci koşu doğru geçmişle logu sıkıştırdı ve CCR için sağlayıcı tarafının `stream: false` olduğunu gösterdi. Son koşu o kola JSON, diğer kola SSE verdi. İstemci iki durumda da tamamlanmış SSE aldı. Ölçüm bu yerel protokol kontrolleriyle sınırlıdır. Sağlayıcı cache sayaçları, fatura etkisi ve gerçek görev başarısı açık. Ürün varsayılanı `eager` kaldı.

## Anthropic CCR ve kullanım kapsamı

15 Kasım 2026’da `bench/optimization/anthropic_usage.py --run`, sabit 0.41.0 motorunda 3 ayrı Anthropic Messages oturumu çalıştırdı. Dokuz kısa istek geçmişi kurdu. Sonraki üç istekte sağlayıcı fixture’ı `headroom_retrieve` çağırdı. Gerçek motor aynı test isteği için bir ek upstream isteği gönderdi ve özgün logu eksiksiz taşıdı. Toplam 12 istemci ve 15 upstream isteği gözlendi. Model, reasoning ve sentetik auth korundu. [Sayısal kayıt](../proofs/anthropic-usage-coverage.json).

| Ölçülen son istek | İstemci sonucu | Gözlenen upstream kullanım toplamı |
| --- | --- | --- |
| İki başarılı çağrı | SSE 200, son çağrı 1.343 token | 2.429 token, 300 önbellek dışı giriş + 2.000 okuma + 100 yazma + 29 çıktı |
| Birincil yanıtta önbellek yazma alanı eksik | SSE 200, son çağrıda yazma 0 | Yazma ve toplam bilinmiyor, çıktı 29 |
| Devam isteği 429, kullanım alanı yok | SSE 502, kullanım alanı yok | Toplam bilinmiyor |

Tam örnekte özel `/stats` satırı yalnız son çağrının dört sayacını gösterdi. Başarısız devamda satırda ilk çağrının sayaçları vardı. Doğrudan upstream fixture cevabı dışındaki bu iki görünüm bütün çağrıları kapsamaz. Fixture geçmişi ayrı oturum anahtarları ve tekil yeni satırla doğrulandı. Bu eşleme izole seri deneyin kapsamındadır. Ürün trafiğinde oturum sahipliği, gerçek kullanım ve fatura ayrı kabulde doğrulanacak. [Anthropic kullanım alanları](https://platform.claude.com/docs/en/build-with-claude/prompt-caching#tracking-cache-performance).

## Anthropic kullanımının motorda kaydı

`tests/runtime/anthropic_usage_capture.py` Windows 10 x64 üzerinde sabit 0.41.0 motorunun sahip olduğu istemciyi kullandı. 15 istemci isteği 19 upstream gönderime dönüştü. Her gözlem motorun verdiği istek kimliğiyle eşleşti. İlk yanıt ve CCR devamı birlikte 2.429 sentetik token olarak kaydedildi. Açıkça bildirilen 0 korundu. Bildirilmeyen cache yazma değeri, başarısız devam ve 429 sonrası başarılı tekrarda toplam bilinmiyor kaldı. İki eşzamanlı SSE isteği kendi sayaçlarıyla tamamlandı. [Doğrulama ve kaynak parmak izleri](../proofs/desktop-anthropic-usage.json).

Bölünmüş SSE çerçeveleri ilk ve son kullanım bilgilerini birleştirdi. Eksik terminal, bozuk çerçeve, boyut ve çağrı sınırı toplamı geçersiz bıraktı. İzlenen akış baytları aynı teslim edildi. Kontrol anahtarı eksik veya yanlış istekler 404 aldı. Yeniden başlatılan motorun kaydı boştu ve önceki anahtar geçersizdi. Kayıtlar yalnız motor neslinin belleğinde tutulur.

Mevcut istemci yanıtı ve `/stats` karşılaştırması ile üç yerel Anthropic duraklatma senaryosu geçti. Native Windows runtime odaklı test 35 başarılı, 6 opt-in test atlandı. Controller 10 test ve Rust format kontrolü geçti. Bu kabul özel motor kaydını kapsar. Masaüstüne yansıtma, canlı sağlayıcı, diğer protokoller ve Windows 11 doğrulaması açık.

## Anthropic kullanımı masaüstü istek ayrıntısında

Windows 10 x64 yerel deneyinde `tests/runtime/anthropic_usage_capture.py --native-reader` gerçek motoru açtı. İki upstream yanıtı, aynı nesil ve tam istek kimliğiyle, sahipliği doğrulanmış native ölçüm okuyucusundan SQLite ve istek ayrıntısına ulaştı. Kullanım alanları 2.400 toplam girdi, 29 çıktı, 2.000 önbellek okuma, 100 önbellek yazma ve iki gönderim denemesi olarak doğrulandı. Giriş sayım temeli ve tasarruf ölçümü bağımsız kaldı. [Doğrulama kaydı](../proofs/anthropic-usage-desktop-detail.json).

Yerel Rust ve istemci testlerinde yanlış nesil ve başka istek kimliği eşleşmedi. Başarısız yanıtta ve eksik sayaçta toplam bilinmiyor kaldı. Geçici eksiklik, önceki doğrulanmış alanı silmedi. Çelişkili sonraki veri, kullanım alanlarını bilinmiyor yaptı ve yerel sıkıştırma farkını korudu. Eski ayrıntı kayıtları yeni alanlar olmadan açıldı. 464 Rust, 223 frontend ve gerçek motor opt-in testi geçti. Windows 11 ve macOS çalıştırması, görsel native yürüyüş, diğer sağlayıcılar ve canlı maliyet kapsamı açık.

## Süreli yük ve başlangıçta Kompress yüklemesi

`bench/optimization/stability.py --run`, önce dört yüzey × dört içerik × üç tur × iki kol ile 96 isteği ısıttı. Ardından 300 saniye boyunca saniyede bir eşlenmiş iş yükü gönderdi. 300 çift/600 istek, kaynak/sayısal/araç/prefix/CCR kontrollerini tamamladı. CPU birleşik benchmark/fixture/controller/helper/engine kapsamıyla tek çekirdeğin %6,24’ü, ek p95 48,67 ms ve istek CPU p95 31,25 ms idi.

İlk koşuda ısınma sonrası gözlenen özel bellek artışı yaklaşık 605 MiB ile 128 MiB sınırını aştı. Tepe yaklaşık 1470 MiB, bitiş yaklaşık 1207 MiB idi. Normal çıkış 0 ve port kapanışı geçti. 646 girdi eşleşti. [Başarısız koşu](../proofs/optimization-stability-failure.json).

120 saniyelik tanıda izlenen içerik ve kayıt depoları yaklaşık 0,2 MiB düzeyinde kaldı. Artış motor sürecinde gözlendi. Sabit kaynakta model başlatıldıktan sonra Kompress’i arka planda yükleyen yol belirlendi. Headroom Desktop bu arka plan yüklemesini kendi motorunda kapattı. İstek gerektiğinde Kompress’i kullanma yolu korunur. Aynı 300 saniye planıyla sonraki koşuda özel bellek artışı 56,18 MiB, tepe 1138,81 MiB ve ek p95 49,88 ms ölçüldü. İki koşuda da ayrı Kompress dosyaları cache’e yazıldı. Bu gözlemler model dosyasının indirilme maliyeti ile yüklenme maliyetini tek başına ayırmaz. [Varsayılan motor ayarlarıyla düzeltme sonrası koşu](../proofs/optimization-stability.json).

0.41.0’da model dosyası indirmeleri kapalı koşu, aynı planla 300 çift, 600 istek ve 249 CCR geri alımı tamamladı. Artış 24,12 MiB, özel bellek tepesi 889,40 MiB, birleşik CPU tek çekirdeğin %4,09’u ve ek p95 47,04 ms oldu. Beş dakikalık pencere için önceden belirlenen bütün eşikler geçti. 653 girdi eşleşti. Motor normal çıkış kodu 0 ile kapandı ve port serbest kaldı. Hugging Face hub cache’inde model yükü sıfır bayttı. Sağlayıcı modeli, eşzamanlı çok istemci yükü ve saatlerce açık kalma ayrı kapsamda kalır.

## Headroom 0.44.0 ile süreli yük

18 Kasım 2026. Önceki 300 saniye planı ve eşikler güncel 0.44.0 pininde kullanıldı. Windows 10’da Responses HTTP/SSE, Chat HTTP/SSE, Anthropic HTTP/SSE ve Responses WebSocket üzerinde 96 ısınma isteği ve 300 ölçüm çifti tamamlandı. 600 ölçüm isteğinde kaynak, kesin sayısal içerik, araç kimliği ve prefix kontrolleri geçti. 249 CCR geri alımı birebir doğrulandı. 688 kaynak ve araç girdisi eşleşti. Motor kod 0 ile normal kapandı ve sahipli port serbest kaldı.

| Ölçüm | Gözlenen | Plan sınırı |
| --- | ---: | ---: |
| Ölçüm çifti | 300 | En az 250 |
| Isınma sonrası özel bellek artışı | 22,96 MiB | En çok 128 MiB |
| Gözlenen özel bellek tepesi | 914,59 MiB | En çok 1.536 MiB |
| Ek p95 gecikme | 46,14 ms | En çok 1.000 ms |
| Birleşik CPU, tek çekirdeğin yüzdesi | %3,32 | En çok %25 |

Aşağıdaki sayılar, her örneğin 75 ölçüm isteğinde aynı `o200k_base` kodlamasıyla sayılan tam JSON gövdeleridir. Ek araç şemaları sayıya dahildir. Değerler yerel sentetik trafikte ana isteğin boyutunu gösterir. [Programatik kanıt](../proofs/optimization-local-acceptance.json).

| Sentetik örnek | Özgün token | İletilen token | Fark |
| --- | ---: | ---: | ---: |
| Kaynak okuması | 724.026 | 725.970 | %0,27 artış |
| Kesin sayısal okuma | 1.114.776 | 1.116.720 | %0,17 artış |
| Tekrarlayan log | 3.110.226 | 1.079.854 | %65,28 azalma |
| Kısa JSON | 37.476 | 39.420 | %5,19 artış |

## H041 açık kabulleri

19 Kasım 2026’da **6 doğrulama ve 3 isteğe bağlı Ar-Ge kabulü** açık. Ana görev sayımı **45/54**.

| Kabul | Tamamlanması için gereken |
| --- | --- |
| Kısıtlı Codex native Windows dosya profili | Uygun native sandbox ile izinli okuma ve açık erişim reddi. Windows 10 üzerinde kullanılan eski sandbox profili başlatmadı. WSL2 aynı dosya sınırını doğruladı. [Dosya ve arşiv kanıtı](../proofs/targeted-query-comparison.json) |
| Windows 11 x64 | Gerçek Windows 11 x64 ortamında 0.44.0 paketli çalışma, kapanış ve trafik. Standart GitHub Windows 11 imajı ARM64. CircleCI, Windows Server imajları sunuyor. [GitHub standart runner’ları](https://docs.github.com/en/actions/reference/runners/github-hosted-runners#standard-github-hosted-runners-for-private-repositories) · [CircleCI Windows imajları](https://circleci.com/docs/guides/execute/using-windows/) |
| macOS ARM64 | Doğrulanmış ücretsiz kapasiteyle 0.44.0 paketli açılış, sahipli kapanış, trafik ve ölçüm. GitHub işi 15 dakikayla sınırlı. [Başlatma koşulları](#19-kasım--kalan-kabullerin-başlatma-koşulları) |
| Canlı Anthropic Messages | Yetkili hesapla gerçek Messages yanıtının kullanım sayaçlarını ilgili Headroom isteğine bağla. [Yerel kullanım kanıtı](../proofs/desktop-anthropic-usage.json) |
| Canlı OpenAI API-key | API-key rotasına yetkili erişim. H008’den devreden kapsamın kendi kullanım kanıtı. [H041-d planı](../decisions/027-optimization-regression.md#h041-d-responses-httpsse-kullanım-ölçümü) |
| Gerçek fatura farkı | Eşlenmiş koşuların karşılaştırılabilir fatura verisi. Token kıyası kaydedildi. [Canlı kıyas](../proofs/targeted-query-comparison.json) |
| Learn geniş fayda kabulü | Somut tekrar örüntüsüyle aynı model ve eforda eşlenmiş görevler. İlk canlı çift fayda göstermedi. [Learn ölçümü](../proofs/learn-merit-pilot.json) |
| Deneysel kurallar e3 | Donduruldu. Yeniden başlatmak için yeni aday ve önceden belirlenmiş kalite/net fayda kapsamı gerekiyor. [Ar-Ge kararı](../research/optimization-roi-review-2026-11-17.md) |
| Deneysel kurallar e4 | e3 geçince kapsamlı etkinleştirme, kapatma ve geri alma kabulü. [Deneysel kurallar planı](../decisions/027-optimization-regression.md#h041-e-deneysel-kurallar) |

### 19 Kasım — Kalan kabullerin başlatma koşulları

- **GitHub macOS.** 6531cc4 adayında `macos-package.yml` elle başlatılan `macos-15` işini seçiyor. Yerel ön kontrolde workflow, iki Bash adımı, üç Bash scripti ve altı Python kaynağı okundu. 230 girdinin önce/sonra özeti eşleşti. 0.44.0 pini doğrulandı. İş, ücretsiz kota ve sıfır harcama sınırı teyit edilince `verify_desktop_traffic=true` ile başlatılabilir. [Yerel ön kontrol](../../work/proofs/macos/h041-package-preflight-3f48e53e19.json) · [Akış](../../.github/workflows/macos-package.yml).
- **Kota ve süre.** GitHub CLI oturumu 19 Kasım’da doğrulandı. Özel deponun geçmiş koşuları okundu. Kullanıcı 0,10 USD kalan kotayı teyit etti. Hesap kullanım özeti için CLI kimliğinin `user` kapsamı yok. 0,10 USD bir tam Mac dakikasına karşılık geliyor. Önceki gerçek Headroom 0.41.0 paket ve Desktop trafik işi 5 dakika 39 saniye sürdü. Bugünkü tarifeyle 6 yuvarlanmış dakika, 0,372 USD dahil kota eder. Güncel 0.44.0 için 15 dakikalık iş limitinin tarife karşılığı 0,93 USD dahil kota. Yeni iş, yeterli ücretsiz kapasite teyidi bekliyor. [Önceki iş](https://github.com/Protocol272/headroom-desktop/actions/runs/43284385567) · [Süre kanıtı](../../work/proofs/macos/h041-remote-history-0136ac2a66.json) · [GitHub tarifesi](https://docs.github.com/en/billing/reference/actions-runner-pricing) · [Kota yenilemesi](https://docs.github.com/en/billing/concepts/product-billing/github-actions).
- **CircleCI macOS seçeneği.** Free plan özel depolar için ayda 30.000 kredi sağlıyor. Apple Silicon `m4pro.medium` dakikada 200 kredi. 15 dakikanın işlem karşılığı 3.000 kredi. Bu yol özel deponun CircleCI’ye açılması için ayrı yetki, hesap kotası ve native GUI ön kontrolü gerektirir. [Plan](https://circleci.com/pricing/) · [Tarife](https://circleci.com/pricing/price-list/) · [Apple Silicon imajları](https://circleci.com/docs/guides/execute/using-macos/).
- **Native Codex profili.** Aktif Codex sürecinin CLI sürümü 0.169.0-alpha.11.1 ve bu bilgisayar Windows 10 x64 build 19045. MXC için uygun güncel Windows 11 ortamı gerekiyor. Önceden hazırlanmış legacy elevated testi, `:root` okuma şartı nedeniyle komutu başlatmadı. [OpenAI sandbox uyumluluğu](https://learn.chatgpt.com/docs/windows/windows-sandbox#mxc-compatibility) · [Önceki koşu](../proofs/targeted-query-comparison.json).

Yerel ön kontrol raporu SHA-256 `38ce4294dd93fadb20762023699e96f197c362f61b73e3eb5908770d1dff4b63`. Native macOS kabulü erişim ve gerçek koşu bekliyor. Bu tarihte H041’in bütün kabullerine ortak bitiş tarihi verilemiyor.

### 18 Kasım erişim kontrolü

- Bu bilgisayar Windows 10 x64 build 19045 ve kurulu Codex CLI 0.169.0-alpha.11.1 kullanıyor. Önceki koşuda unelevated profil komutu başlatmadı ve MXC mevcut değildi. Ayrı ACL kontrolünün izinli okuma ve erişim reddi kanıtı kendi kapsamında geçerli. Adlandırılmış test profilinin iki dosya sınırı için uygun MXC veya yönetici tarafından hazırlanmış elevated sandbox gerekiyor. [Önceki koşu](../proofs/targeted-query-comparison.json) · [OpenAI Windows sandbox belgesi](https://learn.chatgpt.com/docs/windows/windows-sandbox#mxc-compatibility).
- GitHub CLI mevcut oturumu geçersiz bildirdi. Tarayıcıdaki fatura sayfasından hesap kotası alınamadı. macOS ARM64 koşusu için güncel dahil kota ve sıfır ücret sınırı doğrulaması açık. Standart Windows 11 runner etiketi ARM64. Windows 11 x64 doğrulaması ilgili masaüstü ortamını bekliyor. [GitHub runner tablosu](https://docs.github.com/en/actions/reference/runners/github-hosted-runners#standard-github-hosted-runners-for-private-repositories) · [GitHub Actions kullanım kotası](https://docs.github.com/en/billing/concepts/product-billing/github-actions#included-compute-usage-and-storage).
- OpenCode'un ücretsiz sohbet modelleri kendi istemcisiyle kullanılır. H041’in doğrudan uzak Chat Completions gözlemi için bu projede kullanılabilen yetkili bir API rotası gerekiyor. [OpenCode Console uçları](https://opencode.ai/docs/console/models/#endpoints).

### 19 Kasım — Uzak Chat Completions sayaç kaydı

AI Horde’un belgelenmiş anonim API’si üzerinden her taşıma için tek gönderim kullanıldı. Projeye ait izole Headroom 0.44.0 motoru yalnız sabit HTTPS hedefine sentetik aritmetik soruları gönderdi. İki HTTP 200 yanıtında istek kimliği, bitiş durumu ve sağlayıcının kullanım değerleri motorun ilgili gönderim kaydıyla eşleşti. İki isteğin her birinde 49 giriş ve 2 çıkış tokenı vardı. Sağlayıcı sayaçları tahmini olarak işaretlemedi. Önbellek okuma ve yazma değerleri bilinmiyor. SSE’de son kullanım çerçevesi ve `[DONE]` görüldü. [AI Horde kullanım sözleşmesi](https://haidra-org.github.io/AI-Horde/reference/chat_front/#reasoning-and-usage) · [Ölçüm](../proofs/chat-completions-usage.json).

SSE’nin beklenen tek hanesi birebir eşleşti. JSON yanıtı normal sonlandı ve sayaçları korudu. JSON’un model çıktısı tek hane biçim kontrolünü geçmedi. Önceki şemalı 404 ile bitiş ve biçim aşamalarında duran koşular ayrı kaydedildi. Bu doğrulama canlı sayaç aktarımını kapsar. Fatura farkı ve tasarruf oranı ölçülmedi.

### 19 Kasım — Kısıtlı Codex dosya profili

Hazır ve daha önce yalnız yerel fixture trafik kullanmış Windows test profiliyle `elevated` arka ucu denendi. CLI 0.169.0-alpha.11.1, `:root` okuma izni gerektiren denetimde komut başlamadan durdu. Bu sonuç dosyaya erişim reddi olarak sayılmadı. [Sabit Codex kaynak kodu](https://github.com/openai/codex/blob/226fa88979b1a9d76dc36f339ee9c5fbdb2c8e7f/codex-rs/windows-sandbox-rs/src/resolved_permissions.rs#L114).

Aynı CLI sürümünün resmî Linux ikilisi ve Ubuntu paketinden proje içine çıkarılan Bubblewrap 0.9.0 ile mevcut WSL2 ortamı sınandı. `headroom-file-scope` profili kökü kapattı, yalnız test kapsamını ve gerekli çalışma yollarını okudu. İlk ve son kontrolde iki sentetik dosya açıldı. Aradaki koşuda izinli dosya birebir okundu ve hedef dosya erişim reddi verdi. İzinli dosyadan arşiv sorgusu ve tam geri okuma 48 kayıt ile 5.926 özgün baytı doğruladı. On yedi girdi eşleşti. Koşu WSL2 içindeki Codex CLI dosya sınırını kapsar. Model çağrısı yapılmadı. [Codex sandbox belgesi](https://learn.chatgpt.com/docs/sandboxing) · [Programatik kanıt](../proofs/targeted-query-comparison.json).

### 19 Kasım — Anthropic Messages gerçek yerel inference

Windows 10’da sabit Headroom 0.44.0 motoru ve llama.cpp b12359, yerel SmolLM2-135M ile iki sentetik isteği tamamladı. JSON ve SSE’de şemaya uygun doğru cevap, normal bitiş ve motorun istek kimliğiyle eşleşen tek sağlayıcı gönderimi doğrulandı. SSE’nin son kullanım değerleri birikimli olarak okundu. [llama.cpp Messages ucu](https://github.com/ggml-org/llama.cpp/blob/b12359/tools/server/README.md#anthropic-compatible-api-endpoints) · [Anthropic akış sözleşmesi](https://platform.claude.com/docs/en/build-with-claude/streaming).

| Yanıt | Önbellek dışı giriş | Önbellekten okuma | Önbelleğe yazma | Çıkış |
| --- | ---: | ---: | --- | ---: |
| JSON | 44 | 0 | Bilinmiyor | 15 |
| SSE | 11 | 33 | Bilinmiyor | 15 |

İki istek aynı yerel sunucuda sırayla çalıştı. 33 token yerel sağlayıcının bildirdiği önbellek okumasıdır. Önbellek yazma alanı eksik olduğu için tam giriş ve toplam kullanım bilinmiyor kaldı. Üç ayrıştırma ve yedi istek sınırı kontrolü geçti. Yerel motor ve model sunucusu kapandı. Canlı Anthropic hesabı ve fatura karşılaştırması açık. [Programatik kanıt](../proofs/desktop-anthropic-usage.json).

### 19 Kasım — Gerçek yerel modelde eşlenmiş Chat Completions kıyası

Windows 10’da Headroom 0.44.0’ın varsayılan coding/cache profili ile doğrudan istek karşılaştırıldı. Aynı llama.cpp b12359 ve SmolLM2-135M, dört 24 satırlık sentetik logdan aynı sonucu okudu. Prompt cache kapalıydı. İki yanıt da doğru ve tamamlanmıştı. Sayılar model sunucusunun SSE kullanım sayaçlarıdır.

| Rota | Girdi | Çıktı | Toplam |
| --- | ---: | ---: | ---: |
| Doğrudan | 6.647 | 10 | 6.657 |
| Headroom üzerinden | 6.647 | 10 | 6.657 |

İki kolda da 12.013 baytlık araç çıktısı iletildi. %0 girdi tasarrufu ölçüldü. Headroom kullanım kaydı ilgili tek model isteğiyle eşleşti. 67 girdi ve araç dosyası eşleşti. Motor, yerel model ve sahipli portlar kapandı. Bulut modeli çağrısı 0. Sonuç bu iş yükündeki bir çifti kapsar. [Koşucu](../../bench/optimization/chat_local_savings.py) · [Yerel kanıt](../../work/proofs/optimization/chat-real-inference/pairs/cdbbc80e8eba4368b4db81e640194218/result.json)
