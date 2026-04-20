import { expect, test } from "@playwright/test";
import path from "node:path";
import { ensureParent, publicImagePath, repoRoot } from "../lib/paths.mjs";

test("capture browser ide with tutorial loaded", async ({ page }) => {
  const output = publicImagePath("browser/ide-tutorial-loaded.png");
  ensureParent(output);
  const projectPath = path.join(
    repoRoot,
    "examples/tutorials/12_hmi_pid_process_dashboard"
  );
  await page.addInitScript(() => {
    window.localStorage.setItem("trustTheme", "dark");
  });

  const response = await page.goto("http://127.0.0.1:18080/ide", {
    waitUntil: "commit"
  });

  expect(response?.ok()).toBeTruthy();
  await expect(page.locator("#ideTitle")).toHaveText(/truST IDE/i);
  await expect(page.locator("#buildBtn")).toBeVisible();
  const session = await page.evaluate(async () => {
    const response = await fetch("/api/ide/session", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({role: "viewer"}),
    });
    if (!response.ok) {
      throw new Error(`session bootstrap failed: ${response.status}`);
    }
    const payload = await response.json();
    if (!payload?.ok || !payload?.result?.token) {
      throw new Error(`session bootstrap failed: ${payload?.error || "missing token"}`);
    }
    return payload.result;
  });
  const selection = await page.evaluate(async ({ pathStr, sessionToken }) => {
    const response = await fetch("/api/ide/project/open", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Trust-Ide-Session": sessionToken,
      },
      body: JSON.stringify({path: pathStr}),
    });
    if (!response.ok) {
      throw new Error(`project open failed: ${response.status}`);
    }
    const payload = await response.json();
    if (!payload?.ok || !payload?.result) {
      throw new Error(`project open failed: ${payload?.error || "missing selection"}`);
    }
    return payload.result;
  }, {pathStr: projectPath, sessionToken: session.token});
  expect(selection?.active_project || selection?.startup_project).toContain(
    "12_hmi_pid_process_dashboard"
  );
  await page.reload({ waitUntil: "domcontentloaded" });
  await expect(page.locator("#buildBtn")).toBeVisible();
  const fileTree = page.locator("#fileTree");
  await expect(fileTree).toContainText("src");
  await page.locator("#fileTree .ide-tree-row", { hasText: "src" }).click();
  await expect(fileTree).toContainText("main.st");
  await page.locator("#fileTree .ide-tree-row", { hasText: "main.st" }).click();
  await expect(page.locator("#editorTitle")).toContainText("main.st");
  await expect(page.locator("#editorGrid")).toBeVisible();
  await expect(page.locator("#statusProject")).toContainText(
    "12_hmi_pid_process_dashboard"
  );
  await expect(page.locator("body")).toHaveAttribute("data-theme", "dark");
  await page.locator("#buildBtn").click();
  await expect(page.locator("#taskStatus")).toContainText(/build #/i, {
    timeout: 30_000,
  });
  await expect(page.locator("#taskStatus")).toContainText(/success/i, {
    timeout: 120_000,
  });
  await expect(page.locator("#taskOutput")).not.toContainText(
    "Build/Test/Validate output will appear here."
  );

  await page.screenshot({ path: output });
});
