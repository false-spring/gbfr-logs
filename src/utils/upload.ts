import { invoke } from "@tauri-apps/api";

// Also the bug-report host, and the only remote origin the app's CSP permits.
export const SITE_BASE_URL = "https://relink.cleista.cc";

// Server-side batch cap; exceeding it is a 413 for the whole request.
const MAX_BATCH = 25;

const CLIENT_HEADER = "X-GBFR-Logs";

export type UploadOutcome = {
  stored: number;
  duplicates: number;
  rejected: number;
  url: string | null;
};

export class UploadError extends Error {
  constructor(
    message: string,
    // HTTP status, or 0 when the request never got a response.
    readonly status: number
  ) {
    super(message);
    this.name = "UploadError";
  }
}

const postBatch = async (logs: unknown[], appVersion: string): Promise<UploadOutcome> => {
  let response: Response;
  try {
    response = await fetch(`${SITE_BASE_URL}/api/upload`, {
      method: "POST",
      headers: { "Content-Type": "application/json", [CLIENT_HEADER]: appVersion },
      body: JSON.stringify({ logs }),
    });
  } catch (e) {
    throw new UploadError(String(e), 0);
  }

  if (!response.ok) throw new UploadError(`HTTP ${response.status}`, response.status);
  const body = await response.json();
  return {
    stored: body.stored ?? 0,
    duplicates: body.duplicates ?? 0,
    rejected: body.rejected ?? 0,
    url: body.url ? `${SITE_BASE_URL}${body.url}` : null,
  };
};

export const uploadLogs = async (ids: number[], appVersion: string): Promise<UploadOutcome> => {
  if (ids.length === 0) return { stored: 0, duplicates: 0, rejected: 0, url: null };

  const total: UploadOutcome = { stored: 0, duplicates: 0, rejected: 0, url: null };
  for (let at = 0; at < ids.length; at += MAX_BATCH) {
    const chunk = ids.slice(at, at + MAX_BATCH);
    const logs = await Promise.all(chunk.map((id) => invoke("bug_report_payload", { id })));
    const outcome = await postBatch(logs, appVersion);
    total.stored += outcome.stored;
    total.duplicates += outcome.duplicates;
    total.rejected += outcome.rejected;
    total.url ??= outcome.url;
  }
  return total;
};
