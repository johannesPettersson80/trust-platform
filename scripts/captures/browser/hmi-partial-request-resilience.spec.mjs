import net from "node:net";

import { expect, test } from "@playwright/test";

const HOST = "127.0.0.1";
const PORT = 18082;
const HMI_URL = `http://${HOST}:${PORT}/hmi`;

function openIncompletePairClaim() {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection({ host: HOST, port: PORT });
    socket.once("error", reject);
    socket.once("connect", () => {
      socket.write(
        `POST /api/pair/claim HTTP/1.1\r\n` +
          `Host: ${HOST}:${PORT}\r\n` +
          "Content-Type: application/json\r\n" +
          "Content-Length: 4096\r\n" +
          "Connection: keep-alive\r\n" +
          "\r\n" +
          '{"code":"',
        (error) => {
          if (error) {
            reject(error);
          } else {
            resolve(socket);
          }
        }
      );
    });
  });
}

test("HMI remains operational while an incomplete request body is held open", async ({ page }, testInfo) => {
  const slowSocket = await openIncompletePairClaim();
  try {
    await new Promise((resolve) => setTimeout(resolve, 100));
    const response = await page.goto(HMI_URL, {
      waitUntil: "domcontentloaded",
      timeout: 5_000
    });

    expect(response?.ok()).toBeTruthy();
    await expect(page.locator("#pageSidebar")).toContainText("Overview", {
      timeout: 5_000
    });
    await expect(page.locator("#connectionState")).toHaveText(/^connected$/i, {
      timeout: 5_000
    });
    await page.screenshot({
      path: testInfo.outputPath("hmi-partial-request-resilience.png")
    });
  } finally {
    slowSocket.destroy();
  }
});
