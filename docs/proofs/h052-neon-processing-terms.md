# H052-t — Neon processing and transfer review

25 November 2026. The lead agent checked the public terms for the Neon Free pilot. H052-s verified the Frankfurt region, six-hour restore window, and one visible Function trace. The review covers the optional report service. [Pilot evidence](h052-neon-provider-retention.json)

## Provider terms and data flow

- The Neon schedule dated 14 October 2026 names Databricks, Inc. as the contracting provider. The schedule refers to the Databricks Master Cloud Services Agreement. Section 1.1.4 of that agreement incorporates the Data Processing Addendum (DPA). The schedule covers self-service plans. Confirm the legal customer, applicable version, and any separate account terms before production. [Neon product schedule](https://neon.com/platform-terms) · [Databricks agreement](https://www.databricks.com/legal/mcsa)
- The linked DPA is version 3, dated 21 July 2023. Sections 2 and 4 cover processing customer personal data and using subprocessors. Sections 8 and 9 cover restricted transfers and deletion. Section 9.3 gives a 30-day period for deletion or assistance after a written request following termination or expiry. The pilot's six-hour window controls access to the PostgreSQL change history. The live test leaves physical expiry of internal copies open. [Databricks DPA](https://www.databricks.com/legal/dpa) · [Neon history window](https://neon.com/docs/postgres/backup-restore/history-window)
- The report design uses the Frankfurt project for PostgreSQL storage and Function processing. A daily report includes a random installation ID, token counts, the operating system family, and versions. The Neon schedule lists Grafana Labs in the United States for infrastructure services. The general subprocessor list gives AWS a customer-selected processing region. Confirm the actual recipients and locations for rollout, including network and service logs, support access, and any production administrator sign-in data. [Neon product schedule, section 3.2](https://neon.com/platform-terms) · [Databricks subprocessors](https://www.databricks.com/legal/databricks-subprocessors)
- The Databricks agreement addresses provider usage data separately. The provider's privacy notice says service usage data can include IP addresses, account IDs, and email addresses. H052-s covers only the project-visible part of one trace. Verify which data the provider records for the chosen setup and the retention period for each log category. [Databricks agreement, sections 3.3 and 11.25](https://www.databricks.com/legal/mcsa) · [Databricks privacy notice](https://www.databricks.com/legal/privacynotice)

## Türkiye and the EU

The KVKK Authority's transfer page reported no adequacy determination for countries on the review date. This project has no verified adequacy decision for its transfer. The daily uploads are regular transfers. Keep the opt-in for collection. Verify a separate Article 9 basis before sending reports abroad. The occasional transfer exceptions do not cover the planned daily uploads. [KVKK transfer rules](https://www.kvkk.gov.tr/Icerik/2053/Yurtdisina-Aktarim)

Article 9 requires a processing basis and, for this safeguard, enforceable rights and effective remedies. The controller-to-processor standard contract is a candidate when the Turkish operator acts as controller and the provider acts as processor. Confirm that the importing entity will sign the official Turkish form for the account. Complete the annexes and preserve the published clauses except for permitted choices. If the parties sign, notify the Authority within five business days. Include onward transfers in the review. [KVKK standard contracts](https://www.kvkk.gov.tr/Icerik/7929/Standart-Sozlesmeler) · [KVKK signature and notification rules](https://www.kvkk.gov.tr/Icerik/8170/Yurt-Disina-Kisisel-Veri-Aktariminda-Kullanilacak-Standart-Sozlesmelerde-Dikkat-Edilmesi-Gereken-Hususlara-Iliskin-Kamuoyu-Duyurusu)

The Databricks DPA refers to the EU standard contractual clauses for covered transfers. If GDPR applies to the release, verify the Article 28 terms and the Chapter V safeguard for each transfer that needs it. Apply KVKK Article 9 separately to the planned transfer from Türkiye. [Databricks DPA, section 8 and Annex B](https://www.databricks.com/legal/dpa) · [European Commission transfer rules](https://commission.europa.eu/law/law-topic/data-protection/international-dimension-data-protection/rules-international-data-transfers_en)

## Open production checks

- Confirm the operator's legal identity and the contact for rights requests.
- Verify the applicable processing terms and legal recipient for this Free account. Confirm a signed Turkish standard contract and its notification, or document another applicable Article 9 basis.
- Confirm the provider's actual log fields, processing locations, and retention for this service. Confirm how erasure applies to backup copies and how to handle restored data.
- Complete the notice and production cutover checks. Keep real report intake closed while these checks remain open.

The public document review is complete. The production transfer basis and account-specific evidence remain open.
