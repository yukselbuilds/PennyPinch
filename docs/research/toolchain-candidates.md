<a name="h013-bağımlılık-adayları"></a>

# H013 dependency candidates

19 October 2026. The review queried official npm and crates.io metadata in read-only mode. These results cover metadata only. Downloads, builds, and the quality gate require separate evidence. H013 will pin exact versions in lockfiles and a real build. This new project does not need a migration.

| Package | Queried version | Metadata requirement |
| --- | --- | --- |
| `@tauri-apps/api` | 2.12.4 | — |
| `@tauri-apps/cli` | 2.12.3 | Node >=10 |
| `react`, `react-dom` | 19.3.1 | — |
| `vite` | 8.3.4 | Node ^20.19.0 or >=22.12.0 |
| `@vitejs/plugin-react` | 6.1.2 | Node ^20.19.0 or >=22.12.0 |
| `typescript` | 7.0.2 | Node >=16.20.0 |
| `@types/react`, `@types/react-dom` | 19.3.0 | — |
| Rust `tauri` | 2.12.4 | Rust 1.90 |
| Rust `tauri-build` | 2.7.3 | Rust 1.90 |

The host inventory reports Node 25.2.1 and Rust 1.92. These versions meet the two metadata baselines. Verify transitive dependency resolution, the native MSVC build, and WebView launch separately. Version selection does not change the application model or the user's global toolchain.

The review used each npm package's `/latest` metadata endpoint and the crates.io records for both Rust packages. [npm registry](https://registry.npmjs.org/) · [tauri crate](https://crates.io/api/v1/crates/tauri) · [tauri-build crate](https://crates.io/api/v1/crates/tauri-build). The Rust tauri 2.12.4 checksum is `f316751027fc2644221d1a448564d519efb07067fda584879d9cda966391d212`. The tauri-build 2.7.3 checksum is `641dd5fd297c99ab92717b0428cca5b9c6f960b2ac96ca131b3eebc9d70b8ddc`.

New pnpm/Cargo download caches must stay under the project `work/caches/` directory. The review can read the previously installed toolchain. Keep native scaffolding and engine integration separate. Keep Graphite example values out of production.
