"use strict";

const crypto = require("crypto");
const fs = require("fs");
const path = require("path");

function createSimulatorObservations(vscode) {
  return {
    activeTabLabel() {
      return vscode.window.tabGroups.activeTabGroup.activeTab?.label || "";
    },
    allTabLabels() {
      return vscode.window.tabGroups.all.flatMap((group) =>
        group.tabs.map((tab) => tab.label)
      );
    },
    hasVisibleAuthError(text) {
      return /No auth token provided|auth[_-]?token\s+(?:is\s+)?required|runtime\.control\.auth[_-]?token/i.test(
        String(text || "")
      );
    },
  };
}

function isStartingAction(action) {
  return action?.state === "busy" &&
    action.disabled === true &&
    action.text === "Starting…";
}

function isBlockedStartingAction(action) {
  return action?.clicked === false && isStartingAction(action);
}

function isSimulatorVisualState(simulator, health, label) {
  return simulator?.health === health && simulator.statusText === label;
}

async function createScreenshotProof({
  cdp,
  pageSession,
  screenshotDir,
  records,
}) {
  await cdp.send("Page.enable", {}, pageSession);
  return {
    async capture(name) {
      const response = await cdp.send(
        "Page.captureScreenshot",
        {
          format: "png",
          fromSurface: true,
          captureBeyondViewport: false,
        },
        pageSession
      );
      const bytes = Buffer.from(response.result?.data || "", "base64");
      const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
      if (
        bytes.length < 24 ||
        !crypto.timingSafeEqual(bytes.subarray(0, 8), signature)
      ) {
        throw new Error(`Packaged screenshot ${name} was not a valid PNG.`);
      }
      const fileName = `${name}.png`;
      fs.writeFileSync(path.join(screenshotDir, fileName), bytes);
      records.push({
        name,
        file: fileName,
        width: bytes.readUInt32BE(16),
        height: bytes.readUInt32BE(20),
        size_bytes: bytes.length,
        sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
      });
    },
    assertComplete(check, expected) {
      check("packaged-journey-screenshots-captured", records.length === expected, {
        expected,
        actual: records.length,
        files: records.map((item) => item.file),
      });
    },
  };
}

module.exports = {
  createScreenshotProof,
  createSimulatorObservations,
  isBlockedStartingAction,
  isSimulatorVisualState,
  isStartingAction,
};
