import type { HmiPageModel } from "../../drawing/model";
import type { ScreenModel } from "../messages";

/** Every path the page's objects bind, once each, in the page's order (§11, item 6). */
export function boundPaths(page: HmiPageModel): string[] {
  const paths: string[] = [];
  for (const object of page.object) {
    for (const binding of Object.values(object.bind)) {
      for (const path of [binding.path, binding.quality]) if (path && !paths.includes(path)) paths.push(path);
    }
  }
  return paths;
}

/** The page area a screen of `size` leaves the page: less its top bar and its navigation (§4
 * item 7). */
export function pageArea(size: [number, number], screen: ScreenModel): [number, number] {
  const side = screen.navigation === "side" ? 232 : 0;
  const above = (screen.topBar ? 56 : 0) + (screen.navigation === "tabs" ? 48 : 0);
  return [size[0] - side, size[1] - above];
}
