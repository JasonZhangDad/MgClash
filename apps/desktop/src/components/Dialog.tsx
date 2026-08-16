import type { ReactNode } from "react";

import { IconClose } from "./Icons";

interface DialogProps {
  ariaLabel: string;
  children: ReactNode;
  hidden?: boolean;
  onClose: () => void;
  onConfirm?: () => void;
  title: string;
  wide?: boolean;
  /// v2rayN windows put Confirm/Cancel on the bottom dock.
  confirmLabel?: string;
  cancelLabel?: string;
  /// Set when the body carries its own bottom bar, so the shell does not add a
  /// second row of Confirm/Cancel under it — as the node form did.
  ownFoot?: boolean;
}

/** Modal shell matching v2rayN Avalonia windows: title, body, confirm/cancel. */
export function Dialog({
  ariaLabel,
  children,
  hidden = false,
  onClose,
  onConfirm,
  title,
  wide = false,
  confirmLabel = "确定",
  cancelLabel = "取消",
  ownFoot = false,
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
          <button type="button" className="icon-btn" title="关闭" onClick={onClose}>
            <IconClose />
            <span className="sr-only">关闭</span>
          </button>
        </header>
        <div className="dialog-body">{children}</div>
        {ownFoot ? null : (
          <footer className="dialog-foot">
            <button
              type="button"
              className="primary"
              onClick={onConfirm ?? onClose}
            >
              {confirmLabel}
            </button>
            <button type="button" onClick={onClose}>
              {cancelLabel}
            </button>
          </footer>
        )}
      </div>
    </div>
  );
}
