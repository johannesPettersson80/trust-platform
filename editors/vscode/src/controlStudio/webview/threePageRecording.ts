/** File selection is read-only; the scoped host owns fitting and continuation. */
export function bindRecordingOpen(root: HTMLElement, busy: () => boolean,
  start: (json: string) => void, notice: (message: string) => void): void {
  const button = root.querySelector<HTMLButtonElement>("[data-identify-import]")!;
  const input = root.querySelector<HTMLInputElement>("[data-identify-import-file]")!;
  let selection = 0;
  button.addEventListener("click", () => { if (!busy()) { input.value = ""; input.click(); } });
  input.addEventListener("change", async () => {
    const file = input.files?.[0], current = ++selection;
    if (!file || busy()) return;
    if (file.size > 32 * 1024 * 1024) { notice("Choose a recording of at most 32 MiB."); return; }
    try {
      notice("Reading recording…");
      const json = await file.text();
      if (current === selection && !busy()) { notice(""); start(json); }
    } catch { if (current === selection && !busy()) notice("The recording file could not be read. Open it again."); }
  });
}
