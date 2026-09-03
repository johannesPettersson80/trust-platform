// Renders a saved-result ISO timestamp as local date-time without milliseconds, keeping the exact ISO value for a tooltip.

export interface FormattedTimestamp {
  display: string;
  title: string;
}

/** Local "YYYY-MM-DD HH:MM:SS" reading of an ISO timestamp; falls back to the raw text when it cannot be parsed. */
export function formatLocalTimestamp(iso: string): FormattedTimestamp {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return { display: iso, title: iso };
  const pad = (value: number) => String(value).padStart(2, "0");
  const display = `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ` +
    `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
  return { display, title: iso };
}
