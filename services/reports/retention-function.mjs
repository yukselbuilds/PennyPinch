import pg from "pg";
import { pruneExpiredUsageDays } from "./usage.mjs";

const cleanupRole = "pennypinch_reports_cleanup";
const cleanupPath = "/internal/expire-usage";

export function createCleanupPool(connectionString) {
  const url = URL.parse(connectionString ?? "");
  if (!url || !["postgres:", "postgresql:"].includes(url.protocol) ||
      url.username !== cleanupRole || !url.hostname || !url.password || url.pathname.length < 2) {
    return undefined;
  }
  // Ignore all URI options so they cannot disable certificate verification.
  url.search = "";
  url.hash = "";
  const pool = new pg.Pool({
    connectionString: url.href,
    ssl: { rejectUnauthorized: true },
    enableChannelBinding: true,
    max: 1,
    connectionTimeoutMillis: 5_000,
    query_timeout: 10_000,
  });
  pool.on("error", () => {});
  return pool;
}

function reply(status, extraHeaders) {
  return new Response(null, { status, headers: { "cache-control": "no-store", ...extraHeaders } });
}

export async function handleScheduledCleanup(request, { db, triggerId } = {}, now = Date.now()) {
  const url = new URL(request.url);
  const invocationId = request.headers.get("x-neon-trigger-invocation-id");
  // Neon strips this header from requests that did not originate from its trigger service.
  if (!triggerId || !invocationId || request.method !== "POST" || url.protocol !== "https:" ||
      url.pathname !== cleanupPath || url.search) {
    return reply(404);
  }
  const mediaType = request.headers.get("content-type")?.split(";", 1)[0].trim().toLowerCase();
  if (mediaType !== "application/json") return reply(400);
  let event;
  try {
    event = await request.json();
  } catch {
    return reply(400);
  }
  if (event?.version !== 1 || event.invocation_id !== invocationId || event.trigger?.id !== triggerId ||
      event.trigger.type !== "schedule" || typeof event.data?.scheduled_at !== "string" ||
      !Number.isFinite(Date.parse(event.data.scheduled_at))) {
    return reply(400);
  }
  // Use processing time so a delayed delivery applies the current retention cutoff.
  try {
    if (db && await pruneExpiredUsageDays(db, now)) return reply(204);
  } catch {
    // Keep database errors out of the public response.
  }
  return reply(503, { "retry-after": "60" });
}

const triggerId = process.env.REPORTS_CLEANUP_TRIGGER_ID;
const db = triggerId ? createCleanupPool(process.env.REPORTS_CLEANUP_DATABASE_URL) : undefined;

export default {
  fetch(request) {
    return handleScheduledCleanup(request, { db, triggerId });
  },
};
