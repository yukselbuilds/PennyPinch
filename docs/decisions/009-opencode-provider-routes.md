# Decision 009 — OpenCode V1 provider routing

24 October 2026. H021 active. H020 remains closed. This is a bounded product adapter, not general OpenCode/V2/provider compatibility.

## Required behavior

Reuse the common user JSON/JSONC resolver, native discovery, lease, atomic config/ownership writer and conflict-preserving Undo. The exact measured native CLI is1.19.3,180,982,664 bytes, SHA256 `cab0c9a8480f71fbb7cbc88ad91057712ecb500ab41fb9c01dd586cb9d8ea375` (H010 pin). Unknown versions/surfaces stay blocked. Do not run or modify a global OpenCode install for discovery.

One plan covers every eligible provider alias in the selected user file. Eligibility is based on protocol and upstream semantics, not a model allowlist or one hard-coded alias. First profiles: built-in `openai` with its pinned Responses loader, and explicit custom aliases using `@ai-sdk/openai-compatible` Chat Completions against the standard OpenAI upstream. Those two CLI transports need separate native fixture evidence. Preserve aliases, top-level/small/agent model choices, model `id` mapping, reasoning/options, auth, hooks, enabled/disabled provider selections and unrelated bytes.

Only an unambiguous default built-in upstream, an exact public canonical `https://api.openai.com/v1`, or the same stable Headroom endpoint is eligible. Other/custom upstreams, different SDKs and unresolved model-level endpoint/SDK/header overrides are excluded and untouched. A custom compatible alias without an explicit canonical base is not assumed to target OpenAI. Disabled provider entries are not edited. An invalid/ambiguous document blocks rather than guessing. Limits on provider/model/variant counts and key size bound work and receipt size.

Safe exclusions produce explicit partial coverage, not a global blocker on unrelated eligible routes. Zero eligible routes cannot Apply. This does not claim an excluded provider, another config layer, live account or subagent route is covered. Selecting another model behind an eligible alias requires no reinstall; another upstream remains outside the accepted profile.

## Plan and ownership boundary

Keep actual provider keys in native analysis/lease data. The renderer needs only bounded counts, protocol families and partial/unknown status; it supplies no patch path, provider list or original value. The plan must truthfully distinguish multiple observed endpoint states from an absent endpoint. Existing clients retain their strict contracts.

Extend the existing JSONC span patcher, not a second writer. All eligible endpoint edits and the one ownership record commit together. Undo fails closed for any conflicting owned route. New optional bounded receipt rows must retain schema1/legacy single-openai receipt decoding; another client's receipts cannot carry OpenCode routes. Domain-bound provider-key digests may identify rows without retaining raw aliases. A canonical original URL may be retained only as a bounded, validated JSON string literal so escaped original bytes can be restored. Arbitrary URL/auth/config backups are prohibited. Missing endpoints retain ancestor-creation/removal ownership semantics; preexisting same-endpoint routes are never newly owned.

## Settings and UI boundary

The common resolver edits the selected user file; it does not calculate another process's merged runtime settings. Observed Headroom-process `OPENCODE_CONFIG`, `OPENCODE_CONFIG_CONTENT` or `OPENCODE_CONFIG_DIR` overrides must not be silently ignored; block ambiguous target edits and recheck observations at Apply. Project, remote, managed, plugin and external-session effects remain unknown unless separately observed. Auth stores and conversation bodies are not inspected/copied.

The OpenCode UI stays short: eligible route count, excluded route count, protocol family and a clear partial-coverage note. Advanced details are optional; no dense new settings panel. The broader simpler dashboard/accounts/chat requirements remain H022/H025/H026/H050/H051. No paid verification or model/account requirement is introduced.

## Sources and acceptance

The official v1.19.3 tag resolved to commit `e3c4eb242585dcd77be1a867391b7c25fd47ce52`. Bounded reviewed source files/hashes are in `work/proofs/opencode-product/source-pin.json`. Primary sources: [provider model/SDK composition](https://github.com/anomalyco/opencode/blob/e3c4eb242585dcd77be1a867391b7c25fd47ce52/packages/opencode/src/provider/provider.ts), [config loading](https://github.com/anomalyco/opencode/blob/e3c4eb242585dcd77be1a867391b7c25fd47ce52/packages/opencode/src/config/config.ts), [provider schema](https://github.com/anomalyco/opencode/blob/e3c4eb242585dcd77be1a867391b7c25fd47ce52/packages/core/src/v1/config/provider.ts). Sources establish intended code paths, not runtime adoption.

After focused implementation/review, run the final full local gate once, then real native UI→user config→exact CLI→owned ingress/engine→local fixture. Exercise multiple eligible aliases, preserved selector versus wire model, both protocol profiles, JSON/JSONC comments and Unicode, reopen/Undo, conflict and owned cleanup. H010's unchanged helper-only result is not a substitute. Live auth/provider/tools/agents/other OS remain untested where evidence is absent. Finish H021 and stop; no next main task begins automatically.

### H010-b3 yerel geri alma aracı

Windows'taki OpenCode bağlantısı önce salt okunur önizleme gösterir. Kullanıcı Uygula'yı seçince backend aynı planı doğrular ve uygun yönlendirmelerle yerel `headroom_retrieve` aracını tek işlemde ekler. Önceden eklenmiş bağlantı, mevcut geri alma işlemiyle kaldırılıp yeniden önizlenir. Kullanıcı dosyasındaki ve OpenCode'un aynı kullanıcı klasöründen birleştirdiği eski `config.json` içindeki çakışan MCP kaydı yazımı engeller. OpenCode'un dönüştürebildiği eski `config` dosyası varsa da işlem durur. Diğer süreçlerin proje, yönetilen ayar ve eklenti katmanları için etkin rota bilinmiyor kalır.

Köprüyü OpenCode, paketle gelen Python ve doğrulanmış komutla çalıştırır. Araç yalnız sıkıştırma hash'iyle yerel geri alma isteği oluşturur. Sabit endpoint kimliği ile açık soketin Windows süreç yolu ve kullanıcı kimliği doğrulanır. Motor duraklatıldığında önceden sıkıştırılmış sonuçlar aynı geri alma yolu üzerinden erişilebilir kalır. Tam durdurma sırasında araç hata döndürür. Yeniden başlatmada geri alma, aynı endpoint'in ve süresi dolmamış cache kaydının bulunmasına bağlıdır. OpenCode izinleri mevcut kurallardan gelir.

OpenCode köprüyü standart giriş kapanana kadar tutabilir. Release köprüsü mevcut masaüstü kullanım kilidini paylaşır. Kaldırma veya güncelleme için bağlı OpenCode oturumu kapanınca kilit serbest kalır. Kaldırıcı, aktif sahiplik kaydıyla aynı çatışma korumalı geri alma yolunu kullanır. Köprü veya rota değiştirilmişse dosya ve kurulum korunur.

## H010-c — Yerel alt ajan kabulü

14 Kasım. Ana ajan rota/model/izin ve süreç kabulünü üstlenir; ürün kaynağı değişmeden mevcut H010-b3 release'i ve paket manifesti kullanılır. Gerçek OpenCode 1.19.3 `task` aracı foreground child session yaratır. Pinned kaynakta child model açık agent modelinden, yoksa parent assistant metadata'sından alınır; child için ayrı parentID ve task metadata'sında sessionId/model vardır. [Task kaynağı](https://github.com/anomalyco/opencode/blob/e3c4eb242585dcd77be1a867391b7c25fd47ce52/packages/opencode/src/tool/task.ts), [izin türetme](https://github.com/anomalyco/opencode/blob/e3c4eb242585dcd77be1a867391b7c25fd47ce52/packages/opencode/src/agent/subagent-permissions.ts). Gerekli altı kaynak `work/research/h010-subagents/manifest.json` içinde hash'lenir; kaynak davranış kanıtı değildir.

Üç sıralı senaryo: (1) Chat parent ve model miras alan child; (2) aynı parent session'a açık CLI model seçimiyle başka Chat modeli, tekrar miras alan child; (3) Responses parent ve açıkça başka Chat modelindeki child. Fixture hem parent hem child için veri → CCR retrieval → tanık üretir; parent ayrıca gerçek task sonucunu bekler ve final verir. CLI event/task metadata ile child export'unun gerçek parentID/agent/model alanları çapraz doğrulanır. Export yalnız fixture state'inden okunur; transcript/gövde rapora taşınmaz. Model/agent attribution yalnız model adından çıkarılmaz.

Native Önizle/Uygula iki provider'ı bir kez bağlar. Testler sırasında saved route/MCP receipt değişmez; model değişimi aynı sohbette ve yeniden bağlanmadan yürür. Sonunda native Undo özgün JSONC'yi byte-exact geri verir ve GUI/motor normal kapanır. Test-only agent tanımları en fazla bir foreground child çalıştırır, nested/background/task yeniden çağrısı ve shell/read/write/network araçları kapalıdır. İzin sadece sabit MCP ve adı belirtilmiş iki child'a verilir; auto/yolo izin bayrağı kullanılmaz. Her CLI 120s, export 30s, HTTP gövdesi 4MiB ve UI koşusu 15 dakika ile sınırlıdır; başarısız adım tekrarlanıp gizlenmez.

Kapanışta GUI normal onaylı çıkışla durdurulur; aynı release'in mevcut `--headroom-uninstall keep` girişi, test uygulamasının exclusive gate'iyle sahipli bağlantıyı geri alır ve state'i korur. H010-b3'te gerçek UI Undo ayrıca kabul edilmiştir. Böylece kaldırma popup'ının arka plan mesajlarını kabul etmediği ortamda kullanıcının ön plandaki uygulamasına fiziksel fare/klavye gönderilmez; farklı bir config writer eklenmez.

Kabul her senaryoda farklı parent/child session kimliği, child parentID eşitliği, beklenen model/agent, korunmuş sentetik auth, iki rolün marker/byte-exact retrieval/tanık/final'i, tamamlanmış araçlar ve exit0 ister. Aynı source/CLI/runtime/executable/environment önce-sonra fingerprint'leri kaydedilir. Mevcut 332 girdi değişmediyse eski ürün kapısı yeniden kullanılabilir; yeni fixture/bench odaklı sınanır. Canlı sağlayıcı/auth, gerçek model kalitesi/TTFT/fatura, TUI/V2/Desktop ve ikinci OS açık kalır. Bu dilim API-key/hesap/CI veya ücretli kaynak yetkisi vermez.
