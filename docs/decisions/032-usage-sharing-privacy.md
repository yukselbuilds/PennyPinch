# H052–H054 — Privacy for optional usage reports

23 November 2026. This decision covers benchmark reports for a project operated from Türkiye. It covers Türkiye and the EU rules that may apply to the service. The release owner must confirm the operator, launch scope, and data flow before the service accepts real reports.

## Legal basis

Token totals alone can be non-personal. A random installation ID links reports from the same installation. Network metadata and rare combinations of fields can identify a person indirectly. Treat daily installation reports as personal data. Assess whether published totals can still identify or isolate a contributor. [KVKK guidance on anonymization](https://www.kvkk.gov.tr/SharedFolderServer/CMSFiles/8110dc3c-2856-4e54-9129-5e2e375469af.pdf) · [GDPR Article 4 and Recital 26](https://eur-lex.europa.eu/eli/reg/2016/679/oj/eng)

Use voluntary consent for recurring benchmark reports. Show the privacy notice before the consent action. The notice names the controller, the purpose, the data, the collection method and legal basis, the recipients and transfers, retention, and how to exercise rights. Record the consent action and the notice version. Make withdrawal as easy as consent. The core application works with sharing off. [KVKK Articles 5, 10, and 11](https://www.kvkk.gov.tr/Icerik/6649/Personal-Data-Protection-Law) · [KVKK notice and consent decision 2026/347](https://www.kvkk.gov.tr/Icerik/8710/veri-sorumlulari-tarafindan-acik-riza-ve-aydinlatma-metinlerinin-ayri-ayri-duzenlenmesi-gerektigi-hakkinda-kisisel-verileri-koruma-kurulunun-18-02-2026-tarihli-ve-2026-347-sayili-ilke-kararina-iliskin-kamuoyu-duyurusu) · [GDPR Articles 5–7 and 13](https://eur-lex.europa.eu/eli/reg/2016/679/oj/eng)

Check GDPR Article 3 if the release targets people in the EU or monitors their behavior there. Check applicable national ePrivacy rules for access to information on the device. The EDPB explains that locally generated information sent to a server can fall within Article 5(3). Apply the opt-in before the first collection for remote sharing. [GDPR territorial scope](https://www.edpb.europa.eu/system/files/documents/files/file1/edpb_guidelines_3_2018_territorial_scope_after_public_consultation_en_1.pdf) · [EDPB ePrivacy guidance, section 3.2](https://www.edpb.europa.eu/documents/guideline/guidelines-22023-on-technical-scope-of-art-53-of-eprivacy-directive_en)

## Hosting and transfers

Daily sends are regular transfers. For transfers abroad subject to KVKK, check for an applicable adequacy decision or establish an appropriate safeguard under Article 9. Reserve the Article 9 consent exception for occasional transfers that meet its conditions. If the project uses the Turkish standard contract, secure the parties' signatures and notify the KVKK Authority within five business days of signing. [KVKK transfer rules](https://www.kvkk.gov.tr/Icerik/2053/Yurtdisina-Aktarim) · [KVKK standard contract notification](https://www.kvkk.gov.tr/Icerik/8043/Standart-Sozlesme-Bildirim-Modulu-Hakkinda-Kamuoyu-Duyurusu)

Cloudflare Free is the hosting candidate. Review the account's processing terms, contracting entity, subprocessors, access, and data locations. The public DPA includes EU transfer clauses. Confirm a Turkish transfer safeguard for the actual service where required. If GDPR applies, verify the processor terms under Article 28 and the applicable Chapter V transfer mechanism. D1's EU jurisdiction limits where the database runs and persists. Workers can access it worldwide. Check request processing and infrastructure logs separately. [Cloudflare DPA](https://www.cloudflare.com/cloudflare-customer-dpa/) · [D1 data location](https://developers.cloudflare.com/d1/configuration/data-location/) · [Workers Logs](https://developers.cloudflare.com/workers/observability/logs/workers-logs/)

## Data lifecycle and acceptance

- Start daily reports after opt-in. Use measurements made after that choice. Show a payload preview and the last send status. Declining, upgrading, or restarting an opted-out installation must result in zero report uploads.
- Accept only the fields in H054. Exclude prompts, code, paths, account and chat IDs, and secrets. Keep IP addresses and request contents out of application storage and logs. Verify the actual provider logging settings and disclose applicable infrastructure processing.
- On withdrawal, stop new uploads and clear the queue. Test withdrawal during a retry or active upload. Check the server outcome for an upload that was in flight. Use an authorized method to handle access and erasure for earlier reports. Erase personal reports after withdrawal when the law requires it. Provide a contact for requests.
- Remove daily reports from active storage after 30 days. Document backup retention and enforce earlier deletions before a restored database serves requests. D1 Time Travel currently allows seven days of recovery on Free. Verify this in the actual deployment. [D1 Time Travel](https://developers.cloudflare.com/d1/reference/time-travel/)
- Test aggregates for re-identification before long-term retention or publication. A small group, a dominant contributor, or the difference between published totals can expose one installation. Retain and publish only aggregates that pass this check. Show sample size, dates, measurement basis, and coverage. Apply H052's weighted net calculation.
- Give manual feedback its own notice and retention policy. Preview its text and optional metadata before send. Review the processing basis and hosting route for that data before opening the endpoint.

Resolve the controller, notice, rights contact, and transfer basis before accepting real reports. Verify these checks against the actual deployment. Use synthetic reports for prelaunch tests. Keep collection off while these items are open.

## H052-a — Daily usage storage

The storage module accepts a version 1 JSON summary of at most 4,096 bytes. Each summary has a random version 4 UUID, a completed UTC day, an OS family, application and engine versions, and a notice version. Application versions use numeric release or preview versions. The schema accepts notice version 1. H054 will bind the notice version to the user's consent action before sending.

The module accepts the six known Headroom 0.45.0 WebSocket counter bases. The bases measure the whole request input. Each group has its own comparable request count and before and after token totals. The report keeps incomparable and failed request counts separate. A failed request belongs to the failed count only. A report can carry coverage counts when no measured pair exists. The module keeps zero and negative savings.

The database stores one summary per installation and UTC day. It accepts identical retries and refuses changed summaries. The write and its result read run in one D1 transaction. The module accepts the 30 most recent completed UTC days. Each purge call deletes up to 500 expired rows. The first deployment needs a schedule for these calls. [D1 transactions](https://developers.cloudflare.com/d1/worker-api/d1-database/#batch)

The local Miniflare tests use synthetic summaries. Run them from the source checkout.

```powershell
pnpm.cmd --dir services/reports install --ignore-scripts --frozen-lockfile
pnpm.cmd --dir services/reports test
```

The next slice adds authenticated intake and request limits. Administrator access, collection, deployment, and aggregate publication each require their own verification. [Storage evidence](../proofs/h052-daily-usage-storage.json) · [Cloudflare local D1 tests](https://developers.cloudflare.com/d1/best-practices/local-development/)

## H052-b — Daily report HTTP intake

The Worker accepts POST requests at `/v1/usage` over HTTPS. Set `REPORTS_ENABLED` to `true` only for an approved pilot. Bind the `REPORTS_DB` database and the `USAGE_RATE` rate limiter. The Worker keeps the route closed by default. It returns fixed status names and prevents response caching.

The pilot gives each installation its own bearer credential. Generate 32 random bytes for each credential. Encode them as 43 base64url characters without padding. Use `PILOT_USAGE_CREDENTIALS` to bind up to 32 SHA-256 digests of these strings to installation UUIDs. Store the digests as lowercase hexadecimal keys in a JSON object. Keep this configuration on the Worker. Remove a digest to revoke its credential. The Worker accepts only the installation UUID bound to the credential.

Configure six requests per installation per 60 seconds in the rate limiter. The rate limit applies per Cloudflare location and can allow extra requests during propagation. The Worker limits the input to 4,096 bytes as it reads the body. [Cloudflare Rate Limiting](https://developers.cloudflare.com/workers/runtime-apis/bindings/rate-limit/)

D1 accepts up to 128 new reports per UTC day for the service. The transaction reserves a slot only for a new row. An identical retry uses the existing row. Deleting an accepted report preserves the day's quota count. When the daily quota is full, the Worker returns HTTP 429 with the time until the next UTC day. Apply migration 0002 after migration 0001 to keep earlier summaries. [D1 batch transactions](https://developers.cloudflare.com/d1/worker-api/d1-database/#batch)

The local tests use synthetic credentials and reports. Administrator access and bounded queries come next. Verify the account limits, logging, retention schedule, and privacy conditions before live collection. [Intake evidence](../proofs/h052-report-intake.json)

## H052-c — Administrator report queries

The Worker serves GET requests at `/v1/admin/summary`. Bind `ADMIN_ACCESS_ISSUER` to the Cloudflare Access team origin. Set `ADMIN_ACCESS_AUDIENCE` to the audience of the administrator Access application. Apply an Access policy that grants access to authorized administrators. The route stays closed when these bindings are absent.

The Worker verifies the RS256 signature, issuer, audience, and validity times of the Access assertion. It requires an application token for an authenticated identity. It reads public signing keys from the configured team origin. It returns aggregate data with caching disabled. The response excludes installation IDs and administrator identity. [Cloudflare Access JWT validation](https://developers.cloudflare.com/cloudflare-one/access-controls/applications/http-apps/authorization-cookie/validating-json/)

Request 1, 7, or 30 completed UTC days. The default is seven days. D1 reads at most 4,097 rows, which includes one row for overflow detection. The Worker rejects a summary when the range contains more than 4,096 rows. It reports contributing installations, report days, comparable requests, incomparable requests, and failed requests.

The summary keeps token counts exact as decimal strings. It calculates weighted savings from the total before and after counts for each counter basis. It sums each installation's counts before calculating the median percentage. A zero before count gives an undefined percentage. The coverage still includes the measured zero and negative results. The administrator view and any public summary need their own acceptance.

The local tests use generated keys and synthetic reports. Verify the live Access policy before granting administrator access. [Summary evidence](../proofs/h052-admin-summary.json)

## H052-d — Private admin panel

The Worker verifies the signed Access identity before it serves GET `/admin`. Protect `/admin` and `/v1/admin/summary` with the same administrator policy in the live deployment. The default range covers seven completed UTC days. Select one or 30 days for other ranges.

The panel reads the same aggregate D1 summary as the JSON route. It shows local token estimates, weighted savings, and the median per installation for each counter basis. Report coverage includes installations, daily reports, and comparable, incomparable, and failed requests. An empty period, a period without comparable requests, and a database failure have distinct messages.

The Worker generates the HTML and disables caching. The panel contains aggregate measurements only. It uses nonce-bound styles and blocks scripts, framing, and external resources. The local browser tests use synthetic reports and generated Access credentials. Verify the live Access policy and the D1 limits before the pilot. [Panel evidence](../proofs/h052-admin-panel.json)

## H052-e — Retention timer and deployment preflight

The cleanup trigger runs at minute five of each UTC hour. Each run uses the scheduled time to preserve the 30 most recent completed UTC days. It deletes at most 500 expired reports per batch and stops after eight batches. If all eight batches fill, the run reports a backlog and the next trigger continues cleanup. Cleanup runs when the Worker disables intake. [Cron Triggers](https://developers.cloudflare.com/workers/configuration/cron-triggers/)

The preflight config uses a test database ID and keeps intake, Worker URLs, preview URLs, and Worker observability off. Wrangler 4.167.0 built the bundle in a dry run. It applied both migrations in an isolated local D1 database. Use `--dry-run` to build and `--local` to apply the migrations locally. [Preflight config](../../services/reports/wrangler.preflight.jsonc) · [Preflight evidence](../proofs/h052-deployment-preflight.json)

The owner account showed Workers Free and zero D1 databases on 24 November. The Zero Trust dashboard showed initial setup. Workers Free allows 100,000 requests per day and 10 ms of CPU time per HTTP request or cron event. It allows five cron triggers per account. D1 Free allows ten databases, five million row reads per day, and 100,000 row writes per day. Verify account usage and CPU time during the synthetic pilot. [Workers limits](https://developers.cloudflare.com/workers/platform/limits/) · [D1 pricing](https://developers.cloudflare.com/d1/platform/pricing/)

Use a dedicated Free Worker and a private D1 database with the EU jurisdiction for the synthetic pilot. Protect the Worker with an Access policy for the approved administrator. Keep preview URLs off. Use the account's `workers.dev` address for this test. The owner completes any required Zero Trust Free subscription setup. [Workers Access](https://developers.cloudflare.com/workers/configuration/cloudflare-access/) · [Zero Trust setup](https://developers.cloudflare.com/cloudflare-one/setup/) · [D1 data location](https://developers.cloudflare.com/d1/configuration/data-location/)

Use only synthetic reports during validation of the route, access policy, cleanup, and restore. Keep real report collection off pending the controller, notice, transfer, and retention review. D1 Free offers seven days of Time Travel. Delete expired reports and apply queued erasures before a restored database serves requests. [D1 Time Travel](https://developers.cloudflare.com/d1/reference/time-travel/)

## H052-f — Private pilot database

Cloudflare created a separate D1 database for the pilot. Its Settings page showed the EU jurisdiction. D1 Console returned zero application tables in a read-only query. The Time Travel page showed a seven-day restore window. [Provisioning evidence](../proofs/h052-private-pilot-d1.json) · [D1 Time Travel](https://developers.cloudflare.com/d1/reference/time-travel/)

Zero Trust Free requires payment details at activation. The pilot is on hold under the zero-cost constraint. Resume on a free path that meets the access and privacy criteria. Apply the two migrations to the pilot database and use synthetic reports to verify the live Worker and cleanup. [D1 migrations](https://developers.cloudflare.com/d1/reference/migrations/) · [Zero Trust setup](https://developers.cloudflare.com/cloudflare-one/setup/)
