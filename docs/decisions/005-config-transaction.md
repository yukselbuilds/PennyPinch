# 005 — Sahiplikli config işlemi ve geri alma

22 Ekim2026. **Kabul edildi —H018 yerel ürün bağı/native dosya akışı tamamlandı.** KTM/TxF backend'i aynı yerel NTFS volume üzerinde Windows10 build19045'te sınandı. Bu karar Windows11, canlı istemci/auth/model/alt ajan veya yayın desteği doğrulaması değildir. [Son kabul](../proofs/config-transaction.md).

## Kapsam ve güven sınırı

H017'nin kullanıcı config planı uygulanır: Codex `openai_base_url`, Claude `env.ANTHROPIC_BASE_URL`, OpenCode V1 `provider.openai.options.baseURL`. Model, reasoning, provider, auth, hook/shell ve ajan sayısı değişmez. Aktif sohbet DB'si veya başka proje/oturum ayarları okunmaz/yazılmaz. Bilinmeyen sürüm, custom gateway, belirsiz rota ve engelli plan uygulanamaz; sırf UI testini açmak için sürüm/katalog sınırı gevşetilmez.

İlk desteklenen değişiklik, önceden eksik endpoint alanını eklemektir. Aynı Headroom endpoint'i zaten kullanıcı tarafından yazılmışsa yeni sahiplik oluşturulmaz. Önceki özel URL veya sır saklanmasına ihtiyaç duyan overwrite kapsamı açıkça engelli kalır.

Renderer keyfi yol, patch, URL, port veya config içeriği vermez. Apply yalnız backend'de saklanan süreli plan kimliği/beklenen fingerprint ve korelasyon metadata'sını kullanır. Commit öncesi plan süresi, kurulum/tarama, mevcut endpoint kimliği/adresi, config kaynağı/slotu ve dosya kimliği/fingerprint yeniden kontrol edilir. Birden fazla kurulum aynı kullanıcı config'ini paylaşabilir; ayrı kurulum seçmek dosyayı tekrar sahiplenme yetkisi değildir.

## Veri koruma ve geri alma

Patch yalnız gerekli alanı ekler; yorumlar ve ilgisiz baytlar korunur. Parse edilemeyen veya güvenle patch edilemeyen yapı hata üretir. Geri alma güncel belge üzerinde yalnız hâlâ Headroom'a ait değeri kaldırır. Kullanıcı yönetilen alanı değiştirmişse conflict döner; başka ayarlardaki düzenlemeler korunur. Sonradan içerik eklenen container veya config dosyası bütün olarak silinmez. Oluşturulmuş boş container/dosya ancak sahipliği ve güncel durumunun güvenli olduğu kanıtlanırsa kaldırılabilir.

Sahiplik/kurtarma kaydı sabit client/source/field enumları, UUID, loopback endpoint'i ve gerekli hash/dosya kimliklerini taşır. Ham config, auth, eski özel URL, hook komutu veya bütün dosya yedeği uygulama log/state'ine konmaz. H017'nin yalnız redakte bellekte tuttuğu plan lease'i restart sonrası yazım yetkisi sayılmaz. Başarıyla değiştirilmiş alanın sahipliği ise uygulama yeniden açıldığında bulunabilir olmalıdır.

## Yayın primitive'i için kabul kapısı

Mevcut preferences store'u uygulamanın kendi state'i içindir. Buradaki hash kontrolü → handle kapatma → MoveFileEx dizisi, dış editörün de değiştirdiği istemci config'i için karşılaştırmalı atomik yayın kanıtı değildir. Bu yol doğrudan kopyalanmaz. [ReplaceFileW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew) target paylaşımı, desteklenmeyen WRITE_THROUGH flag'i ve kısmi hata durumları ayrıca değerlendirilir. [FileRenameInfoEx](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_rename_info) açık handle ile yayın yeteneği tek başına başka rename'leri engellediği anlamına gelmez.

İlk deney yalnız yeni UUID altındaki sentetik dosyalarla yapılır. Hedef/parent handle açıkken normal writer, replace, rename-away ve delete davranışları ayrı ölçülmelidir. Başarılı bir API dönüşü, dosya kimliği ve son baytlarla doğrulanmadan işlem başarısı sayılmaz. Katı CAS veya güç kesintisi garantisi ölçülmemiş bir API örneğinden çıkarılmaz. Kullanıcı dosyasına yazan production komutu, primitive ve kesinti/kurtarma modeli kabul edilmeden açılmaz.

## Seçilen dar backend: NTFS transaction

Klasik API denemesinde hedef/parent pin'ini güçlü tutmak bizim yayınımızı da engelledi; paylaşımı gevşetip yayını açan varyantta dış ReplaceFile da çalışabildi. Bu deney katı karşılaştırmalı yayın kanıtı vermedi. Yerel KTM/TxF deneyi ise aynı transaction içinde config ve sırsız ownership dosyasını güncelleyebildi. Transacted file handle'ları kapandıktan sonra, commit öncesi ayrı süreçten normal write/replace/delete denemeleri reddedildi. Commit öncesi süreç ölümü iki dosyayı eski; commit sonrası süreç ölümü ikisini yeni bıraktı. Yönetici yetkisi kullanılmadı. [Ölçümler](../proofs/config-transaction.md).

Bu nedenle H018 için izole, capability kontrolü olan NTFS transaction backend'i seçildi. Yerel sabit NTFS ve transaction desteği gereklidir; API/volume uygun değilse yazım sabit hata ile reddedilir. Network/ReFS/FAT, EFS dosyaları, link/reparse veya başka doğrulanmamış yollar zayıf replace fallback'ine geçmez. Kullanıcı ACL/attribute veya sistem ayarları değiştirilerek destek zorlanmaz. Config fingerprint'i transaction içindeki kilitli güncel görünümle doğrulanır; config patch'i ile sahiplik kaydı aynı commit'e dahildir. Uygulamanın tam config yedeği veya auth içeren kardeş staging dosyası tutmasına gerek kalmaz. Bu, Windows'un kendi dosya sistemi undo/journal verisinin hiç oluşmadığı iddiası değildir.

Microsoft [yeni TxF bağımlılıkları yerine alternatifleri öneriyor ve ileriki Windows sürümlerinde kaldırılabileceğini belirtiyor](https://learn.microsoft.com/en-us/windows/win32/fileio/transactional-ntfs-portal). Bu bakım/uyumluluk riski kabul edilen teknik sınırlamadır: denenen alternatifler eşzamanlı dosya değişimini istenen şekilde korumadı; TxF tüm ürüne yayılmaz, yalnız bu yazım backend'inde kalır. Desteklenmeyen ortamda yalnız önizleme çalışabilir. İleriki backend değişimi aynı kabul matrisini sağlamalıdır; bugün Win11, diğer dosya sistemleri veya fiziksel güç kesintisi denenmiş sayılmaz.

[TxF programlama sözleşmesi](https://learn.microsoft.com/en-us/windows/win32/fileio/programming-considerations-for-transacted-fileio-) uyarınca file handle'ları commit öncesi kapatılır; transaction'ın intent/ancestor koruması commit/rollback bitene kadar geçerlidir. Synchronous commit, bounded transaction süresi, RAII rollback ve sabit hata eşleme gerekir. Rust üretim primitive'i ve gerçek manager akışı aynı koşulları ayrıca test etmeden native Python deneyi ürün başarısı sayılmaz.

## Kabul sırası

### macOS APFS adayı — H048-b6b

Windows'un TxF sözleşmesi macOS'a genellenmez. APFS adayı config ve sahipliği ayrı atomik adlarla yayımlar. İki adım arasındaki kesinti, özel uygulama dizinindeki bounded intent ve inode/birth/metadata/içerik özetleriyle tanınır. Yeniden başlatma, eski planı config yazmak için canlandırmaz. Açık kullanıcı eylemi yalnız yayımlanmış config ile eşleşen sahiplik adımını tamamlayabilir; henüz yayımlanmamış intent iptal işareti alır. Salt okunur keşif/snapshot kurtarma yazımı yapmaz.

Kurtarmayı doğru hedefle eşleştirmek için config'in kayıpsız mutlak yol baytları minimum metadata olarak özel state'te tutulur. Kaydedilmiş yol tek başına dosya açma yetkisi değildir; aktif backend hedefi, path binding ve parent kimliği eşleşmelidir. Başka proje hedefi açılmaz. Ham config, auth ve xattr değerleri bu state'e yazılmaz. Mevcut dosyanın staging kopyası kendi istemci dizinindedir; ACL/izin/xattr metadata'sı açık kaynak descriptor'dan `fcopyfile(COPYFILE_METADATA)` ile aktarılır. [Apple copyfile kılavuzu](https://github.com/apple-oss-distributions/copyfile/blob/main/copyfile.3).

Swap ile taşınan eski inode otomatik silinmez. Açık descriptor'a geç yazım korunur ve yeni aynı-hedef eylemini conflict ile durdurur. 64 intent sınırı veri silerek aşılmaz. Bu sınırın kullanıcıya sunumu ve korunmuş sürümlerin güvenli çözümü H048'in açık kabulüdür. Bu bir çok dosyalı katı CAS veya fiziksel güç kaybı garantisi değildir. Mevcut dizindeki config dosyası ilk yazım kapsamıdır; eksik Claude proje dizini ayrıca açıktır. Üretim yazım kodunun native kabulü henüz tamamlanmadı.

H018-a gerçek Windows primitive/yayın deneyi; H018-b byte-preserving patch/undo ve sırsız sahiplik kaydı; H018-c manager/typed komut/UI bağı; H018-d inceleme, son kalite kapısı ve gerçek native fixture akışı. Alt dilim PASS'i ana görevi tamamlamaz. [Görev listesi](../../TASKS.md), [devir noktası](../IMPLEMENTATION_STATUS.md).
