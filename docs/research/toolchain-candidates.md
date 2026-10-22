# H013 bağımlılık adayları

19 Ekim 2026 — Resmî npm/crates.io metadata'sı salt okunur sorgulandı. Bunlar indirilmiş/derlenmiş bağımlılık veya kalite kapısı PASS'i değildir. H013'te kesin sürümler lock dosyaları ve gerçek derlemeyle sabitlenecek; yeni bir proje olduğundan migration yapılmıyor.

| Paket | Sorgulanan sürüm | Metadata gereksinimi |
| --- | --- | --- |
| `@tauri-apps/api` | 2.12.4 | — |
| `@tauri-apps/cli` | 2.12.3 | Node >=10 |
| `react`, `react-dom` | 19.3.1 | — |
| `vite` | 8.3.4 | Node ^20.19.0 veya >=22.12.0 |
| `@vitejs/plugin-react` | 6.1.2 | Node ^20.19.0 veya >=22.12.0 |
| `typescript` | 7.0.2 | Node >=16.20.0 |
| `@types/react`, `@types/react-dom` | 19.3.0 | — |
| Rust `tauri` | 2.12.4 | Rust1.90 |
| Rust `tauri-build` | 2.7.3 | Rust1.90 |

Host envanterindeki Node25.2.1 ve Rust1.92 bu iki metadata tabanını karşılar; gerçek transitive dependency çözümü, MSVC/native derleme ve WebView açılışı ayrıca doğrulanacak. Sürüm seçimi uygulama modelini veya kullanıcının global araç zincirini değiştirmez.

Kaynaklar: [npm registry](https://registry.npmjs.org/) üzerinde her paketin `/latest` metadata endpoint'i; [tauri crate](https://crates.io/api/v1/crates/tauri) ve [tauri-build crate](https://crates.io/api/v1/crates/tauri-build). Rust tauri2.12.4 checksum `f316751027fc2644221d1a448564d519efb07067fda584879d9cda966391d212`; tauri-build2.7.3 checksum `641dd5fd297c99ab92717b0428cca5b9c6f960b2ac96ca131b3eebc9d70b8ddc`.

Yeni pnpm/Cargo indirme cache'leri proje `work/caches/` altında tutulmalı; önceden kurulu toolchain okunabilir. Native scaffold'un engine entegrasyonuyla karıştırılmaması ve Graphite örnek rakamlarının üretime kopyalanmaması gerekir.
