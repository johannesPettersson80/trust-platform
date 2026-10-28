import { useEffect, useRef, useState } from "react";

// The ADS port typed into a browse pane, as the server reads it (`trust-lsp.devicesCanvasAct`
// `ads_port`): the port it names, or why it names none. The pane asks for every text the user types
// and shows the answer of the latest one; it decides nothing about the text itself.

export interface AdsPortCheck {
  /** The port the text names, once the server has answered for this text. */
  readonly port?: number;
  /** Why the text names no port, once the server has answered for this text. */
  readonly error?: string;
  /** Whether the server has not answered for this text yet. */
  readonly pending: boolean;
}

let lastRequestId = 0;

export function useAdsPortCheck(
  post: ((message: unknown) => void) | undefined,
  text: string
): AdsPortCheck {
  const [answer, setAnswer] = useState<{ text: string; port?: number; error?: string }>();
  const requestRef = useRef(0);

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      const message = event.data as Record<string, unknown> | null | undefined;
      if (
        !message ||
        message.type !== "adsPortChecked" ||
        message.requestId !== requestRef.current ||
        typeof message.text !== "string"
      ) {
        return;
      }
      setAnswer({
        text: message.text,
        port: typeof message.port === "number" ? message.port : undefined,
        error: typeof message.error === "string" ? message.error : undefined,
      });
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, []);

  useEffect(() => {
    lastRequestId += 1;
    requestRef.current = lastRequestId;
    post?.({ type: "checkAdsPort", requestId: lastRequestId, text });
  }, [post, text]);

  const current = answer?.text === text ? answer : undefined;
  return { port: current?.port, error: current?.error, pending: current === undefined };
}
