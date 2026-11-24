const integer = new Intl.NumberFormat("en-US");
const rate = new Intl.NumberFormat("en-US", { style: "percent", maximumFractionDigits: 2 });
const date = new Intl.DateTimeFormat("en-US", { dateStyle: "medium", timeZone: "UTC" });
const htmlEntities = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };

function escapeHtml(text) {
  return text.replace(/[&<>"']/g, (character) => htmlEntities[character]);
}

function metric(label, value) {
  return `<div><dt>${label}</dt><dd>${value}</dd></div>`;
}

function savingsRate(percent) {
  if (percent === null) return "Unavailable";
  if (percent > 0 && percent < 0.01) return "&lt;0.01% saved";
  if (percent < 0 && percent > -0.01) return "&lt;0.01% more";
  return rate.format(percent / 100);
}

function counter(group) {
  const positive = group.weighted_saved_percent > 0 ? " positive" : "";
  return `<article class="counter">
    <h3><code>${escapeHtml(group.basis)}</code></h3>
    <dl class="rates${positive}">
      ${metric("Weighted savings", savingsRate(group.weighted_saved_percent))}
      ${metric("Median per installation", savingsRate(group.median_installation_saved_percent))}
    </dl>
    <dl class="totals">
      ${metric("Original tokens", integer.format(BigInt(group.before_tokens)))}
      ${metric("Optimized tokens", integer.format(BigInt(group.after_tokens)))}
      ${metric("Tokens saved", integer.format(BigInt(group.saved_tokens)))}
      ${metric("Comparable requests", integer.format(BigInt(group.comparable_requests)))}
      ${metric("Installations", integer.format(group.installations))}
      ${metric("Installations in median", integer.format(group.installations_with_percentage))}
    </dl>
  </article>`;
}

function panelContent({ coverage, groups }) {
  if (coverage.report_days === 0) return '<p class="message">No reports in this period.</p>';
  return `<section class="coverage" aria-labelledby="coverage">
    <h2 id="coverage">Report coverage</h2>
    <dl>
      ${metric("Installations", integer.format(coverage.installations))}
      ${metric("Daily reports", integer.format(coverage.report_days))}
      ${metric("Comparable requests", integer.format(BigInt(coverage.comparable_requests)))}
      ${metric("Incomparable requests", integer.format(BigInt(coverage.incomparable_requests)))}
      ${metric("Failed requests", integer.format(BigInt(coverage.failed_requests)))}
    </dl>
  </section>
  ${groups.length === 0 ? '<p class="message">No comparable requests in this period.</p>' : `
  <section aria-labelledby="savings">
    <h2 id="savings">Token savings by counter</h2>
    <p class="description">Token counts are local estimates. Weighted savings divides saved tokens by original tokens. The median uses one rate per installation. Negative values show an increase.</p>
    <div class="counters">${groups.map(counter).join("")}</div>
  </section>`}`;
}

function period({ from_day, to_day_exclusive }) {
  const lastDay = new Date(Date.parse(to_day_exclusive) - 86_400_000).toISOString().slice(0, 10);
  const time = (day) => `<time datetime="${day}">${date.format(new Date(day))}</time>`;
  const range = from_day === lastDay ? time(lastDay) : `${time(from_day)} – ${time(lastDay)}`;
  return `<p class="period">${range} <span>Completed UTC days</span></p>`;
}

const styles = `
  :root {
    color-scheme: light dark;
    --background: #181c1b;
    --surface: #202624;
    --raised: #2b3732;
    --line: #394840;
    --text: #f0f5f2;
    --muted: #b7c6bd;
    --accent: #98f0bb;
    font-family: "Segoe UI Variable", "Segoe UI", system-ui, sans-serif;
    font-size: 14px;
    line-height: 1.55;
    background: var(--background);
    color: var(--text);
  }
  @media (prefers-color-scheme: light) {
    :root {
      --background: #f4f7f1;
      --surface: #fff;
      --raised: #e7f1e9;
      --line: #ccd9ce;
      --text: #18241e;
      --muted: #43574a;
      --accent: #1f6948;
    }
  }
  * { box-sizing: border-box; }
  body { margin: 0; }
  .shell { width: min(1056px, calc(100% - 40px)); margin-inline: auto; }
  .masthead { border-bottom: 1px solid var(--line); }
  .brand { padding-block: 16px; margin-block: 0; font-size: 17px; font-weight: 700; }
  .brand span, .rates.positive > div:first-child dd { color: var(--accent); }
  main { padding-block: 38px 48px; }
  .intro { display: flex; flex-wrap: wrap; align-items: end; justify-content: space-between; gap: 16px; }
  h1, h2, h3, p, dl, dd { margin: 0; }
  h1 { font-size: clamp(30px, 5vw, 38px); letter-spacing: -.035em; line-height: 1.15; }
  h2 { font-size: 17px; line-height: 1.35; }
  h3 { color: var(--muted); font-size: 12px; font-weight: 550; overflow-wrap: anywhere; }
  .period { color: var(--muted); margin-top: 9px; }
  .period span { display: inline-block; margin-inline-start: 8px; font-size: 12px; }
  nav {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
    padding: 3px;
    border: 1px solid var(--line);
    border-radius: 10px;
  }
  nav a {
    display: grid;
    place-items: center;
    min-height: 38px;
    padding-inline: 12px;
    border-radius: 7px;
    color: var(--muted);
    text-decoration: none;
    white-space: nowrap;
    font-weight: 600;
  }
  nav a[aria-current] { color: var(--accent); background: var(--raised); }
  @media (hover: hover) { nav a:hover { background: var(--raised); color: var(--text); } }
  nav a:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .coverage, .coverage + section, .message { margin-top: 30px; }
  dl { display: grid; gap: 12px 16px; }
  .coverage dl, .totals { grid-template-columns: repeat(auto-fit, minmax(min(100%, 148px), 1fr)); }
  .coverage dl { border-bottom: 1px solid var(--line); padding-block: 12px 18px; }
  dt, .description { color: var(--muted); font-size: 12px; }
  dd { font-variant-numeric: tabular-nums; font-weight: 650; overflow-wrap: anywhere; }
  .coverage dd { margin-top: 2px; font-size: 22px; }
  .description { margin-top: 5px; }
  .counters, .message { border: 1px solid var(--line); border-radius: 12px; background: var(--surface); }
  .counters { margin-top: 14px; }
  .counter { padding: 18px 20px; }
  .counter + .counter { border-top: 1px solid var(--line); }
  .rates { grid-template-columns: repeat(2, minmax(0, 220px)); margin-block: 12px 14px; }
  .rates dd { font-size: clamp(22px, 3.8vw, 36px); letter-spacing: -.03em; line-height: 1.2; }
  .totals { border-top: 1px solid var(--line); padding-top: 12px; }
  .totals dd { margin-top: 2px; }
  .message { padding: 20px; }
  @media (max-width: 460px) {
    main { padding-top: 26px; }
    .period span { display: block; margin-inline-start: 0; }
    .counter { padding: 15px; }
    .totals, .coverage dl { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
`;

export function renderAdminPanel(days, summary) {
  const nonce = btoa(String.fromCharCode(...crypto.getRandomValues(new Uint8Array(16))));
  const links = [1, 7, 30].map((value) => {
    const current = value === days ? ' aria-current="page"' : "";
    return `<a href="/admin?days=${value}"${current}>${value} ${value === 1 ? "day" : "days"}</a>`;
  }).join("");
  const html = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Usage reports · PennyPinch</title>
  <style nonce="${nonce}">${styles}</style>
</head>
<body>
  <header class="masthead"><p class="shell brand">Penny<span>Pinch</span></p></header>
  <main class="shell">
    <header class="intro">
      <div><h1>Usage reports</h1>${summary ? period(summary.period) : ""}</div>
      <nav aria-label="Report period">${links}</nav>
    </header>
    ${summary ? panelContent(summary) : '<p class="message">Usage reports are unavailable. Refresh the page to try again.</p>'}
  </main>
</body>
</html>`;
  return new Response(html, {
    status: summary ? 200 : 503,
    headers: {
      "content-type": "text/html; charset=utf-8",
      "cache-control": "no-store",
      "content-security-policy": `default-src 'none'; style-src 'nonce-${nonce}'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'`,
      "referrer-policy": "no-referrer",
      "x-content-type-options": "nosniff",
    },
  });
}
