const rtf = new Intl.RelativeTimeFormat("en", { numeric: "auto" });

export function relTime(iso: string | null, now = Date.now()): string {
  if (!iso) return "never";
  const diff = Date.parse(iso) - now;
  const abs = Math.abs(diff);
  const min = 60_000;
  const hour = 60 * min;
  const day = 24 * hour;
  if (abs < hour) return rtf.format(Math.round(diff / min), "minute");
  if (abs < day) return rtf.format(Math.round(diff / hour), "hour");
  if (abs < 30 * day) return rtf.format(Math.round(diff / day), "day");
  if (abs < 365 * day) return rtf.format(Math.round(diff / (30 * day)), "month");
  return rtf.format(Math.round(diff / (365 * day)), "year");
}

export function shortDate(iso: string | null): string {
  if (!iso) return "unknown";
  return new Date(iso).toLocaleDateString("en-CA");
}

export function homeRelative(path: string): string {
  return path.replace(/^\/Users\/[^/]+/, "~");
}
