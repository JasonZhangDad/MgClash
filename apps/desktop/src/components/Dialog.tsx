import type { ReactNode } from "react";

interface DialogProps {
  ariaLabel: string;
  children: ReactNode;
  hidden?: boolean;
  onClose: () => void;
  title: string;
  wide?: boolean;
}

/** Modal shell used for v2rayN-style option / subscription / routing windows. */
export function Dialog({
  ariaLabel,
  children,
  hidden = false,
  onClose,
  title,
  wide = false,
}: DialogProps) {
  return (
    <div className="dialog-backdrop" hidden={hidden} onClick={onClose}>
      <div
        className={wide ? "dialog dialog-wide" : "dialog"}
        role="dialog"
        aria-label={ariaLabel}
        hidden={hidden}
        onClick={(event) => event.stopPropagation()}
      >
        <header className="dialog-head">
          <strong>{title}</strong>
          <button type="button" onClick={onClose}>
            关闭
          </button>
        </header>
        <div className="dialog-body">{children}</div>
      </div>
    </div>
  );
}
