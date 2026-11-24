# H052–H054 — Privacy for optional usage reports

24 November 2026. This decision covers benchmark reports for a project operated from Türkiye. It covers Türkiye and the EU rules that may apply to the service. The release owner must verify the operator, release scope, and data flow before the service accepts real reports.

## Legal basis

Token totals alone can be non-personal. A random installation ID links reports from the same installation. Network metadata and rare combinations of fields can identify a person indirectly. Treat daily installation reports as personal data. Assess whether published totals can still identify or isolate a contributor. [KVKK guidance on anonymization](https://www.kvkk.gov.tr/SharedFolderServer/CMSFiles/8110dc3c-2856-4e54-9129-5e2e375469af.pdf) · [GDPR Article 4 and Recital 26](https://eur-lex.europa.eu/eli/reg/2016/679/oj/eng)

Use voluntary consent for recurring benchmark reports. Show the privacy notice before the consent action. The notice names the controller, the purpose, the data, the collection method and legal basis, the recipients and transfers, retention, and how to exercise rights. Record the consent action and the notice version. Make withdrawal as easy as consent. The core application works with sharing off. [KVKK Articles 5, 10, and 11](https://www.kvkk.gov.tr/Icerik/6649/Personal-Data-Protection-Law) · [KVKK notice and consent decision 2026/347](https://www.kvkk.gov.tr/Icerik/8710/veri-sorumlulari-tarafindan-acik-riza-ve-aydinlatma-metinlerinin-ayri-ayri-duzenlenmesi-gerektigi-hakkinda-kisisel-verileri-koruma-kurulunun-18-02-2026-tarihli-ve-2026-347-sayili-ilke-kararina-iliskin-kamuoyu-duyurusu) · [GDPR Articles 5–7 and 13](https://eur-lex.europa.eu/eli/reg/2016/679/oj/eng)

Check GDPR Article 3 if the release targets people in the EU or monitors their behavior there. Check applicable national ePrivacy rules for access to information on the device. The EDPB explains that locally generated information sent to a server can fall within Article 5(3). Apply the opt-in before the first collection for remote sharing. [GDPR territorial scope](https://www.edpb.europa.eu/system/files/documents/files/file1/edpb_guidelines_3_2018_territorial_scope_after_public_consultation_en_1.pdf) · [EDPB ePrivacy guidance, section 3.2](https://www.edpb.europa.eu/documents/guideline/guidelines-22023-on-technical-scope-of-art-53-of-eprivacy-directive_en)

## Hosting and transfers

The current hosting candidate is Neon Free with PostgreSQL in AWS Europe (Frankfurt). Neon lists the Free plan at $0 and says it requires no credit card. Verify the project's plan, remaining quotas, and billing settings before the synthetic pilot. [Neon pricing](https://neon.com/pricing) · [Neon plans](https://neon.com/docs/introduction/plans)

Neon documents Functions in Frankfurt and says Functions are available on any plan, subject to usage limits. The actual Function entrypoint, administrator authentication, trigger schedule, project configuration, and deployment have not passed review. Verify the database and Function regions in the actual project. Review Neon processing terms, subprocessors, provider access, and service logs before the service accepts real reports. [Neon Functions](https://neon.com/docs/compute/functions/overview) · [Neon data processing agreement](https://neon.com/pdf/DPA.pdf)

Daily reports are regular transfers. For transfers abroad subject to KVKK, verify whether an adequacy decision applies. Use an appropriate safeguard under Article 9 if no adequacy decision applies. Reserve the Article 9 consent exception for occasional transfers that meet its conditions. If the project uses the Turkish standard contract, secure the parties' signatures and notify the KVKK Authority within five business days of signing. If GDPR applies, verify the processor terms under Article 28 and the applicable Chapter V transfer mechanism. [KVKK transfer rules](https://www.kvkk.gov.tr/Icerik/2053/Yurtdisina-Aktarim) · [KVKK standard contract notification](https://www.kvkk.gov.tr/Icerik/8043/Standart-Sozlesme-Bildirim-Modulu-Hakkinda-Kamuoyu-Duyurusu)

## Data lifecycle and acceptance

- Start daily reports only after separate opt-in. Use measurements captured after opt-in. Show a payload preview and the last send status. Declining, upgrading, or restarting an opted-out installation must result in zero report uploads.
- Accept only the H054 fields. Exclude prompts, code, paths, account and chat IDs, and secrets. Keep IP addresses and request contents out of application storage and logs. Verify provider logging settings and disclose infrastructure processing.
- On withdrawal, stop new uploads and clear the queue. Test withdrawal during a retry or active upload. Verify the result of any upload that was in flight. Use an authorized path for access and erasure requests. Delete personal reports after withdrawal when the law requires it. Provide a rights contact.
- Delete daily reports from active storage after 30 days. Document backup and restore retention. Apply expired-record deletion and queued erasures before a restored database serves requests. Neon documents a six-hour Free instant-restore history, limited to 1 GB of changes, and one manual snapshot. Scheduled backups are not available on Free. These provider features do not by themselves prove 30-day erasure from all backups. Verify the selected restore process and its retention. [Neon plan limits](https://neon.com/docs/introduction/plans)
- Test aggregates for re-identification before long-term retention or publication. A small group, a dominant contributor, or the difference between published totals can expose one installation. Suppress aggregates that fail the review. Show sample size, dates, measurement basis, and coverage. Apply H052's weighted net calculation.
- Give manual feedback its own notice and retention policy. Preview its text and optional metadata before send. Review the processing basis and hosting route for that data before opening the endpoint.

Resolve the controller, notice, rights contact, and transfer basis before accepting real reports. Verify these conditions against the actual deployment. Use synthetic reports for local tests. Keep collection off while these items are open.

## H052-a — Daily usage storage

The storage contract accepts a version 1 JSON summary of at most 4,096 bytes. Each summary has a random version 4 UUID, a completed UTC day, an OS family, application and engine versions, and a notice version. Application versions use numeric release or preview versions. The schema accepts notice version 1. H054 must bind the notice version to the user's consent action before sending.

The contract accepts the six known Headroom 0.45.0 WebSocket counter bases. The bases measure whole-request input. Each group has its own comparable request count and before and after token totals. Keep incomparable and failed request counts separate. A failed request belongs only to the failed count. A report can include coverage when it has no measured pair. Preserve zero and negative savings.

Store one summary per installation and UTC day. Accept identical retries and reject changed summaries. Commit the write and its result read in one PostgreSQL transaction. Retain at most the 30 most recent completed UTC days. Exclude older records from reads even when physical deletion is pending. Delete expired rows in bounded batches. Verify the cleanup schedule in the live pilot.

The local implementation uses `services/reports/migrations/0001_reports.sql` and `services/reports/usage.mjs`. The HTTP service is in `services/reports/service.mjs`. The local suite passed 26 tests with isolated PGlite. A separate local PostgreSQL 17.12 run verified the schema, concurrent quota, retry, and expiry paths. These results do not verify Neon migration or a live reporting path. [PostgreSQL port evidence](../proofs/h052-postgres-port.json)

## H052-b — Daily report HTTP intake

The local service accepts report submissions at `/v1/usage` over HTTPS and keeps intake closed by default. It accepts only POST requests with the H052-a schema. It rejects bodies over 4,096 bytes before decoding. It authenticates each installation and limits requests to 6 per installation per fixed UTC minute. It allows at most 128 new reports per UTC day. Keep credentials and report bodies out of logs. The Neon Function URL, deployed authentication, and account limits remain unverified.

Identify each report by installation and UTC day. An identical retry returns `duplicate`. A retry with changed data returns `conflict`. Both preserve the stored report. Return fixed status names and disable response caching.

Return HTTP 429 when the per-installation or daily limit is reached. Do not upgrade the plan or incur charges to continue collection. Enforce the no-card and $0 spending limits before any live setup.

Local HTTP tests must use synthetic credentials and reports. They do not verify Neon authentication, account quotas, provider logs, or a deployed Function. Keep collection off until each live test passes.

## H052-c — Administrator report queries

The local service keeps the administrator route closed by default. Tests inject an authorized `verifyAdmin` function. No production access provider, signed-identity validation, or Neon login has been selected or tested. Do not expose a public query endpoint or accept a user-supplied installation identifier.

Support bounded reads for 1, 7, or 30 completed UTC days. Reject unsupported ranges and excessive row counts. Return only aggregate coverage and counts. Exclude installation IDs and administrator identity.

Keep token counts exact as decimal strings. Calculate weighted savings from total before and after counts within each counter basis. Calculate installation medians only after summing each installation's counts. A zero before count has an undefined percentage. Include measured zero and negative results in coverage. Give the administrator view and any public summary separate privacy acceptance.

The local route tests use a synthetic verifier and reports. They do not prove a deployed login policy or a live summary query.

## H052-d — Private admin panel

The local private-panel renderer is in `services/reports/admin-panel.mjs`. The service serves it only after the injected `verifyAdmin` function approves the request. Local tests cover this dependency. No Neon administrator login or panel deployment has passed acceptance. The panel defaults to seven completed UTC days. It also offers one-day and 30-day ranges.

Show local token estimates, weighted savings, and the per-installation median for each counter basis. Show installations, report days, and comparable, incomparable, and failed requests. Give empty periods, periods without comparable requests, and database failures distinct messages.

Serve aggregate data only. Disable caching. Use nonce-bound styles. Block scripts, framing, and external resources. Test the panel with synthetic reports. Do not claim a private live dashboard until the access policy, queries, and deployment pass.

## H052-e — Bounded retention and scheduled cleanup

Expire reports after 30 days. The local PostgreSQL implementation deletes up to 500 rows per batch and stops after at most 8 batches per call. Keep expired records out of query results while cleanup is delayed. Resume a backlog on the next cleanup call. Local expiry passed against PostgreSQL 17.12. No Neon trigger has been configured or tested for this project.

Neon documents that function triggers can retry and deliver events more than once or out of order. Make cleanup safe to repeat. Verify actual Free-plan limits and retry behavior before scheduling a trigger. [Neon Function Triggers](https://neon.com/docs/compute/functions/triggers/overview) · [Neon plans](https://neon.com/docs/introduction/plans)

Test cleanup, backup restore, and queued erasure with synthetic reports. Keep real report collection off until expiry and restore behavior pass in the actual service.

## H052-f — Neon Free candidate and live status

Neon Free with PostgreSQL in Frankfurt is the current candidate. The public pricing page lists $0 and no credit-card requirement. A Frankfurt project was provisioned separately. This proves project provisioning only. It does not verify the application schema migration, Function entrypoint, administrator login, retention trigger, deployment, or live report flow. The local PostgreSQL port passed its tests. No live report was received, and no live reporting flow has passed acceptance.

The owner approved deletion of the previous empty pilot database. The PostgreSQL port is the active implementation. Keep the existing zero-card and $0 budget limits. Keep real report intake off during the synthetic pilot. [Neon pricing](https://neon.com/pricing) · [Neon Functions](https://neon.com/docs/compute/functions/overview)
