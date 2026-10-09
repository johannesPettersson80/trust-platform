// The name an object goes by in the Objects list and in the quality warnings (specification 39,
// §3.3 item 8, §14.1 item 4), shared by the editor's host and its webview.

/** The name an object goes by: its tag, its instance, its equipment's block in words
 * (`HeatingSequence` is "Heating sequence"), its label or text, else its id. */
export function objectName(object: {
  id: string;
  tag?: string;
  instance?: string;
  block?: string;
  label?: string;
  text?: string;
}): string {
  const block = object.block?.split(".").pop()?.replace(/^FB_/, "");
  return (
    object.tag ||
    object.instance?.split(".").pop() ||
    (block ? wordsOf(block) : undefined) ||
    object.label ||
    object.text ||
    object.id
  );
}

/** An identifier in words: `HeatingSequence` is "Heating sequence", `PowerOutput` "Power output". */
export function wordsOf(name: string): string {
  return name
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/ ([A-Z])(?=[a-z])/g, (_, letter: string) => ` ${letter.toLowerCase()}`);
}

/** The name a new trend pen takes from its variable, as its legend shows it: the instance's tag
 * for its value (`Main.TT101.Value` is "TT101"), the tag and the member in words for another member
 * (`Main.TIC101.Setpoint` is "TIC101 setpoint"), and a program's own variable in words
 * (`Simulation.WindowOpening` is "Window opening"). */
export function penName(path: string): string {
  const parts = path
    .split(".")
    .map((part) => part.replace(/\[.*$/, "").trim())
    .filter(Boolean);
  const inside = parts.length > 1 ? parts.slice(1) : parts;
  if (inside.length === 0) return "Pen";
  const member = inside[inside.length - 1];
  if (inside.length === 1) return wordsOf(member);
  const owner = inside[inside.length - 2];
  return /^value$/i.test(member) ? owner : `${owner} ${wordsOf(member).toLowerCase()}`;
}
