# H052–H054 — Privacy for optional usage reports

24 November 2026. This decision covers benchmark reports for a project operated from Türkiye. It covers Türkiye and the EU rules that may apply to the service. The release owner must verify the operator, release scope, and data flow before the service accepts real reports.

## Legal basis

Token totals alone can be non-personal. A random installation ID links reports from the same installation. Network metadata and rare combinations of fields can identify a person indirectly. Treat daily installation reports as personal data. Assess whether published totals can still identify or isolate a contributor. [KVKK guidance on anonymization](https://www.kvkk.gov.tr/SharedFolderServer/CMSFiles/8110dc3c-2856-4e54-9129-5e2e375469af.pdf) · [GDPR Article 4 and Recital 26](https://eur-lex.europa.eu/eli/reg/2016/679/oj/eng)

Use voluntary consent for recurring benchmark reports. Show the privacy notice before the consent action. The notice names the controller, the purpose, the data, the collection method and legal basis, the recipients and transfers, retention, and how to exercise rights. Record the consent action and the notice version. Make withdrawal as easy as consent. The core application works with sharing off. [KVKK Articles 5, 10, and 11](https://www.kvkk.gov.tr/Icerik/6649/Personal-Data-Protection-Law) · [KVKK notice and consent decision 2026/347](https://www.kvkk.gov.tr/Icerik/8710/veri-sorumlulari-tarafindan-acik-riza-ve-aydinlatma-metinlerinin-ayri-ayri-duzenlenmesi-gerektigi-hakkinda-kisisel-verileri-koruma-kurulunun-18-02-2026-tarihli-ve-2026-347-sayili-ilke-kararina-iliskin-kamuoyu-duyurusu) · [GDPR Articles 5–7 and 13](https://eur-lex.europa.eu/eli/reg/2016/679/oj/eng)

Check GDPR Article 3 if the release targets people in the EU or monitors their behavior there. Check applicable national ePrivacy rules for access to information on the device. The EDPB explains that locally generated information sent to a server can fall within Article 5(3). Apply the opt-in before the first collection for remote sharing. [GDPR territorial scope](https://www.edpb.europa.eu/system/files/documents/files/file1/edpb_guidelines_3_2018_territorial_scope_after_public_consultation_en_1.pdf) · [EDPB ePrivacy guidance, section 3.2](https://www.edpb.europa.eu/documents/guideline/guidelines-22023-on-technical-scope-of-art-53-of-eprivacy-directive_en)

## Hosting and transfers

The current hosting candidate is Neon Free with PostgreSQL in AWS Europe (Frankfurt). Neon lists the Free plan at $0 and says it requires no credit card. Verify the project's plan, remaining quotas, and billing settings before the synthetic pilot. [Neon pricing](https://neon.com/pricing) · [Neon plans](https://neon.com/docs/introduction/plans)

Neon documents Functions in Frankfurt and says Functions are available on any plan, subject to usage limits. The pilot database is in Frankfurt. Verify the Function region in the deployed service. Review the Function entrypoint, administrator authentication, trigger schedule, and deployment. Review Neon processing terms, subprocessors, provider access, and service logs before the service accepts real reports. [Neon Functions](https://neon.com/docs/compute/functions/overview) · [Neon data processing agreement](https://neon.com/pdf/DPA.pdf)

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

The base service keeps the administrator route closed by default. H052-l provides a local verifier for Managed Better Auth. Live administrator sign-in and the deployed access policy need pilot verification. Do not expose a public query endpoint or accept a user-supplied installation identifier.

Support bounded reads for 1, 7, or 30 completed UTC days. Reject unsupported ranges and excessive row counts. Return only aggregate coverage and counts. Exclude installation IDs and administrator identity.

Keep token counts exact as decimal strings. Calculate weighted savings from total before and after counts within each counter basis. Calculate installation medians only after summing each installation's counts. A zero before count has an undefined percentage. Include measured zero and negative results in coverage. Give the administrator view and any public summary separate privacy acceptance.

The local route tests use a synthetic verifier and reports. They do not prove a deployed login policy or a live summary query.

## H052-d — Private admin panel

The local private-panel renderer is in `services/reports/admin-panel.mjs`. The service serves it only after the injected `verifyAdmin` function approves the request. Local tests cover this dependency. No Neon administrator login or panel deployment has passed acceptance. The panel defaults to seven completed UTC days. It also offers one-day and 30-day ranges.

Show local token estimates, weighted savings, and the per-installation median for each counter basis. Show installations, report days, and comparable, incomparable, and failed requests. Give empty periods, periods without comparable requests, and database failures distinct messages.

Serve aggregate data only. Disable caching. Use nonce-bound styles. Block scripts, framing, and external resources. Test the panel with synthetic reports. Do not claim a private live dashboard until the access policy, queries, and deployment pass.

## H052-e — Bounded retention and scheduled cleanup

Expire reports after 30 days. The local PostgreSQL implementation deletes up to 500 rows per batch and stops after at most 8 batches per call. Exclude expired records from query results during a cleanup delay. Resume a backlog on the next cleanup call. Local expiry passed against PostgreSQL 17.12. H052-q verified the schedule and retry on a temporary Neon Free branch.

Neon documents that function triggers can retry and deliver events more than once or out of order. Make cleanup safe to repeat. Verify actual Free-plan limits and retry behavior before scheduling a trigger. [Neon Function Triggers](https://neon.com/docs/compute/functions/triggers/overview) · [Neon plans](https://neon.com/docs/introduction/plans)

Test cleanup, backup restore, and queued erasure with synthetic reports. Keep real report collection off until expiry and restore behavior pass in the actual service.

## H052-f — Neon Free candidate and live status

The Frankfurt project used Neon Free. Project provisioning preceded the local PostgreSQL port and the H052-h migration. Function deployment, administrator sign-in, scheduled cleanup, and live report traffic require separate acceptance.

The owner approved deletion of the previous empty pilot database. The PostgreSQL port is the active implementation. Keep the existing zero-card and $0 budget limits. Keep real report intake off during the synthetic pilot. [Neon pricing](https://neon.com/pricing) · [Neon Functions](https://neon.com/docs/compute/functions/overview)

## H052-h — Neon schema migration

The Frankfurt pilot passed the checks on PostgreSQL 18.7. The migration created three tables and the storage function in one transaction. The catalog showed direct grants to the object owners only. The function uses the caller's permissions. Its source digest matches the migration.

The database accepted 128 synthetic reports and rejected one additional report. It preserved the original report after both an identical retry and a conflicting retry. It accepted a 4,096-byte JSON object and rejected a 4,097-byte object and a JSON array. The test rolled back all writes. The three tables contain zero rows. [Neon migration evidence](../proofs/h052-neon-migration.json)

## H052-i — Restricted runtime database role

Migration `0002_reports_runtime.sql` defines `pennypinch_reports_runtime`. It grants read and insert access for reports, update access for the counter columns, and permission to call the storage function. It grants no report deletion, table alteration, or administrator access. The pilot role has no login credential. Neon confirmed those grants and the denied operations. The test rolled back the synthetic rows and the temporary role assignment. [Role and Function evidence](../proofs/h052-neon-runtime.json)

Configure the deployed Function with a server-side connection for this role. The local adapter rejects a connection URL for another role and requires certificate verification. The Function handler keeps the intake and administrator routes closed. Verify the live connection and the administrator sign-in before enabling pilot intake. [Neon Function environment variables](https://neon.com/docs/compute/functions/environment-variables)

## H052-j — Live closed Function

The lead agent checked the Neon Free plan in the console. A project-scoped key deployed the Function in Frankfurt. The connection probe and all three report routes returned 404 with no-store. The pilot database stayed empty. The lead agent revoked the key and checked the key list after reload. [Live evidence](../proofs/h052-neon-closed-function.json)

The first attempt supplied a precomputed SCRAM verifier. Neon returned SQLSTATE XX000. Neon documents that SQL password changes require plain text. [Neon role management](https://neon.com/docs/manage/roles#manage-roles-with-sql)

Neon returned HTTP 412 for both schema-only requests. The runner checked the empty pilot, then created a temporary branch from that state. The test branch used the same migration grants. The runner created the role with login disabled. It confirmed that the initial password could not authenticate. The Neon API then returned a replacement password. The runner enabled login after the reset finished. [Neon SDK role provisioning](https://github.com/neondatabase/neon-pkgs/blob/main/packages/sdk/src/neon/resources/roles.ts)

The hosted probe returned 204 over the restricted connection. An unapproved probe and all report routes returned 404. The runner disabled the role and checked that the credential failed. It then removed the branch and revoked the project API key. The parent role remained disabled. Real report intake stays closed. [Live connection evidence](../proofs/h052-neon-restricted-connection.json)

## H052-k — Local scheduled cleanup handler

The cleanup Function uses a dedicated database role. The role can delete from the three retention tables. It can read only the columns that select expired rows. The migration grants no permission to read summaries or add records.

Set `REPORTS_CLEANUP_TRIGGER_ID` to the scheduler's trigger ID. Set `REPORTS_CLEANUP_DATABASE_URL` to the restricted role's connection URL. The public Function URL uses HTTPS. The handler accepts internal HTTP with the expected trigger envelope and the Neon invocation header. Neon documents that its edge strips client-supplied headers with the `X-Neon-` prefix. H052-q verified that the deployed Function rejected a public request with this header. [Neon trigger security](https://neon.com/docs/compute/functions/triggers/overview#confirming-a-request-came-from-neon)

The handler uses its current UTC time for the retention cutoff. A backlog or a database error requests a retry after 60 seconds. The local bundle passed the cutoff, duplicate delivery, restricted role, and retry checks on PostgreSQL. H052-q covers the live schedule. Provider logs, backup, and restore acceptance remain open. [Cleanup evidence](../proofs/h052-neon-retention-local.json)

## H052-l — Local administrator access

Use Neon Managed Better Auth for the administrator Function. Pin the audience in `REPORTS_ADMIN_AUDIENCE`. Set `REPORTS_ADMIN_SUBJECT` to the approved operator's stable identifier. Read the issuer and public key endpoint from the Neon environment. Require EdDSA signatures and a token lifetime of at most 15 minutes. Verify the actual issuer, audience, and subject during pilot sign-in. [Neon Function authentication](https://neon.com/docs/compute/functions/authentication)

Allow a valid bearer token on the private report routes. Use a same-origin POST to create a short-lived browser session for panel navigation. Use a same-origin DELETE to clear its cookie. Verify the token on every read. Keep the cookie host-only, HTTP-only, and limited to HTTPS. The managed provider's sign-out remains part of the browser sign-in test.

The local bundle passed the signed session and summary checks with synthetic keys and native PostgreSQL. The administrator Function keeps intake closed. Keep the operator identifier and connection credentials in server configuration. H052-p verified the deployed token contract. [Administrator access evidence](../proofs/h052-neon-admin-local.json)

## H052-m — Browser sign-in for the administrator

Open the account page at `/admin/login`. Send the credential form to the configured Managed Better Auth service over HTTPS. Require the browser request to come from the same origin. Configure that Function origin in the provider's trusted domains. Keep credential values out of application logs, responses, and storage. [Neon authentication flow](https://neon.com/docs/auth/authentication-flow)

Use the temporary provider session to obtain a signed access token. End the provider session before issuing the panel cookie. Verify the token and the approved administrator subject before issuing that cookie. The account page can clear the panel cookie. The signed token remains valid only until its expiry.

Two local Edge flows passed with synthetic credentials. They covered a failed sign-in, an approved operator, report navigation, and sign-out. H052-p verified the flow with the real provider and synthetic users. Complete production operator enrollment before opening the service. [Browser access evidence](../proofs/h052-neon-admin-browser-local.json)

## H052-n — Report cleanup after restore

Recover the data on an isolated branch. Check the restored branch identity and keep report reads, intake, and scheduled jobs closed while you prepare it. Use the current list of verified erasures from outside the database copy. Apply current credential revocations before reconnecting the service. A restored copy can contain data that the operator erased after its recovery point. [Neon restore limits](https://neon.com/docs/introduction/plans#instant-restore)

Use `prepareRestoredReports(db, erasedInstallationIds)` from `services/reports/restore.mjs` with the authorized maintenance connection. Pass an empty list only after checking that no erasures apply. The function removes each installation's summaries and rate row in one database statement. The daily intake counter has no installation ID and retains its value until expiry.

Repeat the function while it returns `false`. Keep the branch isolated if an operation fails. Confirm that revoked credentials fail. Check that an authorized administrator reads only retained data before you route report traffic to the restored branch.

The native test used a PostgreSQL template copy of synthetic data from before an erasure. The local suite verified rollback on an erasure failure. Neon recovery, the live erasure list, the cutover, and provider backup expiry still need acceptance. [PostgreSQL template copy](https://www.postgresql.org/docs/17/manage-ag-templatedbs.html) · [Restore evidence](../proofs/h052-neon-restore-local.json)

## H052-o — Disclosure checks on field benchmarks

The local test used the private report response with 21 synthetic installations. An earlier response covered 20 installations overall. One basis contained only one installation and displayed its exact totals. A different basis contained 20 installations. One installation supplied 99.01% of its original tokens. The final median matched one installation's rate.

Comparing the one-day and seven-day totals recovered the exact contribution of one installation. Comparing two revisions of the same seven-day report recovered the same counts. The disabled route returned 404. A request without administrator approval returned 401. Both responses returned only a status. [Disclosure evidence](../proofs/h052-aggregate-disclosure.json)

Keep the administrator response private. Before publication, review each proposed measurement basis, breakdown, and period. Check for single contributors and dominance. Check the candidate together with earlier published totals, including revisions and overlapping periods. Remove values that expose a contribution directly or through subtraction. Include counts, coverage, and medians in the review. Approve a specific public aggregate only after this review. [Eurostat disclosure control](https://cros.ec.europa.eu/book-page/statistical-disclosure-control) · [ONS guidance on overlapping totals](https://www.ons.gov.uk/methodology/methodologytopicsandstatisticalconcepts/disclosurecontrol/policyonprotectingconfidentialityintablesofbirthanddeathstatistics)

## H052-p — Live administrator sign-in

The lead agent used a temporary branch from the verified empty pilot. Neon Auth issued signed tokens to two synthetic users. The test verified the trusted Function origin and pinned both Auth URLs to branch metadata. The configured Function used a dedicated origin for its first deployment. Earlier readiness checks returned 404. Their cause remains unverified. [Neon JWT contract](https://neon.com/docs/auth/guides/plugins/jwt)

The allowed user opened the panel in Edge and read the empty summary. The other user and an invalid password returned 401. The cross-origin request returned 403 with no cookie. Sign-out cleared the panel cookie. The provider stored zero active sessions after the test.

The reporting role could not read either Auth table in the test. The intake route returned 404. The lead agent removed the branch and revoked the project key. Production operator enrollment and real report intake remain pending. [Live sign-in evidence](../proofs/h052-neon-admin-live.json)

## H052-q — Live scheduled retention

The first deployed handler rejected a genuine trigger because its request URL used internal HTTP. The handler reported 404 before it queried PostgreSQL. The revised handler accepts internal HTTP and keeps the trusted header, method, path, and envelope checks. The PostgreSQL connection still requires TLS.

Neon Free delivered one scheduled event on the isolated test branch. The first call removed 4,000 expired synthetic reports and two old counters. A retry removed the final expired report after 62 seconds. The cutoff report and two current counters remained. Both public test requests returned 404 and left all rows in place. The lead agent removed the test schedule and branch and revoked the project key.

This test covers one scheduled event and its retry. Provider log retention, backup handling, and point-in-time restore remain open. Real report intake stays closed. [Live retention evidence](../proofs/h052-neon-retention-live.json)
