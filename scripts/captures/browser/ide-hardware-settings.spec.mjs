import { expect, test } from "@playwright/test";
import { ensureParent, publicImagePath } from "../lib/paths.mjs";

async function openIde(page, route = "/ide") {
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(String(error?.message || error)));
  const url = `http://127.0.0.1:18080${route}`;
  let response;
  let lastError;
  for (let attempt = 0; attempt < 6; attempt += 1) {
    try {
      response = await page.goto(url, {
        waitUntil: "domcontentloaded",
        timeout: 20_000,
      });
      break;
    } catch (error) {
      lastError = error;
      if (attempt === 5) {
        throw lastError;
      }
      await page.waitForTimeout(1_000);
    }
  }
  expect(response?.ok()).toBeTruthy();
  await expect(page.locator("#ideTabNav")).toHaveAttribute("role", "tablist");
  await expect(page.locator("#statusProject")).toContainText(
    "12_hmi_pid_process_dashboard",
  );
  return pageErrors;
}

async function expectSelectedTab(page, tab) {
  const selected = page.locator(`.ide-tab-btn[data-tab="${tab}"]`);
  await expect(selected).toHaveAttribute("aria-selected", "true");
  await expect(selected).toHaveAttribute("tabindex", "0");
  await expect(page.locator(`.ide-tab-btn[aria-selected="true"]`)).toHaveCount(1);
  await expect(page.locator(`#ideTabPanel_${tab}`)).toBeVisible();
  await expect(page.locator(`#ideTabPanel_${tab}`)).toHaveAttribute(
    "aria-labelledby",
    `ideTabBtn_${tab}`,
  );
  await expect(page).toHaveURL(new RegExp(`/ide/${tab}/?$`));
}

test("hardware and settings deep links preserve tab ARIA and routing state", async ({
  page,
}) => {
  const pageErrors = await openIde(page, "/ide/hardware");
  await expectSelectedTab(page, "hardware");

  await page.keyboard.press("Control+3");
  await expectSelectedTab(page, "settings");

  await page.reload({ waitUntil: "domcontentloaded" });
  await expectSelectedTab(page, "settings");

  await page.keyboard.press("Control+2");
  await expectSelectedTab(page, "hardware");
  expect(pageErrors).toEqual([]);
});

test("hardware renders configured drivers and routes an exact setting without saving", async ({
  page,
}) => {
  const pageErrors = await openIde(page, "/ide/hardware");
  await expectSelectedTab(page, "hardware");

  const paletteCategories = page.locator(
    "#hardwarePalette .hw-palette-category-header",
  );
  await expect(paletteCategories.first()).toBeVisible();
  expect(await paletteCategories.count()).toBeGreaterThanOrEqual(3);
  await expect(paletteCategories.first()).toHaveAttribute("aria-expanded", "true");
  await paletteCategories.first().click();
  await expect(paletteCategories.first()).toHaveAttribute("aria-expanded", "false");
  await paletteCategories.first().click();
  await expect(paletteCategories.first()).toHaveAttribute("aria-expanded", "true");

  const driversToggle = page.locator("#hwDriversPanelToggleBtn");
  await expect(driversToggle).toHaveAttribute("aria-expanded", "false");
  await driversToggle.click();
  await expect(driversToggle).toHaveAttribute("aria-expanded", "true");

  const loopbackCard = page.locator("#hwDriverCards .hw-driver-card", {
    hasText: "Loopback",
  });
  await expect(loopbackCard).toBeVisible();
  const configureInputs = loopbackCard.locator(
    '[data-hw-driver-settings="io.simulated.inputs"]',
  );
  await expect(configureInputs).toBeVisible();
  await expect(configureInputs).toHaveAttribute(
    "data-hw-driver-settings-category",
    "communication",
  );

  await configureInputs.click();
  await expectSelectedTab(page, "settings");
  await expect(
    page.locator('.settings-category-btn[data-category="communication"]'),
  ).toHaveClass(/active/);
  await expect(
    page.locator('[data-settings-key="io.simulated.inputs"]'),
  ).toBeVisible();
  await expect(
    page.locator('[data-settings-key="io.simulated.inputs"]'),
  ).toBeFocused();

  const output = publicImagePath("browser/ide-hardware-settings-deep-link.png");
  ensureParent(output);
  await page.screenshot({ path: output });
  expect(pageErrors).toEqual([]);
});

test("settings filter, security fields, and advanced state render as operator controls", async ({
  page,
}) => {
  const pageErrors = await openIde(page, "/ide/settings");
  await expectSelectedTab(page, "settings");

  const categories = page.locator("#settingsCategories .settings-category-btn");
  await expect(categories).toHaveCount(9);
  await expect(page.locator(".settings-category-search")).toBeVisible();

  await page.locator(".settings-category-search").fill("mqtt password");
  await expect(page.locator(".settings-filter-summary")).toContainText(
    "Filter active",
  );
  await expect(page.locator('[data-settings-key="io.mqtt.password"]')).toBeVisible();
  await expect(page.locator('[data-settings-key="io.mqtt.password"]')).toHaveAttribute(
    "type",
    "password",
  );
  await expect(page.locator('[data-settings-key="resource.name"]')).toHaveCount(0);

  await page.locator("[data-settings-clear-filter]").click();
  await expect(page.locator(".settings-filter-summary")).toHaveCount(0);
  await expect(page.locator('[data-settings-key="resource.name"]')).toBeVisible();

  await page.locator('.settings-category-btn[data-category="communication"]').click();
  await expect(page.locator('[data-settings-key="opcua.password"]')).toHaveAttribute(
    "type",
    "password",
  );
  await page.locator('.settings-category-btn[data-category="security"]').click();
  await expect(page.locator('[data-settings-key="control.auth_token"]')).toHaveAttribute(
    "type",
    "password",
  );

  await page.locator('.settings-category-btn[data-category="advanced"]').click();
  await expect(page.locator("#settingsFormPanel")).toContainText(
    "Runtime State (Read-only)",
  );
  for (const action of [
    "#settingsEditTomlBtn",
    "#settingsExportBtn",
    "#settingsImportBtn",
    "#settingsResetBtn",
  ]) {
    await expect(page.locator(action)).toBeVisible();
  }

  const output = publicImagePath("browser/ide-settings-advanced.png");
  ensureParent(output);
  await page.screenshot({ path: output });
  expect(pageErrors).toEqual([]);
});

test("invalid safe-state JSON is rejected before any settings write", async ({
  page,
}) => {
  const pageErrors = await openIde(page, "/ide/settings");
  await expectSelectedTab(page, "settings");
  await page.locator('.settings-category-btn[data-category="communication"]').click();

  const writes = [];
  await page.route("**/api/**", async (route) => {
    if (route.request().method() === "POST") {
      writes.push({
        url: route.request().url(),
        body: route.request().postData(),
      });
    }
    await route.continue();
  });

  const safeState = page.locator('[data-settings-key="io.safe_state_json"]');
  await safeState.fill('[{"address":"%QX0.0","value":');
  await safeState.blur();

  await expect(page.locator("#ideToast")).toContainText(
    "Failed to save I/O Safe State",
  );
  expect(writes).toEqual([]);
  expect(pageErrors).toEqual([]);
});

test("runtime setting revision conflict retries once and never reports a save", async ({
  page,
}) => {
  const conflictWrites = [];
  await page.route("**/api/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const isRuntimeWrite =
      request.method() === "POST" &&
      (url.pathname === "/api/config-ui/runtime/config" ||
        url.pathname === "/api/ide/file");
    if (!isRuntimeWrite) {
      await route.continue();
      return;
    }
    conflictWrites.push(JSON.parse(request.postData() || "{}"));
    await route.fulfill({
      status: 409,
      contentType: "application/json",
      body: JSON.stringify({ ok: false, error: "revision conflict" }),
    });
  });

  const pageErrors = await openIde(page, "/ide/settings");
  await expectSelectedTab(page, "settings");
  await page.evaluate(() => {
    window.__settingsRuntimeUpdateEvents = 0;
    document.addEventListener("ide-runtime-config-updated", () => {
      window.__settingsRuntimeUpdateEvents += 1;
    });
  });

  await page.locator('.settings-category-btn[data-category="general"]').click();
  const logLevel = page.locator('[data-settings-key="log.level"]');
  await logLevel.selectOption("debug");

  await expect(page.locator("#ideToast")).toContainText(
    "Failed to save Log Level",
  );
  expect(conflictWrites).toHaveLength(2);
  for (const payload of conflictWrites) {
    expect(
      Object.hasOwn(payload, "expected_revision") ||
        Object.hasOwn(payload, "expected_version"),
    ).toBeTruthy();
  }
  expect(
    await page.evaluate(() => window.__settingsRuntimeUpdateEvents),
  ).toBe(0);
  expect(pageErrors).toEqual([]);
});
