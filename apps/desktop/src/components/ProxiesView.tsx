import { formatBytes, latencyQuality } from "../appHelpers";
import type {
  NodeGroupStrategy,
  NodeGroupSummary,
  NodeSummary,
  NodeTraffic,
} from "../session";
import { LatencyBadge, SegmentedControl, StatusDot } from "./ui/Ui";

interface ProxiesViewProps {
  busy: boolean;
  groups: NodeGroupSummary[];
  nodes: NodeSummary[];
  activeNodeId: string | null;
  /// This group's traffic, added up from its members.
  traffic: NodeTraffic;
  selectedGroupId: string | null;
  testing: boolean;
  t: (text: string) => string;
  onSelectGroup: (groupId: string) => void;
  onStrategy: (groupId: string, strategy: NodeGroupStrategy) => void;
  onTestGroup: (ids: string[]) => void;
  onActivate: (id: string) => void;
}

export function ProxiesView({
  busy,
  groups,
  nodes,
  activeNodeId,
  traffic,
  selectedGroupId,
  testing,
  t,
  onSelectGroup,
  onStrategy,
  onTestGroup,
  onActivate,
}: ProxiesViewProps) {
  const group =
    groups.find((candidate) => candidate.id === selectedGroupId) ?? groups[0];
  const members = nodes.filter(
    (node) => group !== undefined && node.groupId === group.id,
  );

  const strategyLabel = (strategy: NodeGroupStrategy) => {
    switch (strategy) {
      case "urlTest":
        return t("自动");
      case "fallback":
        return t("故障转移");
      case "loadBalance":
        return t("负载均衡");
      default:
        return t("手动选择");
    }
  };

  return (
    <section className="proxies-pane" aria-label={t("代理组")}>
      <nav className="group-rail" aria-label={t("代理组列表")}>
        <p className="nav-group">{t("代理组")} PROXY GROUPS</p>
        {groups.map((candidate) => {
          const count = nodes.filter((node) => node.groupId === candidate.id).length;
          const active = nodes.find(
            (node) => node.groupId === candidate.id && node.id === activeNodeId,
          );
          return (
          <button
            key={candidate.id}
            type="button"
            className={
              candidate.id === group?.id ? "proxy-card active" : "proxy-card"
            }
            onClick={() => onSelectGroup(candidate.id)}
          >
            <span className="proxy-card-head">
              <span className="proxy-card-name">{candidate.name}</span>
              <span className="mono muted">{count}</span>
            </span>
            <span className="proxy-card-type">{strategyLabel(candidate.strategy)}</span>
            <span className="proxy-card-active">
              <StatusDot
                size={5}
                tone={active ? "ok" : "muted"}
              />
              {active?.name ?? "—"}
            </span>
          </button>
          );
        })}
      </nav>

      {group === undefined ? (
        <p className="hint">{t("尚未创建代理组")}</p>
      ) : (
        <div className="proxy-main">
          <div className="proxy-toolbar">
            <strong>{group.name}</strong>
            <span>{t("策略")}</span>
            <SegmentedControl
              ariaLabel={t("代理组策略")}
              disabled={busy}
              value={group.strategy}
              onChange={(strategy) => onStrategy(group.id, strategy)}
              options={[
                { value: "select", label: t("手动选择") },
                { value: "urlTest", label: t("自动") },
                { value: "fallback", label: t("故障转移") },
                { value: "loadBalance", label: t("负载均衡") },
              ]}
            />
            <span>
              {t("节点数")} {members.length}
            </span>
            <span aria-label={t("代理组流量")}>
              {t("今日")} ↑ {formatBytes(traffic.todayUploadBytes)} ↓{" "}
              {formatBytes(traffic.todayDownloadBytes)} · {t("总计")} ↑{" "}
              {formatBytes(traffic.totalUploadBytes)} ↓{" "}
              {formatBytes(traffic.totalDownloadBytes)}
            </span>
            <button
              type="button"
              disabled={busy || testing || members.length === 0}
              onClick={() => onTestGroup(members.map((node) => node.id))}
            >
              {t("测试本组延迟")}
            </button>
          </div>

          {members.length === 0 ? (
            <p className="hint">{t("当前分组没有节点")}</p>
          ) : (
            <div className="member-grid" aria-label={t("代理组节点")}>
              {members.map((node) => (
                <article
                  key={node.id}
                  className={
                    node.id === activeNodeId
                      ? "member-card is-current"
                      : "member-card"
                  }
                >
                  <StatusDot
                    size={8}
                    tone={
                      node.id === activeNodeId
                        ? "ok"
                        : node.enabled
                          ? "muted"
                          : "line"
                    }
                  />
                  <span className="member-copy">
                    <span className="member-name">
                      {node.name}
                      {node.id === activeNodeId ? (
                        <em className="tag-active">{t("活动")}</em>
                      ) : null}
                    </span>
                    <span className="mono muted">
                      {node.server}:{node.port}
                    </span>
                  </span>
                  <LatencyBadge
                    quality={
                      node.latencyMs === null
                        ? null
                        : latencyQuality(node.latencyMs)
                    }
                    label={
                      node.latencyMs === null ? "—" : `${node.latencyMs} ms`
                    }
                  />
                  <button
                    type="button"
                    aria-label={`${t("设为活动")} ${node.name}`}
                    disabled={
                      busy || !node.enabled || node.id === activeNodeId
                    }
                    onClick={() => onActivate(node.id)}
                  >
                    {t("设为活动")}
                  </button>
                </article>
              ))}
            </div>
          )}
        </div>
      )}
    </section>
  );
}
