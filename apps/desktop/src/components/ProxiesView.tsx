import { formatBytes } from "../appHelpers";
import type {
  NodeGroupStrategy,
  NodeGroupSummary,
  NodeSummary,
  NodeTraffic,
} from "../session";

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

  return (
    <section className="proxies-pane" aria-label={t("代理组")}>
      <nav className="group-rail" aria-label={t("代理组列表")}>
        {groups.map((candidate) => (
          <button
            key={candidate.id}
            type="button"
            className={
              candidate.id === group?.id ? "proxy-card active" : "proxy-card"
            }
            onClick={() => onSelectGroup(candidate.id)}
          >
            <span className="proxy-card-name">{candidate.name}</span>
            <span className="proxy-card-type">{candidate.strategy}</span>
          </button>
        ))}
      </nav>

      {group === undefined ? (
        <p className="hint">{t("尚未创建代理组")}</p>
      ) : (
        <>
          <div className="profiles-toolbar">
            <strong>{group.name}</strong>
            <label>
              {t("策略")}
              <select
                aria-label={t("代理组策略")}
                disabled={busy}
                value={group.strategy}
                onChange={(event) =>
                  onStrategy(group.id, event.target.value as NodeGroupStrategy)
                }
              >
                <option value="select">{t("手动选择")}</option>
                <option value="urlTest">{t("自动")}</option>
                <option value="fallback">{t("故障转移")}</option>
                <option value="loadBalance">{t("负载均衡")}</option>
              </select>
            </label>
            <span>
              {t("节点数")} {members.length}
            </span>
            <button
              type="button"
              disabled={busy || testing || members.length === 0}
              onClick={() => onTestGroup(members.map((node) => node.id))}
            >
              {t("测试本组延迟")}
            </button>
            <span aria-label={t("代理组流量")}>
              {t("今日")} ↑ {formatBytes(traffic.todayUploadBytes)} ↓{" "}
              {formatBytes(traffic.todayDownloadBytes)} · {t("总计")} ↑{" "}
              {formatBytes(traffic.totalUploadBytes)} ↓{" "}
              {formatBytes(traffic.totalDownloadBytes)}
            </span>
          </div>

          {members.length === 0 ? (
            <p className="hint">{t("当前分组没有节点")}</p>
          ) : (
            <table className="node-list" aria-label={t("代理组节点")}>
              <thead>
                <tr>
                  <th>{t("名称")}</th>
                  <th>{t("协议")}</th>
                  <th>{t("服务器")}</th>
                  <th>{t("延迟")}</th>
                  <th>{t("操作")}</th>
                </tr>
              </thead>
              <tbody>
                {members.map((node) => (
                  <tr
                    key={node.id}
                    className={
                      node.id === activeNodeId ? "active-node" : undefined
                    }
                  >
                    <td>
                      {node.name}
                      {node.id === activeNodeId ? (
                        <em className="tag-active">{t("活动")}</em>
                      ) : null}
                    </td>
                    <td>{node.protocol}</td>
                    <td>{`${node.server}:${node.port}`}</td>
                    <td>
                      {node.latencyMs === null ? "—" : `${node.latencyMs} ms`}
                    </td>
                    <td>
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
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </>
      )}
    </section>
  );
}
