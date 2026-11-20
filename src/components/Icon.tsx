import type { CSSProperties } from "react";

const paths = {
  overview: "M3.5 3.5h6v6h-6zM14.5 3.5h6v6h-6zM3.5 14.5h6v6h-6zM14.5 14.5h6v6h-6z",
  applications: "M8 3v4m8-4v4M6 7h12v4a6 6 0 0 1-12 0V7Zm6 10v4",
  sessions: "M5 4h14a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H9l-6 4V6a2 2 0 0 1 2-2ZM7 9h10M7 13h6",
  activity: "M3 12h4l3-7 4 14 3-7h4",
  settings: "M4 7h9m4 0h3M4 17h3m4 0h9M13 4v6M7 14v6",
  shield: "m12 3 8 3v5c0 5-5 8-8 10-3-2-8-5-8-10V6l8-3Zm-4 9 3 3 5-6",
  arrow: "M5 12h14m-5-5 5 5-5 5",
  chevron: "m9 5 7 7-7 7",
  play: "m8 5 11 7-11 7V5Z",
  pause: "M8 5v14M16 5v14",
  refresh: "M20 7v5h-5M4 17v-5h5M6.2 7a7 7 0 0 1 11.6-1L20 9M4 15l2.2 3A7 7 0 0 0 17.8 17",
  chart: "M4 4v16h16M8 15v-4m5 4V7m5 8v-6",
  terminal: "m5 7 5 5-5 5m8 0h6",
  asterisk: "M12 3v18M4.2 7.5l15.6 9M4.2 16.5l15.6-9",
  code: "m8 6-6 6 6 6m8-12 6 6-6 6m-3-14-2 16",
  power: "M12 3v9M6.2 5.8a8 8 0 1 0 11.6 0",
} as const;

export function Icon({name, className = "", style}: {name: keyof typeof paths; className?: string; style?: CSSProperties}) {
  return <svg className={`ui-icon ${className}`} width="20" height="20" viewBox="0 0 24 24" fill="none"
    stroke="currentColor" strokeWidth="1.65" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false" style={style}>
    <path d={paths[name]} />
  </svg>;
}

export function PennyPinchMark() {
  return <svg className="brand-mark" width="28" height="28" viewBox="0 0 28 28" fill="none" aria-hidden="true" focusable="false">
    <path d="M4 6.5A2.5 2.5 0 0 1 6.5 4H10v9H4V6.5Zm0 10.5h6v7H6.5A2.5 2.5 0 0 1 4 21.5V17Zm10-13h7.5A2.5 2.5 0 0 1 24 6.5V24h-6V10h-4V4Z" fill="currentColor" />
  </svg>;
}
