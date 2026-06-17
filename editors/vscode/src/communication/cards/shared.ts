import {
  statusLabel,
  type CommunicationCardModel,
} from "../capability";
import {
  type CommApplyResponse,
  type CommSchemaResponse,
} from "../schemaForm";
import { escapeAttribute, escapeHtml } from "../html";

export interface CardRenderOptions {
  schema?: CommSchemaResponse;
  activeProtocolId?: string;
  applyResult?: CommApplyResponse;
}

export function renderSetupCard(
  card: CommunicationCardModel,
  options: CardRenderOptions
): string {
  const setupButton =
    card.protocol.id === "enterprise"
      ? ""
      : `<button data-action="setupProtocol" data-protocol="${escapeAttribute(card.protocol.id)}">Set up...</button>`;
  const testButton = card.protocol.supportsTest
    ? `<button class="secondary" data-action="setupProtocol" data-protocol="${escapeAttribute(card.protocol.id)}">Test connection</button>`
    : "";
  return renderCardShell(card, `${setupButton}${testButton}`, options);
}

export function renderCardShell(
  card: CommunicationCardModel,
  protocolActions: string,
  options: CardRenderOptions,
  extraClass = ""
): string {
  const activeProtocolSchema = options.schema?.protocols.find(
    (protocol) => protocol.id === card.protocol.id
  );
  const activeClass =
    options.activeProtocolId === card.protocol.id && activeProtocolSchema
      ? " active-protocol"
      : "";
  return `<article class="card${activeClass}${extraClass}" data-protocol="${escapeAttribute(card.protocol.id)}" data-status="${escapeAttribute(card.status)}">
    <h3>${escapeHtml(card.protocol.title)} <span class="pill ${escapeAttribute(card.status)}">${escapeHtml(statusLabel(card.status))}</span></h3>
    <p>${escapeHtml(card.protocol.purpose)}</p>
    <p class="state-detail muted">${escapeHtml(card.detail)}</p>
    ${renderNextStep(card)}
    <h4>You need</h4>
    <ul class="requirements">${card.protocol.requirements.map((item) => `<li>${escapeHtml(item)}</li>`).join("")}</ul>
    <div class="actions">
      ${protocolActions}
      <button class="secondary" data-action="openDocs" data-docs-path="${escapeAttribute(card.protocol.docsPath)}">Docs</button>
    </div>
  </article>`;
}

function renderNextStep(card: CommunicationCardModel): string {
  if (!shouldRenderNextStep(card)) {
    return "";
  }
  return `<p class="next-step muted">${escapeHtml(card.nextStep)}</p>`;
}

function shouldRenderNextStep(card: CommunicationCardModel): boolean {
  const step = card.nextStep.trim();
  if (step.length === 0) {
    return false;
  }
  if (card.capability?.next_action?.kind === "none") {
    return false;
  }
  return !(card.status === "connected" && step.toLowerCase() === "status");
}
