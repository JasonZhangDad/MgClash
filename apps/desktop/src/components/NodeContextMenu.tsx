import type { NodeSummary } from "../session";

interface NodeContextMenuProps {
  target: NodeSummary;
  position: { x: number; y: number };
  /// The rows the action applies to: the checked selection when the click
  /// landed inside it, otherwise just this row.
  batch: string[];
  busy: boolean;
  connected: boolean;
  selected: boolean;
  reorderable: boolean;
  index: number;
  nodeCount: number;
  hasEnabledInBatch: boolean;
  runningNodeId: string | null;
  t: (text: string) => string;
  onActivate: () => void;
  onToggleEnabled: () => void;
  onTest: () => void;
  onSpeedTest: () => void;
  onUrlTest: () => void;
  onClone: () => void;
  onExportLinks: () => void;
  onShowQrCode: () => void;
  onGroup: () => void;
  onSetFront: () => void;
  onEdit: () => void;
  onMove: (direction: "up" | "down") => void;
  onSortByLatency: () => void;
  onDelete: () => void;
  onRemoveDuplicates: () => void;
}

/**
 * The per-node actions, grouped the way a desktop client groups them.
 *
 * Thirteen items in one column is a wall to read; the groups are the same
 * actions with the reading order a user expects — what the connection does,
 * what edits the node, what leaves the app, what reorders the list, and what
 * destroys something.
 */
export function NodeContextMenu({
  target,
  position,
  batch,
  busy,
  connected,
  selected,
  reorderable,
  index,
  nodeCount,
  hasEnabledInBatch,
  runningNodeId,
  t,
  onActivate,
  onToggleEnabled,
  onTest,
  onSpeedTest,
  onUrlTest,
  onClone,
  onExportLinks,
  onShowQrCode,
  onGroup,
  onSetFront,
  onEdit,
  onMove,
  onSortByLatency,
  onDelete,
  onRemoveDuplicates,
}: NodeContextMenuProps) {
  const suffix = batch.length > 1 ? ` (${batch.length})` : "";
  const running = connected && selected;
  return (
    <ul
      className="context-menu"
      role="menu"
      aria-label={`节点操作 ${target.name}`}
      style={{ left: position.x, top: position.y }}
    >
      <li className="menu-group-label" role="presentation">
        {t("连接")}
      </li>
      <li>
        <button
          type="button"
          role="menuitem"
          disabled={busy || selected || !target.enabled}
          onClick={onActivate}
        >
          {t("设为活动")}
        </button>
      </li>
      <li>
        <button
          type="button"
          role="menuitem"
          disabled={busy || !hasEnabledInBatch}
          onClick={onTest}
        >
          {`${t("测试延迟")}${suffix}`}
        </button>
      </li>
      <li>
        <button
          type="button"
          role="menuitem"
          disabled={busy || !connected || !selected}
          onClick={onUrlTest}
        >
          {t("URL 测试")}
        </button>
      </li>
      <li>
        <button
          type="button"
          role="menuitem"
          disabled={busy || !connected || !selected}
          onClick={onSpeedTest}
        >
          {t("下载测速")}
        </button>
      </li>

      <li className="menu-group-label" role="presentation">
        {t("编辑")}
      </li>
      <li>
        <button
          type="button"
          role="menuitem"
          disabled={busy || running}
          onClick={onToggleEnabled}
        >
          {target.enabled ? t("禁用节点") : t("启用节点")}
        </button>
      </li>
      <li>
        <button type="button" role="menuitem" disabled={busy} onClick={onGroup}>
          {t("设置分组")}
        </button>
      </li>
      <li>
        <button
          type="button"
          role="menuitem"
          disabled={busy || running}
          onClick={onSetFront}
        >
          {t("前置代理")}
        </button>
      </li>
      {target.deletable && (
        <li>
          <button
            type="button"
            role="menuitem"
            disabled={busy || running}
            onClick={onEdit}
          >
            {t("编辑")}
          </button>
        </li>
      )}
      <li>
        <button
          type="button"
          role="menuitem"
          disabled={busy || !target.deletable}
          onClick={onClone}
        >
          {t("克隆所选")}
        </button>
      </li>

      <li className="menu-group-label" role="presentation">
        {t("分享")}
      </li>
      <li>
        <button
          type="button"
          role="menuitem"
          disabled={busy}
          onClick={onExportLinks}
        >
          {`${t("导出分享链接")}${suffix}`}
        </button>
      </li>
      <li>
        <button
          type="button"
          role="menuitem"
          disabled={busy}
          onClick={onShowQrCode}
        >
          {t("显示二维码")}
        </button>
      </li>

      <li className="menu-group-label" role="presentation">
        {t("排序")}
      </li>
      <li>
        <button
          type="button"
          role="menuitem"
          disabled={busy || !reorderable || index === 0}
          onClick={() => onMove("up")}
        >
          {t("上移")}
        </button>
      </li>
      <li>
        <button
          type="button"
          role="menuitem"
          disabled={busy || !reorderable || index === nodeCount - 1}
          onClick={() => onMove("down")}
        >
          {t("下移")}
        </button>
      </li>
      <li>
        <button
          type="button"
          role="menuitem"
          disabled={busy || !reorderable || nodeCount < 2}
          onClick={onSortByLatency}
        >
          {t("按延迟排序")}
        </button>
      </li>

      <li className="menu-group-label" role="presentation">
        {t("移除")}
      </li>
      <li>
        <button
          type="button"
          role="menuitem"
          className="danger"
          disabled={
            busy ||
            !target.deletable ||
            (connected && batch.some((id) => id === runningNodeId))
          }
          onClick={onDelete}
        >
          {target.deletable ? `${t("移除所选")}${suffix}` : t("订阅节点不可移除")}
        </button>
      </li>
      <li>
        <button
          type="button"
          role="menuitem"
          className="danger"
          disabled={busy || connected}
          onClick={onRemoveDuplicates}
        >
          {t("移除重复")}
        </button>
      </li>
    </ul>
  );
}
