import type { OptimizationControlProps, OptimizationView } from "../../lib/optimization";
import "./optimization-control.css";
import { Icon } from "../../components/Icon";

const copy = {
  tr: {
    title: "Sıkıştırma",
    phase: {
      enabled: "Açık",
      paused: "Duraklatıldı",
      ready: "Motor hazır",
      stopped: "Durduruldu",
      starting: "Başlatılıyor",
      stopping: "Durduruluyor",
      failed: "Hata bildirdi",
      unavailable: "Kullanılamıyor",
      unknown: "Bilinmiyor",
    },
    description: {
      enabled: "Yeni istekler sıkıştırılır. Yalnız Headroom’a yönlendirilen trafik işlenir.",
      paused: "Yeni sıkıştırma duraklatıldı. Süren yanıtlar ve önceki içeriğe erişim devam eder.",
      ready: "Motor hazır; bu, tüm istemcilerin bağlı veya tasarrufun ölçülmüş olduğunu göstermez.",
      stopped: "Başlatarak sıkıştırmayı etkinleştir. Bağlantı ayarların korunur.",
      starting: "Motor başlatılıyor; sıkıştırma durumu henüz doğrulanmadı.",
      stopping: "Motor durduruluyor; istemci trafiği ayrıca değerlendirilir.",
      failed: "Motor hata bildirdi; kayıtlı istemci ayarları ayrı durumdadır.",
      unavailable: "Motor durumu okunamıyor.",
      unknown: "Sıkıştırma durumu doğrulanmadı.",
    },
    primary: {
      start: "Başlat",
      pause: "Duraklat",
      resume: "Devam et",
      refresh: "Durumu yenile",
    },
    pending: {
      start: "Başlatılıyor…",
      stop: "Durduruluyor…",
      pause: "Duraklatılıyor…",
      resume: "Devam ediliyor…",
    },
    more: "Motor seçenekleri",
    stop: "Motoru durdur",
    stopWarning: "Motoru durdurmak yönlendirmeyi geri almaz; bağlı uygulamalar bağlantı hatası alabilir.",
  },
  en: {
    title: "Optimization",
    phase: {
      enabled: "Enabled",
      paused: "Paused",
      ready: "Engine ready",
      stopped: "Stopped",
      starting: "Starting",
      stopping: "Stopping",
      failed: "Reported an error",
      unavailable: "Unavailable",
      unknown: "Unknown",
    },
    description: {
      enabled: "New requests are compressed. Only traffic routed through Headroom is processed.",
      paused: "New compression is paused. Current responses and access to earlier content continue.",
      ready: "The engine is ready; this does not show that all clients are connected or savings are measured.",
      stopped: "Start the engine to enable compression. Your connection settings stay in place.",
      starting: "The engine is starting; optimization status is not confirmed yet.",
      stopping: "The engine is stopping; client traffic is assessed separately.",
      failed: "The engine reported an error; saved client settings are separate.",
      unavailable: "Engine status is unavailable.",
      unknown: "Optimization status could not be verified.",
    },
    primary: {
      start: "Start",
      pause: "Pause",
      resume: "Resume",
      refresh: "Refresh status",
    },
    pending: {
      start: "Starting…",
      stop: "Stopping…",
      pause: "Pausing…",
      resume: "Resuming…",
    },
    more: "Engine options",
    stop: "Stop engine",
    stopWarning: "Stopping the engine does not undo routing; connected applications may report connection errors.",
  },
} as const;

type Copy = { [Key in keyof typeof copy.en]: typeof copy.en[Key] extends object
  ? { [NestedKey in keyof typeof copy.en[Key]]: string }
  : string };

export function OptimizationControl({ language, view, onPrimary, onStop }: OptimizationControlProps) {
  const text = copy[language === "tr" ? "tr" : "en"] as Copy;
  const pending = view.pending !== null;
  const phase = view.phase as OptimizationView["phase"];
  const primaryLabel = pending ? text.pending[view.pending!] : text.primary[view.primary];

  return (
    <aside className="overview__engine optimization-control" aria-label={text.title} aria-busy={pending}>
      <div className="overview__engine-status">
        <span className={`overview__status-dot optimization-control__dot is-${phase}`} aria-hidden="true" />
        <div>
          <span>{text.title}</span>
          <strong>{text.phase[phase]}</strong>
        </div>
      </div>
      <p className="optimization-control__description">{text.description[phase]}</p>
      {view.pauseUnavailable && <p className="optimization-control__description" role="status">{language==="tr"?"Bu çalışma için güvenli duraklatma doğrulanamıyor.":"Safe pause is unavailable for this runtime history."}</p>}
      <button className="overview__primary optimization-control__primary" type="button"
        aria-busy={pending} disabled={view.disabled || pending} onClick={onPrimary}>
        <Icon name={view.primary === "pause" ? "pause" : view.primary === "refresh" ? "refresh" : "play"} /><span>{primaryLabel}</span>
      </button>
      <details className="overview__engine-more optimization-control__more">
        <summary>{text.more}<Icon name="chevron" /></summary>
        <p>{text.stopWarning}</p>
        <button type="button" disabled={!view.canStop || pending} onClick={onStop}>
          {view.pending === "stop" ? text.pending.stop : text.stop}
        </button>
      </details>
    </aside>
  );
}
