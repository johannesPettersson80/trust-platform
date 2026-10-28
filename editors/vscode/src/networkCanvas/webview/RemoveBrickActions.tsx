import React, { useEffect, useRef, useState } from "react";

export function RemoveBrickActions({
  identity,
  description,
  onRemove,
  children,
  busy = false,
}: {
  identity: string;
  description: string;
  onRemove: () => void;
  children: React.ReactNode;
  busy?: boolean;
}) {
  const [confirming, setConfirming] = useState(false);
  const removeButton = useRef<HTMLButtonElement>(null);

  useEffect(() => setConfirming(false), [identity]);

  const cancel = () => {
    setConfirming(false);
    requestAnimationFrame(() => removeButton.current?.focus());
  };

  if (confirming) {
    return (
      <>
        <div
          className="trust-message trust-message--error endpoint-remove-confirmation"
          style={{ flexBasis: "100%", margin: 0 }}
        >
          {description}
        </div>
        <button onClick={cancel} className="trust-button" style={{ flex: 1 }}>
          Cancel
        </button>
        <button disabled={busy} onClick={onRemove} className="trust-button trust-button--danger" style={{ flex: 1 }}>
          {busy ? "Removing…" : "Confirm remove"}
        </button>
      </>
    );
  }

  return (
    <>
      {children}
      <button
        ref={removeButton}
        onClick={() => setConfirming(true)}
        className="trust-button trust-button--danger"
      >
        Remove
      </button>
    </>
  );
}
