import { invoke, isTauri } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import type { PreferenceLanguage } from "../../lib/preferences";

const copy = {
  tr: {
    body: "Bir Codex veya Claude Code oturum dosyası (.jsonl) seç. Learn, tekrar eden hataları cihazında inceleyip öneri gösterir.",
    action: "Oturum dosyası seç",
    experimentalTitle: "Deneysel kurallar",
    experimentalBody: "Seçtiğin oturumdan yerel kural adaylarını incele.",
    experimentalAction: "Kural adaylarını önizle",
    experimentalReady: "Kural adayları hazır. Kaynak değişiklikleriyle birlikte değerlendir.",
    trialTitle: "Yinelenen günlük satırları",
    trialBody: "Özgün dosyaya bu ekranda 15 dakika erişebilirsin. Deneme sonuçları birebir geri açılarak doğrulanır.",
    trialAction: "Yerelde dene",
    trialBusy: "Yerel deneme sürüyor…",
    trialExact: "Özgün çıktı birebir geri alındı.",
    trialBytes: "Denenen stdout",
    trialCounts: (eligible: string, compacted: string, protectedCount: string) => `${eligible} uygun çıktı · ${compacted} küçültüldü · ${protectedCount} korundu`,
    trialUnknown: (count: string) => `${count} çıktının çağrı veya oturum kapsamı için yeterli kanıt yok.`,
    trialEmpty: "Bu dosyada denemeye uygun, stdout ve çıkış durumu açık bir günlük kaydı bulunamadı.",
    busy: "Dosya seçimi veya inceleme sürüyor…",
    ready: "Learn önerileri hazır.",
    empty: "Bu dosyada aynı çağrının art arda en az üç kez başarısız olduğu bir durum bulunamadı.",
    count: (failures: string, line: string) => `${failures} başarısız çağrı · İlk örnek ${line}. satır`,
    recovered: (count: string) => `Aynı çağrı ${count} örnekte bir sonraki denemede başarılı oldu.`,
    errors: {
      unavailable: "Learn önizlemesi bu derlemede kullanılamıyor.",
      unreadable: "Seçilen dosya okunamadı.",
      invalid_file: "Geçerli bir Codex veya Claude Code oturum dosyası seç.",
      too_large: "Dosya sınırı 16 MiB ve 64.000 satır.",
      timed_out: "İnceleme zaman sınırını aştı. Daha küçük bir oturum dosyası dene.",
      failed: "Learn önizlemesi alınamadı. Yeniden deneyebilirsin.",
      expired: "Özgün kaydın erişim süresi doldu. Oturum dosyasını yeniden seç.",
    },
    suggestions: {
      path: "İlk denemeden önce çalışma klasörünü ve dosya yolunu doğrula.",
      dependency: "Projenin komutunu ve gerekli bağımlılıkları çalıştırmadan önce doğrula.",
      permission: "İşlem için gereken erişim yetkisini ve dosya sahipliğini kontrol et.",
      timeout: "Yavaş adımı ayırıp daha dar kapsamlı bir kontrol dene.",
      failure: "Çağrıyı tekrarlamadan önce ilk hata çıktısını incele.",
    },
  },
  en: {
    body: "Choose a Codex or Claude Code session file (.jsonl). Learn looks for repeated failures on your device and shows suggestions.",
    action: "Choose session file",
    experimentalTitle: "Experimental rules",
    experimentalBody: "Review local rule candidates from a session you choose.",
    experimentalAction: "Preview rule candidates",
    experimentalReady: "Rule candidates are ready. Review them alongside source changes.",
    trialTitle: "Repeated log lines",
    trialBody: "The original file is available on this screen for 15 minutes. Trial results are decoded to verify exact recovery.",
    trialAction: "Try locally",
    trialBusy: "Running the local trial…",
    trialExact: "The original output was recovered exactly.",
    trialBytes: "Tested stdout",
    trialCounts: (eligible: string, compacted: string, protectedCount: string) => `${eligible} eligible outputs · ${compacted} compacted · ${protectedCount} protected`,
    trialUnknown: (count: string) => `The call or session scope of ${count} outputs has insufficient evidence.`,
    trialEmpty: "This file has no eligible log with explicit stdout and exit status.",
    busy: "Waiting for file selection or analysis…",
    ready: "Learn suggestions are ready.",
    empty: "This file has no call that failed at least three times in a row.",
    count: (failures: string, line: string) => `${failures} failed calls · First example on line ${line}`,
    recovered: (count: string) => `The same call succeeded on the next attempt in ${count} cases.`,
    errors: {
      unavailable: "Learn preview is unavailable in this build.",
      unreadable: "The selected file could not be read.",
      invalid_file: "Choose a valid Codex or Claude Code session file.",
      too_large: "The file limit is 16 MiB and 64,000 lines.",
      timed_out: "Analysis exceeded the time limit. Try a smaller session file.",
      failed: "Learn preview failed. You can try again.",
      expired: "Access to the original capture has expired. Choose the session file again.",
    },
    suggestions: {
      path: "Confirm the working directory and file path before the first attempt.",
      dependency: "Verify the project's command and dependencies before running it.",
      permission: "Check the operation's permissions and file ownership.",
      timeout: "Isolate the slow step and try a narrower check.",
      failure: "Inspect the first error before retrying the call.",
    },
  },
};

type Suggestion = {
  kind: keyof typeof copy.en.suggestions;
  failures: number;
  recoveries: number;
  first_line: number;
};
type Result = { status: "ready"; source: "codex" | "claude_code"; suggestions: Suggestion[]; replay_id?: string }
  | { status: keyof typeof copy.en.errors | "busy" | "cancelled" };

type TrialResult = { status: "trial_ready"; report: {
  rule: "repeated_log_lines_v1";
  checked: number;
  protected: number;
  ambiguous: number;
  eligible: number;
  compacted: number;
  before_bytes: number;
  after_bytes: number;
  round_trip_exact: boolean;
} } | { status: keyof typeof copy.en.errors | "busy" };

function LocalTrial({ language, id, enabled }: { language: PreferenceLanguage; id: string; enabled: boolean }) {
  const text = copy[language];
  const numbers = new Intl.NumberFormat(language);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<TrialResult | null>(null);
  const request = useRef(0);
  useEffect(() => () => { request.current += 1; }, []);

  async function run() {
    const sequence = ++request.current;
    setBusy(true);
    try {
      const reply = await invoke<TrialResult>("replay_rules", { replayId: id });
      if (request.current === sequence) setResult(reply);
    } catch {
      if (request.current === sequence) setResult({ status: "failed" });
    } finally {
      if (request.current === sequence) setBusy(false);
    }
  }

  const report = result?.status === "trial_ready" ? result.report : null;
  const status = busy || result?.status === "busy" ? text.trialBusy
    : result && result.status !== "trial_ready" ? text.errors[result.status] : "";
  return <section aria-label={text.trialTitle}>
    <h4>{text.trialTitle}</h4><p className="operating-settings__status-line">{text.trialBody}</p>
    <button className="button" type="button" disabled={!enabled || busy} onClick={() => void run()}>{text.trialAction}</button>
    <div className="operating-settings__status-line" role="status">
      {status}
      {report && <>
        <p>{report.eligible > 0 ? text.trialExact : text.trialEmpty}</p>
        {report.eligible > 0 && <p>{text.trialBytes} · {numbers.format(report.before_bytes)} → {numbers.format(report.after_bytes)} {language === "tr" ? "bayt" : "bytes"}</p>}
        <p>{text.trialCounts(numbers.format(report.eligible), numbers.format(report.compacted), numbers.format(report.protected))}</p>
        {report.ambiguous > 0 && <p>{text.trialUnknown(numbers.format(report.ambiguous))}</p>}
      </>}
    </div>
  </section>;
}

function Preview({ language, experimental = false, enabled = true }: {
  language: PreferenceLanguage;
  experimental?: boolean;
  enabled?: boolean;
}) {
  const text = copy[language];
  const numbers = new Intl.NumberFormat(language);
  const available = isTauri() && navigator.userAgent.includes("Windows NT");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<Result | null>(null);
  const request = useRef(0);
  useEffect(() => () => { request.current += 1; }, []);
  const replayId = result?.status === "ready" ? result.replay_id : undefined;
  useEffect(() => {
    if (!experimental || !replayId) return;
    const release = () => { void invoke("release_rule_preview", { replayId }).catch(() => {}); };
    const timer = setTimeout(() => { release(); setResult({ status: "expired" }); }, 15 * 60 * 1000);
    return () => { clearTimeout(timer); release(); };
  }, [experimental, replayId]);

  async function select() {
    const sequence = ++request.current;
    setBusy(true);
    try {
      const reply = await invoke<Result>("preview_learn", {
        turkish: language === "tr", ...(experimental ? { experimentalRules: true } : {}),
      });
      if (request.current === sequence && reply.status !== "cancelled") setResult(reply);
      else if (request.current !== sequence && reply.status === "ready" && reply.replay_id) {
        void invoke("release_rule_preview", { replayId: reply.replay_id }).catch(() => {});
      }
    } catch {
      if (request.current === sequence) setResult({ status: "failed" });
    } finally {
      if (request.current === sequence) setBusy(false);
    }
  }

  const status = busy || result?.status === "busy" ? text.busy
    : !available ? text.errors.unavailable
    : result?.status === "ready" ? (result.suggestions.length === 0 ? text.empty : experimental ? text.experimentalReady : text.ready)
    : result && result.status !== "cancelled" ? text.errors[result.status]
    : "";
  return <div className="operating-settings__section operating-settings__section--status">
      <button className="button" type="button" disabled={!available || busy || !enabled} onClick={() => void select()}>
        {experimental ? text.experimentalAction : text.action}
      </button>
      <div className="operating-settings__status-line" role="status">{status}</div>
      {!busy && result?.status === "ready" && result.suggestions.length > 0 && <>
        <h4>{result.source === "codex" ? "Codex" : "Claude Code"}</h4>
        <ul className="learn-suggestions">
          {result.suggestions.map(({ kind, failures, recoveries, first_line }) => <li key={kind}>
            <p className="operating-settings__status-line"><strong>{text.suggestions[kind]}</strong><br />
              {text.count(numbers.format(failures), numbers.format(first_line))}
              {recoveries > 0 && <><br />{text.recovered(numbers.format(recoveries))}</>}
            </p>
          </li>)}
        </ul>
      </>}
      {experimental && replayId && <LocalTrial key={replayId} language={language} id={replayId} enabled={enabled && !busy} />}
    </div>;
}

export function Learn({ language, experimental, preferencesReady, onExperimentalChange }: {
  language: PreferenceLanguage;
  experimental: boolean;
  preferencesReady: boolean;
  onExperimentalChange: (enabled: boolean) => void;
}) {
  const text = copy[language];
  return <section className="operating-settings" aria-labelledby="learn-title">
    <div className="operating-settings__heading"><h3 id="learn-title">Learn</h3><p>{text.body}</p></div>
    <Preview language={language} />
    <section aria-labelledby="experimental-rules-title">
      <label className="setting-row">
        <span><strong id="experimental-rules-title">{text.experimentalTitle}</strong><small>{text.experimentalBody}</small></span>
        <input type="checkbox" aria-label={text.experimentalTitle} checked={experimental} disabled={!preferencesReady}
          onChange={event => onExperimentalChange(event.target.checked)} />
      </label>
      {experimental && <Preview language={language} experimental enabled={preferencesReady} />}
    </section>
  </section>;
}
