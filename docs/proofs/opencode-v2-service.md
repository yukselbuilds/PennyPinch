# OpenCode V2 normal servis ve tam TUI

15 Kasım 2026. Windows 10 22H2 x64 (19045), OpenCode 2.3.0 Windows x64 baseline ve Headroom 0.41.0. [Kabul planı](../decisions/009-opencode-provider-routes.md#h010-h--v2-normal-servis-ve-tam-tui), [programatik kayıt](opencode-v2-service.json), [inceleme](../reviews/H010-h.md).

Gerçek Headroom penceresindeki Önizle ve Uygula, izole OpenCode profilindeki iki sağlayıcı rotasını ve paketli MCP köprüsünü kaydetti. Normal servis, bu işlemden sonra `opencode service start` ile açıldı. Kayıt dosyası, dinleyici PID'si ve Windows Job sahipliği aynı servis örneğini gösterdi. CLI ve TUI komutları OpenCode'un normal servis keşfini kullandı. [Sabitlenen OpenCode servis çözümleyicisi](https://github.com/anomalyco/opencode/blob/a6b15d33daf6ab7d35569f6d711fdb630b9f04e7/packages/cli/src/services/server-connection.ts).

| Yüzey | Model seçimi | Yerel sonuç |
| --- | --- | --- |
| Normal `opencode run` | `lab/slot`, Chat Completions | Üç tamamlanmış yerel MCP çağrısı, dört model isteği, birebir final, exit 0 |
| Varsayılan tam TUI, Windows PTY | Dosyadaki varsayılan `openai/slot`, Responses | Üç tamamlanmış yerel MCP çağrısı, dört model isteği, terminalde final, `/exit` ile exit 0 |

Terminalin kayıtlı servis portuna açık TCP bağlantısı Windows'ta gözlendi. İki ayrı oturum aynı servis veritabanındaydı. Her oturumda doğru model, özgün test mesajı, üç tamamlanmış araç, son yanıt ve başarılı sonuç eşleşti. Her iki turda aynı örnek log **14.999 bayttan 103 bayta** indi ve özgün baytlarıyla geri çağrıldı. Bayt azalması yalnız bu log örneğine aittir. Gerçek token ve maliyet ölçülmedi.

`opencode service stop` exit 0 ile tamamlandı. Kayıt ve port kapandı, servis Job'ında süreç kalmadı. Headroom arayüzünden bağlantı kaldırıldığında özgün JSONC birebir döndü ve sahiplik kaydı bırakıldı. GUI, istemci ve servis normal kapandı. İzole motor ayarı da önceki baytlarına döndü. Test, yeni başlayan servisle yerel sentetik trafiği kapsar. Açık servisin ayarı anında benimsemesi, OpenCode Desktop, V2 alt ajanı, canlı hesap ve diğer işletim sistemleri ayrıca doğrulanacak.

Nihai yerel koşu `work/proofs/opencode-v2-service/native-fee4adc891/result.json`. İlk CLI koşusunda araç döngüsü tamamlandı. Test, finali boş `session_part` tablosunda aradığı için kabul durdu. Doğrulayıcı mesajın gerçek `content` alanını okumaya geçirildi. İlk koşunun normal servis, port, ayar ve sahiplik temizliği kendi kaydında korunur.

Nihai gerçek koşuda 22.705 kaynak, paket ve fixture girdisi önce/sonra eşleşti. Katalog doğrulanan sürüm ve ortamla güncellendi. Son yerel kapıda 222 Node ve 462 Rust testi geçti, 19 test ignored kaldı. Rustfmt, frontend ve Rust embedded derlemeleri geçti. Katalog güncellemesi dışında son ürün girdileri önceki adayla eşleşti.
