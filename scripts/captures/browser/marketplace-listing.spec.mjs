import { expect, test } from "@playwright/test";
import { ensureParent, publicImagePath } from "../lib/paths.mjs";

test("capture truST marketplace listing", async ({ page }) => {
  const output = publicImagePath("install/marketplace-listing.png");
  ensureParent(output);
  await page.emulateMedia({ colorScheme: "dark" });

  const response = await page.goto(
    "https://marketplace.visualstudio.com/items?itemName=trust-platform.trust-lsp",
    {
      waitUntil: "domcontentloaded"
    }
  );

  expect(response?.ok()).toBeTruthy();
  await expect(page.locator("body")).toContainText("truST LSP", {
    timeout: 30_000
  });
  await expect(page.locator("body")).toContainText("trust-platform", {
    timeout: 30_000
  });
  await expect(page.locator("body")).toContainText("Install", {
    timeout: 30_000
  });
  const rejectCookies = page.getByRole("button", { name: "Reject", exact: true });
  if (await rejectCookies.isVisible().catch(() => false)) {
    await rejectCookies.click();
  }

  await page.screenshot({ path: output });
});
