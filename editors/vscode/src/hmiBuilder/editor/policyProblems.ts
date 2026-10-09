// The write-policy problem as the editor reads it (specification 39, §12.2.3), shared by the host
// and the webview, which offers the fix beside the problem.

/** The write-policy check's message for a point no rule covers (`check_page_policy`). */
const NOT_WRITABLE = /^"([^"]+)" is not writable by the runtime's write policy$/;

/** The point a problem names when no write rule covers it. */
export function notWritable(message: string): string | undefined {
  return NOT_WRITABLE.exec(message)?.[1];
}
