import type { CommunicationCardModel } from "../capability";
import { type CardRenderOptions, renderSetupCard } from "./shared";

export function renderRuntimeMeshCard(
  card: CommunicationCardModel,
  options: CardRenderOptions
): string {
  return renderSetupCard(card, options);
}
