// The Discover pane of Devices & Connections: one search, drawn from the Rust view of the search status
// (design §4.5 "D4: Discover pane"; specification 25 §2). Rust gives every text and every rule: the origin
// choices, the `Only search` choices with the reason of each scan that cannot run, the labels and placeholders
// of the known address and subnet inputs, the source rows with their words and reasons, one card per device
// with its protocols, address, identity and actions, and the text of an empty result. This component draws
// them as given and posts a chosen action exactly as the view gives it. Its only state is what the user types
// into the two inputs. The origin and the ticks are the view's own, because the search that would use them is
// not in the view yet, so a change to them reports nothing here.

import React from "react";

import { BrowseNotice } from "./BrowsePane";
import { InspectorAction } from "./Inspector";
import type { Notice } from "./formTypes";
import { pillStyle } from "./styles";
import {
  addBodyStyle,
  addCloseStyle,
  addDialogStyle,
  addHeaderStyle,
  addTitleStyle,
} from "./styles/addDialog";
import { addRowStyle, browseEmptyStyle, loadingDotStyle } from "./styles/browse";
import {
  discoverAddressStyle,
  discoverCardActionsStyle,
  discoverCardStyle,
  discoverChoiceStyle,
  discoverFieldStyle,
  discoverIdentityStyle,
  discoverIdentityValueStyle,
  discoverInputStyle,
  discoverLabelStyle,
  discoverProtocolStyle,
  discoverReasonStyle,
  discoverRowStyle,
  discoverSectionStyle,
  discoverTitleStyle,
  discoverWarningStyle,
} from "./styles/discover";
import type {
  Action,
  CardProtocol,
  DeviceCard,
  DiscoverView,
  OnlyChoice,
  OriginChoice,
  SourceRow,
  TargetsInput,
} from "./types";

/**
 * The origin and the ticks are the view's: the next view says which origin and sources a search uses. A change
 * needs the Search action, which the view does not carry yet, so the control reports nothing.
 */
function viewDecides(): void {
  // Nothing to do: the control is drawn as the view gives it.
}

/** `Scan from`: this computer, then each runtime, with the chosen one as the view says. */
function OriginChoices({ origins }: { origins: OriginChoice[] }) {
  return (
    <section data-role="discover-origins" style={discoverSectionStyle}>
      {origins.map((origin) => (
        <label
          key={origin.id}
          data-role="discover-origin"
          data-origin={origin.id}
          title={origin.reason}
          style={discoverChoiceStyle(origin.enabled)}
        >
          <input
            type="radio"
            name="discover-origin"
            data-role="discover-origin-radio"
            checked={origin.selected}
            disabled={!origin.enabled}
            onChange={viewDecides}
          />
          <span>{origin.label}</span>
          {origin.reason !== undefined && (
            <span data-role="discover-origin-reason" style={discoverReasonStyle}>
              {origin.reason}
            </span>
          )}
        </label>
      ))}
    </section>
  );
}

/** `Only search`: one tick per source, disabled with its reason when a scan from the origin cannot run. */
function OnlyChoices({ choices }: { choices: OnlyChoice[] }) {
  return (
    <section data-role="discover-only" style={discoverSectionStyle}>
      {choices.map((choice) => (
        <label
          key={choice.source}
          data-role="discover-only-choice"
          data-source={choice.source}
          style={discoverChoiceStyle(choice.enabled)}
        >
          <input
            type="checkbox"
            data-role="discover-only-tick"
            checked={choice.checked}
            disabled={!choice.enabled}
            onChange={viewDecides}
          />
          <span data-role="discover-only-label">{choice.label}</span>
          {choice.reason !== undefined && (
            <span data-role="discover-only-reason" style={discoverReasonStyle}>
              {choice.reason}
            </span>
          )}
        </label>
      ))}
    </section>
  );
}

/** `Known address or subnet`: the two inputs, with the labels and placeholders the view gives. */
function Targets({ targets }: { targets: TargetsInput }) {
  return (
    <section data-role="discover-targets" style={discoverSectionStyle}>
      <label data-role="discover-target" style={discoverFieldStyle}>
        <span>{targets.hosts_label}</span>
        <input
          type="text"
          data-role="discover-hosts"
          placeholder={targets.hosts_placeholder}
          style={discoverInputStyle}
        />
      </label>
      <label data-role="discover-target" style={discoverFieldStyle}>
        <span>{targets.cidr_label}</span>
        <input
          type="text"
          data-role="discover-cidr"
          placeholder={targets.cidr_placeholder}
          style={discoverInputStyle}
        />
      </label>
    </section>
  );
}

/** The source rows: each source's word in its tone, its count, and its reason when it did not run. */
function SourceRows({ rows }: { rows: SourceRow[] }) {
  return (
    <section data-role="discover-sources" style={discoverSectionStyle}>
      {rows.map((row) => (
        <div
          key={row.id}
          data-role="discover-source"
          data-source={row.id}
          data-tone={row.tone}
          data-count={row.count}
          style={discoverRowStyle}
        >
          <span data-role="discover-source-label" style={discoverLabelStyle}>
            {row.label}
          </span>
          <span data-role="discover-source-word" style={pillStyle(row.tone)}>
            {row.word}
          </span>
          {row.reason !== undefined && (
            <span data-role="discover-source-reason" style={discoverReasonStyle}>
              {row.reason}
            </span>
          )}
        </div>
      ))}
    </section>
  );
}

/** A protocol line of a card: its name, how it was found and its confidence. */
function ProtocolLine({ protocol }: { protocol: CardProtocol }) {
  return (
    <div
      data-role="discover-card-protocol"
      data-simulated={protocol.simulated ? "true" : "false"}
      style={discoverProtocolStyle}
    >
      <span data-role="discover-card-protocol-name">{protocol.name}</span>
      <span data-role="discover-card-protocol-source" style={discoverReasonStyle}>
        {protocol.source}
      </span>
      <span data-role="discover-card-protocol-confidence" style={discoverReasonStyle}>
        {protocol.confidence}
      </span>
    </div>
  );
}

/**
 * One device card: its title, its addresses, every protocol found on it, its identity, when it was last seen,
 * its warnings, and its actions.
 */
function Card({ card, onAction }: { card: DeviceCard; onAction: (action: Action) => void }) {
  return (
    <div data-role="discover-card" data-card={card.id} style={discoverCardStyle}>
      <span data-role="discover-card-title" style={discoverTitleStyle}>
        {card.title}
      </span>
      {card.addresses.map((address) => (
        <span key={address} data-role="discover-card-address" style={discoverAddressStyle}>
          {address}
        </span>
      ))}
      {card.protocols.map((protocol, index) => (
        <ProtocolLine key={index} protocol={protocol} />
      ))}
      {card.identity.map((row) => (
        <div key={row.label} data-role="discover-card-identity" style={discoverIdentityStyle}>
          <span data-role="discover-card-identity-label" style={discoverReasonStyle}>
            {row.label}
          </span>
          <span data-role="discover-card-identity-value" style={discoverIdentityValueStyle}>
            {row.value}
          </span>
        </div>
      ))}
      <span data-role="discover-card-last-seen" style={discoverReasonStyle}>
        {card.last_seen}
      </span>
      {card.warnings.map((warning, index) => (
        <span key={index} data-role="discover-card-warning" style={discoverWarningStyle}>
          {warning}
        </span>
      ))}
      {card.stale_reason !== undefined && (
        <span data-role="discover-card-stale" style={discoverReasonStyle}>
          {card.stale_reason}
        </span>
      )}
      <div data-role="discover-card-actions" style={discoverCardActionsStyle}>
        {card.actions.map((action) => (
          <InspectorAction key={action.id} action={action} onAction={onAction} />
        ))}
      </div>
    </div>
  );
}

/**
 * The Discover pane: its title and close button, its origin, `Only search` and known address choices, its
 * source rows and cards, its empty text, and its footer with the pane's actions and the result of the last one.
 */
export function DiscoverPane({
  title,
  discover,
  notice,
  onAction,
  onClose,
}: {
  /** The pane's title: the words of the toolbar action that opened it. */
  title: string;
  /** The Discover view, as Rust gives it. */
  discover: DiscoverView;
  /** The result of the last action, when it names this pane. */
  notice?: Notice;
  /** Called with a chosen action, unchanged. */
  onAction: (action: Action) => void;
  /** Called when the close button is chosen. */
  onClose: () => void;
}) {
  return (
    <div
      data-role="discover-view"
      data-running={discover.running ? "true" : "false"}
      style={addDialogStyle}
    >
      <header style={addHeaderStyle}>
        <span data-role="discover-title" title={title} style={addTitleStyle}>
          {title}
        </span>
        {discover.running && <span data-role="discover-running" style={loadingDotStyle} />}
        <button type="button" data-role="discover-close" onClick={onClose} style={addCloseStyle}>
          ✕
        </button>
      </header>
      <div style={addBodyStyle}>
        {discover.progress_text !== undefined && (
          <p data-role="discover-progress" style={discoverReasonStyle}>
            {discover.progress_text}
          </p>
        )}
        <OriginChoices origins={discover.origins} />
        <OnlyChoices choices={discover.only} />
        <Targets targets={discover.targets} />
        <SourceRows rows={discover.sources} />
        {discover.cards.length > 0 && (
          <section data-role="discover-cards" style={discoverSectionStyle}>
            {discover.cards.map((card) => (
              <Card key={card.id} card={card} onAction={onAction} />
            ))}
          </section>
        )}
        {discover.empty_text !== undefined && (
          <p data-role="discover-empty" style={browseEmptyStyle}>
            {discover.empty_text}
          </p>
        )}
        <div data-role="discover-footer" style={addRowStyle}>
          {discover.actions.map((action) => (
            <InspectorAction key={action.id} action={action} onAction={onAction} />
          ))}
          {notice !== undefined && <BrowseNotice notice={notice} onAction={onAction} />}
        </div>
      </div>
    </div>
  );
}
