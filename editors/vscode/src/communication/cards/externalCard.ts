import type { AdsPanelAction } from "../../adsPanel";
import type { CommunicationCardModel } from "../capability";
import { renderAdsCard } from "./adsCard";
import { type CardRenderOptions, renderSetupCard } from "./shared";

export function renderExternalCard(
  card: CommunicationCardModel,
  focusedAdsAction: AdsPanelAction | undefined,
  options: CardRenderOptions
): string {
  if (card.protocol.id === "ads") {
    return renderAdsCard(card, focusedAdsAction, options);
  }
  return renderSetupCard(card, options);
}
