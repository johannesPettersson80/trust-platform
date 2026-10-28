// Where the keys go (specification 39, §21.3 item 6): the canvas's shortcuts act only while the
// canvas has the focus, or the page with no control focused. A button, a field, a menu or a dialog
// keeps its keys, Enter and the arrows included.

/** What keeps its own keys: controls, editable text, and anything in a menu or a dialog. */
const KEEPS_KEYS = [
  "button",
  "input",
  "textarea",
  "select",
  "a[href]",
  '[contenteditable]:not([contenteditable="false"])',
  '[role="menu"]',
  '[role="menubar"]',
  '[role="menuitem"]',
  '[role="menuitemcheckbox"]',
  '[role="menuitemradio"]',
  '[role="dialog"]',
  '[role="alertdialog"]',
  '[role="listbox"]',
  '[role="combobox"]',
  '[role="textbox"]',
  '[role="slider"]',
  '[role="spinbutton"]',
].join(", ");

/** The canvas: the page being edited and the room around it. */
const CANVAS = ".hb-canvas";

function asElement(target: EventTarget | null): Element | null {
  return target !== null && typeof (target as Element).closest === "function" ? (target as Element) : null;
}

/** Whether a key pressed on `target` belongs to the canvas: the target is in the canvas and no
 * control, or it is the page itself, with nothing focused. */
export function canvasOwnsKey(target: EventTarget | null): boolean {
  const element = asElement(target);
  if (!element) return true; // the window or the document: nothing has the focus
  const page = element.ownerDocument;
  if (element === page.body || element === page.documentElement) return true;
  return element.closest(KEEPS_KEYS) === null && element.closest(CANVAS) !== null;
}

/** A press on the canvas gives it the keys. The canvas's presses keep the browser from moving the
 * focus, for its drags, so a control that had the focus gives it up here, as on any other press. */
export function pressTakesKeys(event: MouseEvent): void {
  const target = asElement(event.target);
  if (!target || target.closest(CANVAS) === null || target.closest(KEEPS_KEYS) !== null) return;
  const focused = target.ownerDocument.activeElement as HTMLElement | null;
  if (focused && typeof focused.blur === "function" && !canvasOwnsKey(focused)) focused.blur();
}
