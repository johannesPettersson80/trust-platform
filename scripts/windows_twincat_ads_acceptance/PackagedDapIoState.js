"use strict";

async function requestIoStateEvent(vscode, session, timeoutMs = 7_000) {
  let subscription;
  let timer;
  const event = new Promise((resolve) => {
    subscription = vscode.debug.onDidReceiveDebugSessionCustomEvent((candidate) => {
      if (candidate.session.id === session.id && candidate.event === "stIoState") {
        resolve(candidate.body);
      }
    });
  });
  const timeout = new Promise((_resolve, reject) => {
    timer = setTimeout(
      () => reject(new Error("Timed out waiting for stIoState.")),
      timeoutMs
    );
  });
  try {
    const [, eventBody] = await Promise.race([
      Promise.all([session.customRequest("stIoState"), event]),
      timeout,
    ]);
    return eventBody;
  } finally {
    if (timer) clearTimeout(timer);
    if (subscription) subscription.dispose();
  }
}

module.exports = { requestIoStateEvent };
