// The equipment inspector's conversation with the editor's host (specification 39, §21.3 item 2):
// the details of the inspected instance, the outcome of a change, and the requests that ask for
// them.

import { useCallback, useEffect, useMemo, useState } from "react";

import type { HostToWebview } from "../messages";
import type { DependencyPick } from "./dependencyPick";
import type { EquipmentEditing } from "./EquipmentConfiguration";
import type { VsCodeApi } from "./vscodeApi";

export function useEquipmentEditing(api: VsCodeApi, pick: (request: DependencyPick) => void): EquipmentEditing {
  const [state, setState] = useState<EquipmentEditing["state"]>();
  const [result, setResult] = useState<EquipmentEditing["result"]>();
  useEffect(() => {
    const onMessage = (event: MessageEvent<HostToWebview>): void => {
      const message = event.data;
      if (message?.type === "equipmentDetails") {
        setState({ instance: message.instance, details: message.details, error: message.error });
      } else if (message?.type === "equipmentConfigured") {
        setResult((previous) => ({
          instance: message.instance,
          error: message.error,
          sequence: (previous?.sequence ?? 0) + 1,
        }));
      }
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, []);
  // The requests stay the same functions while the answers change, so an effect that asks once
  // per inspected instance can depend on them.
  const ask = useCallback(
    (instance: string, block?: string) => api.postMessage({ type: "equipmentInspect", instance, block }),
    [api],
  );
  const hardware = useCallback((instance: string) => api.postMessage({ type: "equipmentHardware", instance }), [api]);
  const apply = useCallback(
    (instance: string, changes: Parameters<EquipmentEditing["apply"]>[1], revision: number) =>
      api.postMessage({ type: "equipmentConfigure", instance, changes, revision }),
    [api],
  );
  const picture = useCallback(
    (instance: string, block: string, symbol: string, bind?: Record<string, string>) =>
      api.postMessage({ type: "equipmentPicture", instance, block, symbol, ...(bind ? { bind } : {}) }),
    [api],
  );
  const importPicture = useCallback(() => api.postMessage({ type: "importPicture" }), [api]);
  const rename = useCallback(
    (instance: string, name: string) => api.postMessage({ type: "equipmentRename", instance, name }),
    [api],
  );
  return useMemo(
    () => ({ pick, state, result, ask, hardware, apply, picture, importPicture, rename }),
    [pick, state, result, ask, hardware, apply, picture, importPicture, rename],
  );
}
