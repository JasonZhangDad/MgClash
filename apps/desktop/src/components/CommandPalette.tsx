import { useEffect, useMemo, useState } from "react";

import { latencyQuality } from "../appHelpers";
import type { AppModel } from "../hooks/useAppController";
import { setRoutingMode } from "../session";
import { IconSearch } from "./Icons";
import { LatencyBadge, StatusDot } from "./ui/Ui";

export function CommandPalette({ app }: { app: AppModel }) {
  const {
    t,
    paletteOpen,
    setPaletteOpen,
    nodes,
    nodeTests,
    connected,
    onToggleConnect,
    onTestAllNodes,
    onRefreshAllSubscriptions,
    goTo,
    onActivateNode,
    status,
    run,
  } = app;
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);

  const actions = useMemo(
    () => [
      {
        id: "connect",
        label: connected ? t("断开") : t("连接"),
        hint: "⌃⌥C",
        run: onToggleConnect,
      },
      {
        id: "nodes",
        label: t("切换节点"),
        hint: "⌘⇧N",
        run: () => goTo("nodes"),
      },
      {
        id: "test",
        label: t("全部测速"),
        hint: "⌘T",
        run: () => void onTestAllNodes(),
      },
      {
        id: "subs",
        label: t("更新全部订阅 (不通过代理)"),
        hint: "⌘R",
        run: () => void onRefreshAllSubscriptions(),
      },
      {
        id: "mode",
        label: t("状态栏路由模式"),
        hint: "⌘M",
        run: () => {
          const order = ["global", "rule", "direct"] as const;
          const current = status?.mode ?? "global";
          const next = order[(order.indexOf(current) + 1) % order.length];
          void run(() => setRoutingMode(next));
        },
      },
      {
        id: "logs",
        label: t("信息"),
        hint: "⌘L",
        run: () => goTo("logs"),
      },
    ],
    [
      connected,
      goTo,
      onRefreshAllSubscriptions,
      onTestAllNodes,
      onToggleConnect,
      run,
      status?.mode,
      t,
    ],
  );

  const needle = query.trim().toLowerCase();
  const visibleActions = actions.filter((item) =>
    item.label.toLowerCase().includes(needle),
  );
  const visibleNodes = nodes.filter((node) =>
    `${node.name} ${node.server} ${node.protocol}`.toLowerCase().includes(needle),
  );
  const total = visibleActions.length + visibleNodes.length;

  useEffect(() => {
    setIndex(0);
  }, [query, paletteOpen]);

  useEffect(() => {
    if (!paletteOpen) {
      setQuery("");
    }
  }, [paletteOpen]);

  if (!paletteOpen) {
    return null;
  }

  const runAt = (next: number) => {
    if (next < visibleActions.length) {
      visibleActions[next]?.run();
      setPaletteOpen(false);
      return;
    }
    const node = visibleNodes[next - visibleActions.length];
    if (node) {
      void onActivateNode(node.id);
      setPaletteOpen(false);
    }
  };

  return (
    <div
      className="palette-backdrop"
      onClick={() => setPaletteOpen(false)}
    >
      <div
        className="palette-panel"
        role="dialog"
        aria-label={t("命令面板")}
        onClick={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown") {
            event.preventDefault();
            setIndex((current) => (current + 1) % Math.max(1, total));
          } else if (event.key === "ArrowUp") {
            event.preventDefault();
            setIndex((current) =>
              (current - 1 + Math.max(1, total)) % Math.max(1, total),
            );
          } else if (event.key === "Enter") {
            event.preventDefault();
            runAt(index);
          }
        }}
      >
        <header className="palette-head">
          <IconSearch />
          <input
            autoFocus
            aria-label={t("输入命令或节点名称…")}
            placeholder={t("输入命令或节点名称…")}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
          <kbd>esc</kbd>
        </header>
        <div className="palette-body">
          {visibleActions.length > 0 ? (
            <>
              <p className="nav-group">{t("操作")} ACTIONS</p>
              {visibleActions.map((item, itemIndex) => (
                <button
                  key={item.id}
                  type="button"
                  className={itemIndex === index ? "is-on" : undefined}
                  onMouseEnter={() => setIndex(itemIndex)}
                  onClick={() => runAt(itemIndex)}
                >
                  <span>{item.label}</span>
                  <kbd>{item.hint}</kbd>
                </button>
              ))}
            </>
          ) : null}
          {visibleNodes.length > 0 ? (
            <>
              <p className="nav-group">{t("节点")} NODES</p>
              {visibleNodes.map((node, nodeIndex) => {
                const pos = visibleActions.length + nodeIndex;
                const test = nodeTests[node.id];
                const ms =
                  test?.status === "success" ? test.latencyMs : node.latencyMs;
                return (
                  <button
                    key={node.id}
                    type="button"
                    className={pos === index ? "is-on" : undefined}
                    onMouseEnter={() => setIndex(pos)}
                    onClick={() => runAt(pos)}
                  >
                    <StatusDot
                      tone={node.id === status?.node?.id ? "ok" : "muted"}
                    />
                    <span>{node.name}</span>
                    <span className="mono muted">
                      {node.server}:{node.port}
                    </span>
                    <LatencyBadge
                      quality={ms === null ? null : latencyQuality(ms)}
                      label={ms === null ? "—" : `${ms} ms`}
                    />
                  </button>
                );
              })}
            </>
          ) : null}
        </div>
      </div>
    </div>
  );
}