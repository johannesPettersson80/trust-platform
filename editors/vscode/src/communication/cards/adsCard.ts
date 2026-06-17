import type { AdsPanelAction } from "../../adsPanel";
import type { CommunicationCardModel } from "../capability";
import { escapeHtml } from "../html";
import { type CardRenderOptions, renderCardShell } from "./shared";

export function renderAdsCard(
  card: CommunicationCardModel,
  focusedAdsAction: AdsPanelAction | undefined,
  options: CardRenderOptions
): string {
  const focus = focusedAdsAction ? ` Focus: ${escapeHtml(focusedAdsAction)}.` : "";
  const actions = `<button data-action="adsWorkflow" data-ads-action="addDevice">Connect to TwinCAT</button>
    <button data-action="adsWorkflow" data-ads-action="importSymbols">Import symbols</button>
    <button data-action="adsWorkflow" data-ads-action="serverStatus">Expose to TwinCAT</button>
    <button class="secondary" data-action="adsWorkflow" data-ads-action="diagnose">Diagnose</button>
    <button class="secondary" data-action="adsWorkflow" data-ads-action="addRoute">Routes</button>
    ${focus ? `<span class="muted">${focus}</span>` : ""}`;
  return renderCardShell(card, actions, options, focusedAdsAction ? " focused" : "");
}
