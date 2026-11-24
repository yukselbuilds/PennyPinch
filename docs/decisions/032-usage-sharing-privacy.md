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
