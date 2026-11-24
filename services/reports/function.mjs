import pg from "pg";
import { handleReportRequest } from "./service.mjs";

const runtimeRole = "pennypinch_reports_runtime";

export function createReportPool(connectionString) {
  const url = URL.parse(connectionString ?? "");
  if (!url || !["postgres:", "postgresql:"].includes(url.protocol) ||
      url.username !== runtimeRole || !url.hostname || !url.password || url.pathname.length < 2) {
    return undefined;
  }

  let password;
  let database;
  try {
    password = decodeURIComponent(url.password);
    database = decodeURIComponent(url.pathname.slice(1));
  } catch {
    return undefined;
  }

  // Explicit options keep URI query parameters from changing the role or TLS.
  const pool = new pg.Pool({
    host: url.hostname,
    port: url.port ? Number(url.port) : 5432,
    user: runtimeRole,
    password,
    database,
    ssl: { rejectUnauthorized: true },
    enableChannelBinding: true,
    max: 2,
    connectionTimeoutMillis: 5_000,
    query_timeout: 10_000,
  });
  // The pool removes a failed idle client before it emits this error.
  pool.on("error", () => {});
  return pool;
}

const db = createReportPool(process.env.DATABASE_URL);

export default {
  fetch(request) {
    return handleReportRequest(request, { db });
  },
};
