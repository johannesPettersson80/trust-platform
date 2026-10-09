// A page's or a panel's own texts in the display's language (specification 39, §14.5 item 4): its
// title and each object's label, text and question; tags are names and stay as written.

import type { HmiPageModel } from "./model";
import { translate } from "./speech";

/** The page or panel as written, its own texts in the frame's language. */
export function translatedPage(page: HmiPageModel): HmiPageModel {
  const object = page.object.map((shown) => ({
    ...shown,
    ...(shown.label !== undefined ? { label: translate(shown.label) } : {}),
    ...(shown.text !== undefined ? { text: translate(shown.text) } : {}),
    ...(shown.confirm !== undefined ? { confirm: translate(shown.confirm) } : {}),
  }));
  return { ...page, title: translate(page.title), object };
}
