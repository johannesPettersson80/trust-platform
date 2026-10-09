// The words beside a control and in the notice when a command has a reason (specification 39,
// §21.2 item 3): a refused command reads "Not accepted: <reason>", the reason being the runtime's
// own, for a method command the device's `CommandText`. Other outcomes keep the runtime's words.

import { say } from "../drawing/speech";

/** What the runtime says when an instance refuses without a reason of its own. */
const NOT_ACCEPTED = "not accepted";

/** The words a command's outcome shows for its reason, in the display's language. */
export function commandReason(outcome: string, reason: string): string {
  const said = reason.trim();
  if (outcome !== "refused") return said;
  if (said === "" || said.toLowerCase() === NOT_ACCEPTED) return say("Not accepted");
  return say("Not accepted: {reason}", { reason: said });
}
