# Optimizasyon ROI ve kapsam değerlendirmesi

17 Kasım 2026. Kullanıcının paylaştığı Gemini değerlendirmesi tavsiye kaynağı olarak incelendi; ölçüm veya görev kapatma yetkisi sayılmadı.

## Uygulanan kararlar

- JSON adayının ilk gerçek çifti için sonuçtan önce ayrı durma kaydı oluşturuldu. Aynı seçili hesap/model/efor, mevcut görev/veri/korumalar ve sekiz gönderim sınırı korundu. Kalite hatası ret, eksik tur/sayaç/cache koşulu durma, geçerli fakat %5'in altında ilk toplam-kullanım sinyali Ar-Ge'yi dondurma nedenidir. Bu iş kararı tek çiftten genel etki çıkarmak değildir; olumlu sinyal de mevcut bağımsız matrisin yerine geçmez.
- Yalnız sabit testin `sent` / `retry` tamsayıları ve normal, bilinen araç sırası/durumları kaydedildi. Beklenmeyen string alanları, prompt, özel reasoning ve ham model kodu saklanmadı. Yeni gözlem katmanı model isteğini, yanıtını, talimatını veya bütçesini değiştirmedi; aynı sınırlı gözlem iki kolda da çalıştı.
- Koşu beş kontrol + üç aday gönderimiyle sınırda bitti. Kontrol doğru sonuç verdi; adayın görünür CLI kaydında iki `inspect_result` tamamlanması vardı ve son tur tamamlanmadı. Adayın bütün kullanım toplamı bilinmiyor, çiftin cold koşulu doğrulanmamış kaldı. Kalite/savings kabulü verilmedi. Yeni tekrar veya üçüncü kural adayı otomatik başlatılmayacak. [Kayıt](../proofs/rules-json-live-pilot.json).
- Sonraki mevcut H041 çalışma önceliği güvenli hedefli sorgudur. `total_lines`, `matched_count` ve `truncated` gibi sinyaller ancak gerçekten hesaplanabiliyorsa gösterilir. Bütünlük veya eşleşme garantisi, snapshot/sahiplik ve tam tarama/bütçe kanıtına bağlıdır; alan adı eklemek tek başına garanti sağlamaz. Geri okuma, retry ve ek model turları toplamda kalır.

## Düzeltilen yorumlar

1. **Boyut eşiği şimdi eklenmedi.** 1.500–2.000 tahmini token ve %30 yerel küçülme önerileri aday hipotezidir. Hazır küçük görevi sonradan pasif bırakmak A/B'yi değiştirir. Küçük çıktıya dokunmamak ek dönüşüm riskini azaltabilir; genel model hata riskini sıfırlamaz. Mevcut aday zaten büyüyen JSON yerine özgün stdout'u döndürür. Yerel bayt/token azalması, bütün sağlayıcı turlarındaki net faydayı kanıtlamaz.
2. **Hareket eden Anthropic işareti tek başına cache kaybı değildir.** Resmî otomatik cache akışında işaret yeni bloklara ilerler; eski prefix okunabilir ve eklenen bölüm yazılabilir. Değişen prefix, TTL ve lookback sınırı ayrıca önemlidir. Çok turlu okuma/yazma ve toplam maliyetin birlikte değerlendirilmesi doğrudur; yerel işaret konumundan tek başına fatura sonucu çıkarılmaz. [Anthropic prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching#how-automatic-caching-works-in-multi-turn-conversations).
3. **Hedefli sorgunun büyük sayıları sağlayıcı ölçümü değildir.** Projemizdeki sabit yerel örneğin tüm girdi JSON'u 60.295 → 693–709 tokenizer tokenıydı. Sonradan tam okuma dahil edilince 61.200–61.228 oldu. Gerçek istemci izinleri, aynı modelle görev kalitesi ve sağlayıcı maliyeti hâlâ açık. 36.000 → 41 gibi başka bir bağlamdan gelen oran ürün vaadi yapılmaz. [Yerel kaynak](../proofs/targeted-query-pilot.json).
4. **H039 yeniden bloke edilmiyor.** H039'un Windows 10 yerel kabulü geçmişte tamamlandı. H048'in eski macOS geliştirme kabulü de yeni 0.44.0 pininin native kabulüyle aynı şey değildir. H041/H042, Windows 11, yeni native pin ve üretim/API eksikleri kendi açık/not_tested sınırlarıyla kalır. Opt-in Ar-Ge'yi dondurmak bunları geçmiş yapmaz. Mevcut ücretsiz kota ve hesap sınırları korunur; yeni CI, API-key satın alma veya dağıtım başlatılmaz.

## Çekirdek ve Ar-Ge ayrımı

Çekirdek kayıt; izinli rotadaki güvenilir kullanım eşlemesi, süreç/ayar sahipliği, paket/OS uyumluluğu ve yayın kabulüdür. Learn fayda pilotu, deneysel kurallar ve Serena ayrı, varsayılan kapalı Ar-Ge kabulleridir. Hepsi mevcut görev listesinde açık kalır; Ar-Ge denemesi ürünün çekirdek uyumluluk kanıtı yerine geçmez. Dondurulmuş e3'ten kabulü gerektiren e4'e geçilmez. Ana H041 tamamlandı olarak işaretlenmez; aynı ana görevdeki bağımsız hedefli sorgu hazırlığı ilerleyebilir.
