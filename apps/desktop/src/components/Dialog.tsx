import type { ReactNode } from "react";

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
        <footer className="dialog-foot">
          <button type="button" className="primary" onClick={onConfirm ?? onClose}>
            {confirmLabel}
          </button>
          <button type="button" onClick={onClose}>
            {cancelLabel}
          </button>
        </footer>
      </div>
    </div>
  );
}
