import * as assert from "assert";
import { Webview } from "./hmiBuilderWebview";
import type { HmiPageModel } from "../../hmiBuilder/drawing/model";

const wait = (): Promise<void> => new Promise((done) => setTimeout(done, 100));
const PAGE: HmiPageModel = {
  title: "Cell",
  pipe: [],
  object: [
    {
      id: "start",
      type: "button",
      at: [40, 40],
      size: [140, 44],
      rotation: 0,
      text: "Start",
      confirm: "Start the pump?",
      bind: { target: { path: "Main.Run", direction: "write" } },
    },
    {
      id: "photo",
      type: "image",
      at: [300, 40],
      size: [100, 100],
      rotation: 0,
      image: "hmi/assets/cell.png",
      bind: {},
    },
  ],
};

suite("HMI Builder editor: named Live operation", function () {
  this.timeout(15000);
  test("Live uses the runtime login, confirmation and command path, and ends when Design returns", async () => {
    const view = new Webview();
    await view.loaded(PAGE);
    view.send({ type: "pages", pages: [{ name: "overview", title: "Cell", hidden: false }], current: "overview" });
    view.query('[data-action="live"]').click();
    await wait();
    const open = view.posted.find((message) => message.type === "operateOpen");
    assert.ok(open, "Live requests a named operating connection for this page");
    const id = open.id as string;
    view.send({ type: "live", state: "connected", target: "Cell runtime" });
    view.send({ type: "operateReady", id, target: "Cell runtime" });
    const handled = new Set<number>();
    let user: string | undefined;
    const answer = async (): Promise<void> => {
      for (let round = 0; round < 5; round++) {
        await wait();
        for (const message of view.posted.filter((entry) => entry.type === "operateRequest")) {
          const request = message.request as number;
          if (handled.has(request)) continue;
          handled.add(request);
          const path = message.path as string;
          if (path === "/api/hmi/login") user = "anna";
          const body = path.startsWith("/api/hmi/page")
            ? {
                screen: { size: [960, 640], top_bar: { title: "Cell", user: true, logo: "hmi/assets/logo.svg" } },
                page: {
                  name: new URL(path, "http://live.invalid").searchParams.get("name") ?? "overview",
                  model: PAGE,
                },
                pages: ["overview", "detail"],
                navigation: [
                  { name: "overview", title: "Cell" },
                  { name: "detail", title: "Detail" },
                ],
                bundle: "v1",
                roles: { start: "operator" },
                errors: [],
              }
            : path === "/api/hmi/command"
              ? { outcome: "applied" }
              : user
                ? { ok: true, user, role: "operator" }
                : { ok: true, session: null };
          view.send({ type: "operateReply", id, request, status: 200, body });
        }
      }
    };
    await answer();
    const root = view.query('[data-part="live-operating"]');
    assert.ok(root, "the runtime display replaces the design canvas");
    assert.ok(!view.query(".hb-zoom"), "design zoom is not offered for the runtime layout");
    assert.doesNotMatch(
      view.query('[data-part="live-status"]').textContent,
      /operate in the runtime display/i,
      "the status describes in-editor operation",
    );
    assert.strictEqual(
      root.querySelector('[data-object="photo"] image').getAttribute("href"),
      "https://assets.test/hmi/assets/cell.png",
      "Live draws project images from the editor asset root",
    );
    assert.strictEqual(
      root.querySelector(".hmi-brand img").src,
      "https://assets.test/hmi/assets/logo.svg",
      "the runtime logo uses the same project assets",
    );
    root.querySelector('[data-object="start"]').dispatchEvent(new view.window.MouseEvent("click", { bubbles: true }));
    await wait();
    const form = root.querySelector('[data-part="login"]');
    assert.ok(form, "operating asks for a named login");
    form.querySelector('[name="user"]').value = "anna";
    form.querySelector('[name="password"]').value = "test-only";
    form.dispatchEvent(new view.window.Event("submit", { bubbles: true, cancelable: true }));
    await answer();
    assert.match(root.textContent, /anna/);
    root.querySelector('[data-object="start"]').dispatchEvent(new view.window.MouseEvent("click", { bubbles: true }));
    await wait();
    assert.match(root.querySelector('[data-part="confirm"]').textContent, /Start the pump/);
    assert.ok(
      !view.posted.some((message) => message.path === "/api/hmi/command"),
      "nothing is sent before confirmation",
    );
    root.querySelector('[data-action="confirm"]').click();
    await answer();
    const commands = view.posted.filter((message) => message.path === "/api/hmi/command");
    assert.strictEqual(commands.length, 1);
    assert.strictEqual(root.querySelector('[data-object="start"]').getAttribute("data-outcome"), "applied");
    assert.strictEqual((commands[0].body as { bundle: string }).bundle, "v1");
    root.querySelector('[data-action="theme"]').click();
    assert.ok(root.querySelector(".hmi-theme-dark"));
    root.querySelector('a[href="/hmi/detail"]').click();
    await answer();
    assert.strictEqual(
      view.posted.filter((message) => message.type === "operateOpen").length,
      1,
      "navigation keeps the same named connection",
    );
    assert.ok(
      view.posted.some((message) => typeof message.path === "string" && message.path.includes("name=detail")),
      "navigation loads the selected runtime page inside Live mode",
    );
    assert.match(view.query('[data-part="live-operating"]').textContent, /anna/);
    assert.ok(
      view.query('[data-part="live-operating"] .hmi-theme-dark'),
      "the chosen theme lasts across page navigation",
    );
    const streams = view.posted.filter((message) => message.type === "operateSocket");
    view.send({
      type: "operateStream",
      id,
      socket: streams[streams.length - 1].socket as number,
      data: JSON.stringify({ bundle: "v2" }),
    });
    await wait();
    const reads = () =>
      view.posted.filter((message) => message.type === "operateRequest" && String(message.path).includes("name=detail"))
        .length;
    const beforeReload = reads();
    view.query('[data-part="live-operating"] [data-action="reload"]').click();
    await answer();
    assert.strictEqual(reads(), beforeReload + 1, "Reload loads the current address again");
    view.query('[data-action="design"]').click();
    await wait();
    assert.ok(view.posted.some((message) => message.type === "operateClose" && message.id === id));
    assert.strictEqual(view.query('[data-part="live-operating"]'), null);
  });
});
