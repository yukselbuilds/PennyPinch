import { invoke, isTauri } from "@tauri-apps/api/core";
import { useState } from "react";
import type { PreferenceLanguage } from "../../lib/preferences";

const copy = {
  tr: {
    body: "Bir Codex veya Claude Code oturum dosyası (.jsonl) seç. Learn, tekrar eden hataları cihazında inceleyip öneri gösterir.",
    action: "Oturum dosyası seç",
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
type Result = { status: "ready"; source: "codex" | "claude_code"; suggestions: Suggestion[] }
  | { status: keyof typeof copy.en.errors | "busy" | "cancelled" };

export function Learn({ language }: { language: PreferenceLanguage }) {
  const text = copy[language];
  const numbers = new Intl.NumberFormat(language);
  const available = isTauri() && navigator.userAgent.includes("Windows NT");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<Result | null>(null);

  async function select() {
    setBusy(true);
    try {
      const reply = await invoke<Result>("preview_learn", { turkish: language === "tr" });
      if (reply.status !== "cancelled") setResult(reply);
    } catch {
      setResult({ status: "failed" });
    } finally {
      setBusy(false);
    }
  }

  const status = busy || result?.status === "busy" ? text.busy
    : !available ? text.errors.unavailable
    : result?.status === "ready" ? (result.suggestions.length === 0 ? text.empty : text.ready)
    : result && result.status !== "cancelled" ? text.errors[result.status]
    : "";
  return <section className="operating-settings" aria-labelledby="learn-title">
    <div className="operating-settings__heading"><h3 id="learn-title">Learn</h3><p>{text.body}</p></div>
    <div className="operating-settings__section operating-settings__section--status">
      <button className="button" type="button" disabled={!available || busy} onClick={() => void select()}>{text.action}</button>
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
    </div>
  </section>;
}
