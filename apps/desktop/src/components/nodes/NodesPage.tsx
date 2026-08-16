import { useState } from "react";

import { formatBytes, formatClock, formatRate, latencyQuality } from "../../appHelpers";
import type { AppModel } from "../../hooks/useAppController";
import type { NodeGroupStrategy, NodeSummary } from "../../session";
import { loadSessionStatus, setNodeEnabled } from "../../session";
import { NodeContextMenu } from "../NodeContextMenu";
import { SortableHeader } from "../SortableHeader";
import {
  IconBolt,
  IconEdit,
  IconExpand,
  IconGrid,
  IconList,
  IconMore,
  IconPlus,
} from "../Icons";
import { Badge, EmptyState, LatencyBadge, StatusDot } from "../ui/Ui";
import type { NodeSortColumn } from "../../appHelpers";

function nodeGroupStrategyBadge(
  strategy: NodeGroupStrategy,
  t: (text: string) => string,
): string {
  switch (strategy) {
    case "urlTest":
      return t("自动");
    case "fallback":
      return t("故障转移");
    case "loadBalance":
      return t("负载均衡");
    default:
      return "";
  }
}

function nodeLatency(
  candidate: NodeSummary,
  nodeTests: AppModel["nodeTests"],
  t: (text: string) => string,
): { label: string; quality: "good" | "ok" | "bad" | "fail" | null } {
  const testResult = nodeTests[candidate.id];
  let label =
    candidate.latencyMs === null
      ? candidate.lastTestedAt === null
        ? "—"
        : t("失败")
      : `${candidate.latencyMs} ms`;
  if (testResult?.status === "testing") {
    label = t("测试中");
  } else if (testResult?.status === "success") {
    label = `${testResult.latencyMs} ms`;
  } else if (testResult?.status === "timeout") {
    label = t("超时");
  } else if (testResult?.status === "failed") {
    label = t("失败");
  }
  const liveMs =
    testResult?.status === "success" ? testResult.latencyMs : candidate.latencyMs;
  const quality =
    testResult?.status === "timeout" ||
    testResult?.status === "failed" ||
    (testResult === undefined &&
      candidate.latencyMs === null &&
      candidate.lastTestedAt !== null)
      ? "fail"
      : liveMs === null
        ? null
        : latencyQuality(liveMs);
  return { label, quality };
}

export function NodesPage({ app }: { app: AppModel }) {
  const {
    t,
    busy,
    checkedNodes,
    columnWidths,
    connected,
    groupingNodeId,
    frontChoice,
    frontingNodeId,
    node,
    nodeGroupFilter,
    nodeGroupName,
    nodeGroupNames,
    nodeGroupStrategy,
    nodeGroups,
    nodeMenu,
    nodeQuery,
    nodeSort,
    nodeTestInProgress,
    nodeTests,
    nodeTraffic,
    nodeView,
    nodes,
    nodesTab,
    onActivateNode,
    onCloneNode,
    onDeleteNode,
    onDeleteNodes,
    onEditNode,
    onExportNodeLink,
    onExportNodeLinks,
    onGroupNode,
    onMoveNode,
    onRemoveDuplicateNodes,
    onResizeColumn,
    onSaveNodeFront,
    onSaveNodeGroup,
    onShowNodeQrCode,
    onSortColumn,
    onSortNodesByLatency,
    onSpeedTest,
    onTestAllNodes,
    onTestNode,
    onTestNodes,
    onTestUrl,
    onCancelNodeTests,
    openCreateForm,
    openNodeMenu,
    resetNodeGroupForm,
    resetSubscriptionForm,
    run,
    setCheckedNodes,
    setColumnWidths,
    setDialog,
    setFrontChoice,
    setFrontingNodeId,
    setInspectedId,
    setNodeGroupFilter,
    setNodeGroupName,
    setNodeGroupStrategy,
    setNodeMenu,
    setNodeQuery,
    setNodeView,
    setNodes,
    setNodesTab,
    setStatus,
    subscriptions,
    testingAllNodes,
    toggleCheckedNode,
    visibleNodes,
    refreshingSubscriptions,
    setRefreshingSubscriptions,
    onRefreshSubscription,
    onToggleSubscriptionEnabled,
    onImportClipboard,
    onEditSubscription,
    onDeleteSubscription,
  } = app;
  const [overflowOpen, setOverflowOpen] = useState(false);
  const selectedIds = visibleNodes
    .map((item) => item.id)
    .filter((id) => checkedNodes.has(id));

  const rowDot = (candidate: NodeSummary) => {
    if (candidate.id === node?.id) {
      return "ok" as const;
    }
    if (nodeTests[candidate.id]?.status === "testing") {
      return "warn" as const;
    }
    if (!candidate.enabled) {
      return "line" as const;
    }
    return "muted" as const;
  };

  return (
    <div className="page-nodes">
      <div className="page-tabs">
        <button
          type="button"
          className={nodesTab === "all" ? "is-on" : undefined}
          onClick={() => setNodesTab("all")}
        >
          {t("全部节点")} <span className="tab-count">{nodes.length}</span>
        </button>
        <button
          type="button"
          className={nodesTab === "groups" ? "is-on" : undefined}
          onClick={() => setNodesTab("groups")}
        >
          {t("分组")} <span className="tab-count">{nodeGroups.length}</span>
        </button>
        <button
          type="button"
          className={nodesTab === "subs" ? "is-on" : undefined}
          onClick={() => setNodesTab("subs")}
        >
          {t("订阅")} <span className="tab-count">{subscriptions.length}</span>
        </button>
      </div>
      <section className="profiles-pane" hidden={nodesTab !== "all"}>
        <nav className="group-chips" aria-label={t("节点分组筛选")}>
          <button
            type="button"
            className={nodeGroupFilter === "all" ? "active" : undefined}
            onClick={() => setNodeGroupFilter("all")}
          >
            {t("全部")}
          </button>
          <button
            type="button"
            className={nodeGroupFilter === "ungrouped" ? "active" : undefined}
            onClick={() => setNodeGroupFilter("ungrouped")}
          >
            {t("未分组")}
          </button>
          {nodeGroups.map((group) => (
            <button
              key={group.id}
              type="button"
              className={nodeGroupFilter === group.id ? "active" : undefined}
              onClick={() => setNodeGroupFilter(group.id)}
            >
              {(() => {
                const badge = nodeGroupStrategyBadge(group.strategy, t);
                return badge === "" ? group.name : `${group.name} · ${badge}`;
              })()}
            </button>
          ))}
        </nav>
        <div className="nodes-toolbar">
          <input
            className="toolbar-search"
            aria-label={t("搜索节点名称 / 服务器 / 协议")}
            placeholder={t("搜索节点名称 / 服务器 / 协议")}
            value={nodeQuery}
            onChange={(event) => setNodeQuery(event.target.value)}
          />
          <label className="toolbar-select">
            {t("分组")}
            <select
              aria-label={t("节点分组筛选")}
              value={nodeGroupFilter}
              onChange={(event) =>
                setNodeGroupFilter(
                  event.target.value as typeof nodeGroupFilter,
                )
              }
            >
              <option value="all">{t("全部")}</option>
              <option value="ungrouped">{t("未分组")}</option>
              {nodeGroups.map((group) => (
                <option key={group.id} value={group.id}>
                  {group.name}
                </option>
              ))}
            </select>
          </label>
          <label className="toolbar-select">
            {t("排序")}
            <select
              aria-label={t("排序")}
              value={nodeSort?.column ?? "manual"}
              onChange={(event) => {
                const column = event.target.value;
                if (column === "manual") {
                  app.setNodeSort(null);
                  return;
                }
                onSortColumn(column as NodeSortColumn);
              }}
            >
              <option value="manual">{t("排序")}</option>
              <option value="latency">{t("延迟")}</option>
              <option value="name">{t("别名")}</option>
              <option value="protocol">{t("协议")}</option>
              <option value="server">{t("地址")}</option>
            </select>
          </label>
          <div className="segmented view-toggle" role="group">
            <button
              type="button"
              className={nodeView === "list" ? "is-on" : undefined}
              aria-label={t("列表视图")}
              onClick={() => setNodeView("list")}
            >
              <IconList />
            </button>
            <button
              type="button"
              className={nodeView === "cards" ? "is-on" : undefined}
              aria-label={t("卡片视图")}
              onClick={() => setNodeView("cards")}
            >
              <IconGrid />
            </button>
          </div>
          <div className="nodes-toolbar-end">
            {testingAllNodes ? (
              <button type="button" onClick={onCancelNodeTests}>
                {t("取消测速")}
              </button>
            ) : (
              <button
                type="button"
                className="btn-secondary"
                title={t("一键多线程测试延迟和速度")}
                disabled={
                  busy ||
                  nodeTestInProgress ||
                  !nodes.some((candidate) => candidate.enabled)
                }
                onClick={() => void onTestAllNodes()}
              >
                <IconBolt />
                {t("全部测速")}
              </button>
            )}
            <button
              type="button"
              className="btn-primary"
              disabled={busy}
              onClick={() => openCreateForm()}
            >
              <IconPlus />
              {t("添加节点")}
            </button>
            <div className="overflow-wrap">
              <button
                type="button"
                className="icon-btn"
                aria-label={t("操作")}
                onClick={() => setOverflowOpen((open) => !open)}
              >
                <IconMore />
              </button>
              {overflowOpen ? (
                <div className="popover">
                  <button
                    type="button"
                    onClick={() => {
                      setColumnWidths({});
                      setOverflowOpen(false);
                    }}
                  >
                    <IconExpand /> {t("自适应列宽")}
                  </button>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => {
                      setDialog("subscriptions");
                      setOverflowOpen(false);
                    }}
                  >
                    <IconEdit /> {t("订阅分组设置")}
                  </button>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => {
                      resetSubscriptionForm();
                      setDialog("subscriptions");
                      setOverflowOpen(false);
                    }}
                  >
                    <IconPlus /> {t("添加")}
                  </button>
                </div>
              ) : null}
            </div>
          </div>
        </div>
        <div className="profiles-toolbar" hidden>
          <input
            className="toolbar-search"
            aria-label={t("搜索节点名称 / 服务器 / 协议")}
            value={nodeQuery}
            onChange={(event) => setNodeQuery(event.target.value)}
          />
        </div>

        {refreshingSubscriptions ? (
          <div className="sub-refresh-banner" aria-label={t("正在更新订阅")}>
            <i className="pulse-dot" />
            {t("正在更新订阅")}
            <button
              type="button"
              onClick={() => setRefreshingSubscriptions(false)}
            >
              {t("取消")}
            </button>
          </div>
        ) : null}

        <div className="node-workspace">
          <div className="node-panel">
            {refreshingSubscriptions ? (
              <div className="node-skeleton" aria-hidden>
                {Array.from({ length: 6 }, (_, index) => (
                  <div key={index} className="skeleton-row" />
                ))}
              </div>
            ) : null}
            {nodes.length === 0 ? (
              <EmptyState
                label={t("还没有代理节点")}
                title={t("还没有代理节点")}
                body={t("尚未导入节点")}
                actions={
                  <>
                    <button
                      type="button"
                      className="btn-primary"
                      onClick={() => setDialog("import")}
                    >
                      {t("导入订阅")}
                    </button>
                    <button
                      type="button"
                      className="btn-secondary"
                      onClick={() => openCreateForm()}
                    >
                      {t("添加节点")}
                    </button>
                    <button
                      type="button"
                      className="btn-secondary"
                      onClick={() => void onImportClipboard()}
                    >
                      {t("从剪贴板导入分享链接")}
                    </button>
                  </>
                }
              />
            ) : visibleNodes.length === 0 ? (
              <EmptyState
                title={t("当前分组没有节点")}
                body={t("当前分组没有节点")}
              />
            ) : nodeView === "cards" ? (
              <div className="node-card-grid" aria-label={t("节点列表")}>
                {visibleNodes.map((candidate) => {
                  const { label, quality } = nodeLatency(candidate, nodeTests, t);
                  return (
                    <button
                      key={candidate.id}
                      type="button"
                      className="node-card"
                      onClick={() => setInspectedId(candidate.id)}
                      onDoubleClick={() => {
                        if (!busy && candidate.enabled) {
                          void onActivateNode(candidate.id);
                        }
                      }}
                    >
                      <StatusDot tone={rowDot(candidate)} />
                      <strong>{candidate.name}</strong>
                      <Badge>{candidate.protocol}</Badge>
                      <span className="mono muted">
                        {candidate.server}:{candidate.port}
                      </span>
                      <LatencyBadge quality={quality} label={label} />
                    </button>
                  );
                })}
              </div>
            ) : (
              <table className="node-list node-grid" aria-label={t("节点列表")}>
                <thead>
                  <tr>
                    <th className="node-check">
                      <input
                        type="checkbox"
                        aria-label={t("全选节点")}
                        checked={
                          visibleNodes.length > 0 &&
                          visibleNodes.every((item) =>
                            checkedNodes.has(item.id),
                          )
                        }
                        onChange={(event) =>
                          setCheckedNodes(
                            event.target.checked
                              ? new Set(visibleNodes.map((item) => item.id))
                              : new Set(),
                          )
                        }
                      />
                    </th>
                    <th aria-hidden />
                    {(
                      [
                        ["别名", "name"],
                        ["协议", "protocol"],
                        ["地址", "server"],
                        ["分组", undefined],
                        ["延迟", "latency"],
                      ] as [string, NodeSortColumn | undefined][]
                    ).map(([label, column]) => (
                      <SortableHeader
                        key={label}
                        column={column}
                        label={label}
                        sort={nodeSort}
                        width={columnWidths[label]}
                        t={t}
                        onSort={onSortColumn}
                        onResize={onResizeColumn}
                      />
                    ))}
                    <th>{t("速度")}</th>
                    <th>{t("今日流量")}</th>
                    <th>{t("累计流量")}</th>
                    <th className="node-actions-col" />
                  </tr>
                </thead>
                <tbody>
                  {visibleNodes.map((candidate) => {
                    const selected = candidate.id === node?.id;
                    const { label, quality } = nodeLatency(
                      candidate,
                      nodeTests,
                      t,
                    );
                    const inspectedRow = app.inspectedId === candidate.id;
                    const today =
                      (nodeTraffic[candidate.id]?.todayUploadBytes ?? 0) +
                      (nodeTraffic[candidate.id]?.todayDownloadBytes ?? 0);
                    const total =
                      (nodeTraffic[candidate.id]?.totalUploadBytes ?? 0) +
                      (nodeTraffic[candidate.id]?.totalDownloadBytes ?? 0);
                    return (
                      <tr
                        key={candidate.id}
                        className={
                          [
                            selected ? "active-node" : "",
                            inspectedRow ? "inspected-node" : "",
                            candidate.enabled ? "" : "disabled-node",
                          ]
                            .filter(Boolean)
                            .join(" ") || undefined
                        }
                        onClick={() => setInspectedId(candidate.id)}
                        onContextMenu={(event) =>
                          openNodeMenu(event, candidate.id)
                        }
                        onDoubleClick={() => {
                          if (!busy && !nodeTestInProgress && candidate.enabled) {
                            void onActivateNode(candidate.id);
                          }
                        }}
                      >
                        <td className="node-check">
                          <input
                            type="checkbox"
                            aria-label={`选择 ${candidate.name}`}
                            checked={checkedNodes.has(candidate.id)}
                            onClick={(event) => event.stopPropagation()}
                            onChange={() => toggleCheckedNode(candidate.id)}
                          />
                        </td>
                        <td>
                          <StatusDot tone={rowDot(candidate)} />
                        </td>
                        <td>
                          <span
                            className={
                              candidate.enabled
                                ? "node-name"
                                : "node-name node-disabled"
                            }
                          >
                            {selected ? (
                              <em className="tag-active">{t("活动")}</em>
                            ) : null}
                            {candidate.name}
                            {!candidate.enabled ? (
                              <em className="pill">{t("已禁用")}</em>
                            ) : null}
                          </span>
                        </td>
                        <td>
                          <Badge>{candidate.protocol}</Badge>
                        </td>
                        <td className="mono muted">
                          {candidate.server}:{candidate.port}
                        </td>
                        <td>
                          {candidate.groupId === null
                            ? t("未分组")
                            : (nodeGroupNames.get(candidate.groupId) ??
                              t("未知分组"))}
                        </td>
                        <td>
                          <LatencyBadge
                            quality={quality}
                            label={label}
                          />
                        </td>
                        <td>
                          {(() => {
                            const speed = app.nodeSpeeds[candidate.id];
                            if (speed?.status === "testing") {
                              return t("测试中");
                            }
                            if (
                              speed?.status === "success" &&
                              speed.bytesPerSecond !== null &&
                              speed.bytesPerSecond !== undefined
                            ) {
                              return formatRate(speed.bytesPerSecond);
                            }
                            if (speed?.status === "timeout") {
                              return t("超时");
                            }
                            if (speed?.status === "failed") {
                              return t("失败");
                            }
                            return "—";
                          })()}
                        </td>
                        <td className="mono muted">{formatBytes(today)}</td>
                        <td className="mono muted">{formatBytes(total)}</td>
                        <td className="node-actions">
                          <button
                            type="button"
                            aria-label={`操作 ${candidate.name}`}
                            aria-haspopup="menu"
                            disabled={busy || nodeTestInProgress}
                            onClick={(event) => {
                              event.stopPropagation();
                              openNodeMenu(event, candidate.id);
                            }}
                          >
                            <IconMore />
                          </button>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}

            {nodeMenu !== null &&
              (() => {
                const target = nodes.find((item) => item.id === nodeMenu.nodeId);
                if (target === undefined) {
                  return null;
                }
                const reorderable =
                  nodeGroupFilter === "all" && nodeSort === null;
                const act = (run_: () => void) => () => {
                  setNodeMenu(null);
                  run_();
                };
                const visible = new Set(visibleNodes.map((item) => item.id));
                const batch = checkedNodes.has(target.id)
                  ? [...checkedNodes].filter((id) => visible.has(id))
                  : [target.id];
                return (
                  <NodeContextMenu
                    target={target}
                    position={{ x: nodeMenu.x, y: nodeMenu.y }}
                    batch={batch}
                    busy={busy}
                    connected={connected}
                    selected={target.id === node?.id}
                    reorderable={reorderable}
                    index={nodes.findIndex((item) => item.id === target.id)}
                    nodeCount={nodes.length}
                    hasEnabledInBatch={batch.some((id) =>
                      nodes.some(
                        (candidate) =>
                          candidate.id === id && candidate.enabled,
                      ),
                    )}
                    runningNodeId={node?.id ?? null}
                    t={t}
                    onActivate={act(() => void onActivateNode(target.id))}
                    onToggleEnabled={act(() =>
                      void run(async () => {
                        setNodes(
                          await setNodeEnabled(target.id, !target.enabled),
                        );
                        setStatus(await loadSessionStatus());
                      }),
                    )}
                    onTest={act(() =>
                      batch.length > 1
                        ? void onTestNodes(batch)
                        : void onTestNode(target.id),
                    )}
                    onUrlTest={act(() => void onTestUrl())}
                    onSpeedTest={act(() => void onSpeedTest())}
                    onClone={act(() => void onCloneNode(target.id))}
                    onExportLinks={act(() =>
                      batch.length > 1
                        ? void onExportNodeLinks(batch)
                        : void onExportNodeLink(target.id),
                    )}
                    onShowQrCode={act(() => void onShowNodeQrCode(target))}
                    onGroup={act(() => onGroupNode(target, batch))}
                    onSetFront={act(() => {
                      setFrontingNodeId(target.id);
                      setFrontChoice(target.frontNodeId ?? "");
                    })}
                    onEdit={act(() => void onEditNode(target))}
                    onMove={(direction) =>
                      act(() => void onMoveNode(target.id, direction))()
                    }
                    onSortByLatency={act(() => void onSortNodesByLatency())}
                    onDelete={act(() =>
                      batch.length > 1
                        ? void onDeleteNodes(batch)
                        : void onDeleteNode(target.id),
                    )}
                    onRemoveDuplicates={act(() =>
                      void onRemoveDuplicateNodes(),
                    )}
                  />
                );
              })()}

            {frontingNodeId !== null && (
              <div className="settings-form" aria-label={t("设置前置代理")}>
                <label>
                  {t("前置代理节点")}
                  <select
                    aria-label={t("前置代理节点")}
                    disabled={busy}
                    value={frontChoice}
                    onChange={(event) => setFrontChoice(event.target.value)}
                  >
                    <option value="">{t("不使用")}</option>
                    {nodes
                      .filter(
                        (candidate) =>
                          candidate.id !== frontingNodeId &&
                          candidate.frontNodeId === null,
                      )
                      .map((candidate) => (
                        <option key={candidate.id} value={candidate.id}>
                          {candidate.name}
                        </option>
                      ))}
                  </select>
                </label>
                <p className="hint">
                  {t("先连到前置节点，再从它连到当前节点。只支持一跳。")}
                </p>
                <div className="actions">
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => void onSaveNodeFront()}
                  >
                    {t("保存前置代理")}
                  </button>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => {
                      setFrontingNodeId(null);
                      setFrontChoice("");
                    }}
                  >
                    {t("取消")}
                  </button>
                </div>
              </div>
            )}

            {groupingNodeId !== null && (
              <div className="settings-form" aria-label={t("设置节点分组")}>
                <label>
                  {t("分组名称")}
                  <input
                    aria-label={t("节点分组")}
                    disabled={busy}
                    list="node-group-options"
                    value={nodeGroupName}
                    onChange={(event) => setNodeGroupName(event.target.value)}
                  />
                  <datalist id="node-group-options">
                    {nodeGroups.map((group) => (
                      <option key={group.id} value={group.name} />
                    ))}
                  </datalist>
                </label>
                <label>
                  {t("分组策略")}
                  <select
                    aria-label={t("分组策略")}
                    value={nodeGroupStrategy}
                    disabled={busy}
                    onChange={(event) =>
                      setNodeGroupStrategy(
                        event.target.value as NodeGroupStrategy,
                      )
                    }
                  >
                    <option value="select">{t("手动选择")}</option>
                    <option value="urlTest">{t("URL-TEST 自动测速")}</option>
                    <option value="fallback">{t("故障转移")}</option>
                    <option value="loadBalance">{t("负载均衡")}</option>
                  </select>
                </label>
                <p className="hint">
                  {nodeGroupStrategy === "urlTest"
                    ? t("该分组连接时由 Core 测速选节点")
                    : nodeGroupStrategy === "fallback"
                      ? t("按列表顺序尝试节点，失败则切换下一个")
                      : nodeGroupStrategy === "loadBalance"
                        ? t("连接时在组成员之间分配流量")
                        : t("连接时使用你在列表中选中的节点")}
                </p>
                <div className="actions">
                  <button
                    type="button"
                    disabled={busy}
                    onClick={() => void onSaveNodeGroup()}
                  >
                    {t("保存分组")}
                  </button>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={resetNodeGroupForm}
                  >
                    {t("取消")}
                  </button>
                </div>
              </div>
            )}
          </div>
        </div>
        {selectedIds.length > 0 ? (
          <div className="selection-bar" aria-label={t("已选")}>
            <span>
              {t("已选")} {selectedIds.length}
            </span>
            <button
              type="button"
              disabled={busy || nodeTestInProgress}
              onClick={() => void onTestNodes(selectedIds)}
            >
              {t("测试延迟")}
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => {
                const first = visibleNodes.find((item) =>
                  selectedIds.includes(item.id),
                );
                if (first) {
                  onGroupNode(first, selectedIds);
                }
              }}
            >
              {t("移动到分组")}
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => void onExportNodeLinks(selectedIds)}
            >
              {t("导出分享链接")}
            </button>
            <button
              type="button"
              className="btn-danger-ghost"
              disabled={busy}
              onClick={() => void onDeleteNodes(selectedIds)}
            >
              {t("删除")}
            </button>
            <span className="muted">
              {t("双击行连接 · 右键更多操作")}
            </span>
          </div>
        ) : null}
      </section>

      {nodesTab === "groups" ? (
        <div className="group-cards">
          {nodeGroups.length === 0 ? (
            <EmptyState
              title={t("尚未创建代理组")}
              body={t("尚未创建代理组")}
            />
          ) : (
            nodeGroups.map((group) => {
              const members = nodes.filter((item) => item.groupId === group.id);
              const active = members.find((item) => item.id === node?.id);
              const badge = nodeGroupStrategyBadge(group.strategy, t);
              return (
                <button
                  key={group.id}
                  type="button"
                  className="group-card"
                  onClick={() => {
                    setNodeGroupFilter(group.id);
                    setNodesTab("all");
                  }}
                >
                  <div className="group-card-head">
                    <strong>{group.name}</strong>
                    {badge ? (
                      <em className="ui-badge tone-neutral">{badge}</em>
                    ) : null}
                    <span className="mono">{members.length}</span>
                  </div>
                  <p>
                    {t("活动")} {active?.name ?? "—"}
                    {active?.latencyMs != null
                      ? ` · ${active.latencyMs} ms`
                      : ""}
                  </p>
                </button>
              );
            })
          )}
        </div>
      ) : null}

      {nodesTab === "subs" ? (
        <div className="sub-cards">
          {refreshingSubscriptions ? (
            <div className="sub-refresh-banner" aria-label={t("正在更新订阅")}>
              <i className="pulse-dot" />
              {t("正在更新订阅")}
            </div>
          ) : null}
          {subscriptions.length === 0 ? (
            <EmptyState
              title={t("尚未添加订阅")}
              body={t("尚未添加订阅")}
              actions={
                <button
                  type="button"
                  className="btn-primary"
                  onClick={() => {
                    resetSubscriptionForm();
                    setDialog("subscriptions");
                  }}
                >
                  {t("添加订阅")}
                </button>
              }
            />
          ) : (
            <div aria-label={t("订阅列表")}>
              {subscriptions.map((item) => (
                <article
                  key={item.id}
                  className="sub-card"
                  aria-label={`订阅 ${item.name}`}
                >
                  <div className="sub-card-head">
                    <strong>{item.name}</strong>
                    <em
                      className={`ui-badge ${
                        item.lastError
                          ? "tone-dang"
                          : item.enabled
                            ? "tone-ok"
                            : "tone-neutral"
                      }`}
                    >
                      {item.lastError
                        ? t("失败")
                        : item.enabled
                          ? t("已是最新")
                          : t("已禁用")}
                    </em>
                    <div className="sub-actions">
                      <button
                        type="button"
                        aria-label={`刷新 ${item.name}`}
                        disabled={busy || nodeTestInProgress || !item.enabled}
                        onClick={() => void onRefreshSubscription(item.id)}
                      >
                        {t("更新")}
                      </button>
                      <button
                        type="button"
                        aria-label={`${t("更新当前订阅 (通过代理)")} ${item.name}`}
                        disabled={busy || nodeTestInProgress || !item.enabled}
                        onClick={() => void onRefreshSubscription(item.id)}
                      >
                        {t("更新当前订阅 (通过代理)")}
                      </button>
                      <button
                        type="button"
                        aria-label={`编辑 ${item.name}`}
                        disabled={busy || connected || nodeTestInProgress}
                        onClick={() => {
                          onEditSubscription(item);
                          setDialog("subscriptions");
                        }}
                      >
                        {t("编辑")}
                      </button>
                      <button
                        type="button"
                        aria-label={`删除订阅 ${item.name}`}
                        disabled={busy || connected || nodeTestInProgress}
                        onClick={() => void onDeleteSubscription(item.id)}
                      >
                        {t("删除")}
                      </button>
                    </div>
                  </div>
                  <p className="mono muted" aria-label={t("订阅地址")}>
                    {item.url === "" ? "—" : item.url}
                  </p>
                  <div className="sub-meta">
                    <span>
                      {t("节点")} {item.nodeCount}
                    </span>
                    <span>
                      {t("上次更新")}{" "}
                      {item.lastUpdatedAt === null
                        ? t("从未")
                        : formatClock(item.lastUpdatedAt)}
                    </span>
                    <span>
                      {t("自动更新")}{" "}
                      {item.autoUpdate ? t("启用") : t("关闭")}
                    </span>
                    <label className="sb-inline">
                      {t("启用")}
                      <input
                        aria-label={`启用 ${item.name}`}
                        className="toggle"
                        type="checkbox"
                        checked={item.enabled}
                        disabled={busy}
                        onChange={(event) =>
                          void onToggleSubscriptionEnabled(
                            item,
                            event.target.checked,
                          )
                        }
                      />
                    </label>
                  </div>
                  {item.lastError ? (
                    <div className="sub-error">
                      {item.lastError}
                      <button
                        type="button"
                        onClick={() => void onRefreshSubscription(item.id)}
                      >
                        {t("重试")}
                      </button>
                    </div>
                  ) : null}
                </article>
              ))}
            </div>
          )}
          <button
            type="button"
            className="sub-add"
            onClick={() => {
              resetSubscriptionForm();
              setDialog("subscriptions");
            }}
          >
            {t("添加订阅")}
          </button>
        </div>
      ) : null}
    </div>
  );
}
