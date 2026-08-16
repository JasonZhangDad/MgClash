import { formatBytes } from "../appHelpers";
import { EmptyState } from "./ui/Ui";
import type { ConnectionSnapshot } from "../session";

interface ConnectionsViewProps {
  busy: boolean;
  connected: boolean;
  /// Only sing-box exposes the Clash API the list comes from.
  supported: boolean;
  snapshot: ConnectionSnapshot | null;
  query: string;
  t: (text: string) => string;
  onQuery: (query: string) => void;
  onRefresh: () => void;
  onClose: (id: string) => void;
  onCloseAll: () => void;
  onAddRule: (host: string) => void;
  onSelect?: (id: string) => void;
  selectedId?: string | null;
  processFilter?: string;
  networkFilter?: string;
  ruleFilter?: string;
  chainFilter?: string;
  onProcessFilter?: (value: string) => void;
  onNetworkFilter?: (value: string) => void;
  onRuleFilter?: (value: string) => void;
  onChainFilter?: (value: string) => void;
}

/** How long a connection has been open, as the table shows it. */
export function connectionAge(start: string, now: number): string {
  const opened = Date.parse(start);
  if (Number.isNaN(opened)) {
    return "—";
  }
  const seconds = Math.max(0, Math.round((now - opened) / 1_000));
  if (seconds < 60) {
    return `${seconds}s`;
  }
  if (seconds < 3_600) {
    return `${Math.floor(seconds / 60)}m${seconds % 60}s`;
  }
  return `${Math.floor(seconds / 3_600)}h${Math.floor((seconds % 3_600) / 60)}m`;
}

export function ConnectionsView({
  busy,
  connected,
  supported,
  snapshot,
  query,
  t,
  onQuery,
  onRefresh,
  onClose,
  onCloseAll,
  onAddRule,
  onSelect,
  selectedId,
  processFilter = "",
  networkFilter = "",
  ruleFilter = "",
  chainFilter = "",
  onProcessFilter,
  onNetworkFilter,
  onRuleFilter,
  onChainFilter,
}: ConnectionsViewProps) {
  const needle = query.trim().toLowerCase();
  const connections = (snapshot?.connections ?? []).filter((entry) => {
    if (
      needle !== "" &&
      ![entry.host, entry.destination, entry.process, entry.rule, entry.chain]
        .join(" ")
        .toLowerCase()
        .includes(needle)
    ) {
      return false;
    }
    if (processFilter !== "" && entry.process !== processFilter) {
      return false;
    }
    if (networkFilter !== "" && entry.network !== networkFilter) {
      return false;
    }
    if (ruleFilter !== "" && entry.rule !== ruleFilter) {
      return false;
    }
    if (chainFilter !== "" && entry.chain !== chainFilter) {
      return false;
    }
    return true;
  });
  const now = Date.now();

  return (
    <section className="connections-pane" aria-label={t("连接")}>
      <div className="profiles-toolbar">
        <input
          className="toolbar-search"
          aria-label={t("搜索连接")}
          placeholder={t("搜索连接")}
          value={query}
          onChange={(event) => onQuery(event.target.value)}
        />
        {(
          [
            [t("进程"), "process", processFilter, onProcessFilter],
            [t("网络"), "network", networkFilter, onNetworkFilter],
            [t("规则"), "rule", ruleFilter, onRuleFilter],
            [t("链路"), "chain", chainFilter, onChainFilter],
          ] as const
        ).map(([label, field, value, setter]) => {
          const options = [
            ...new Set(
              (snapshot?.connections ?? [])
                .map((entry) => entry[field])
                .filter((item) => item !== ""),
            ),
          ].sort();
          return (
            <label
              key={field}
              className={value ? "filter-chip is-on" : "filter-chip"}
            >
              {label}
              <select
                aria-label={label}
                value={value}
                onChange={(event) => setter?.(event.target.value)}
              >
                <option value="">{t("全部")}</option>
                {options.map((option) => (
                  <option key={option} value={option}>
                    {option}
                  </option>
                ))}
              </select>
              {value ? (
                <button
                  type="button"
                  className="filter-clear"
                  aria-label={`${t("全部")} ${label}`}
                  onClick={() => setter?.("")}
                >
                  ✕
                </button>
              ) : null}
            </label>
          );
        })}
        <span>
          {t("连接数")} {connections.length}
        </span>
        <span>
          ↑ {formatBytes(snapshot?.uploadTotalBytes ?? 0)} ↓{" "}
          {formatBytes(snapshot?.downloadTotalBytes ?? 0)}
        </span>
        <button
          type="button"
          className="icon-btn"
          disabled={busy || !connected || !supported}
          title={t("刷新")}
          onClick={onRefresh}
        >
          {t("刷新")}
        </button>
        <button
          type="button"
          className="icon-btn"
          disabled={
            busy || !connected || !supported || connections.length === 0
          }
          title={t("全部关闭")}
          onClick={onCloseAll}
        >
          {t("全部关闭")}
        </button>
      </div>

      {!supported ? (
        <EmptyState
          title={t("Xray 不提供连接列表，请改用 sing-box。")}
          body={t("Xray 不提供连接列表，请改用 sing-box。")}
        />
      ) : !connected ? (
        <EmptyState
          title={t("连接后才会有连接记录")}
          body={t("连接后才会有连接记录")}
        />
      ) : connections.length === 0 ? (
        <EmptyState
          title={t("当前没有连接")}
          body={t("当前没有连接")}
        />
      ) : (
        <table className="conn-grid" aria-label={t("连接列表")}>
          <thead>
            <tr>
              <th>{t("主机")}</th>
              <th>{t("进程")}</th>
              <th>{t("网络")}</th>
              <th>{t("规则")}</th>
              <th>{t("链路")}</th>
              <th>{t("上传")}</th>
              <th>{t("下载")}</th>
              <th>{t("时长")}</th>
              <th>{t("操作")}</th>
            </tr>
          </thead>
          <tbody>
            {connections.map((entry) => (
              <tr
                key={entry.id}
                className={entry.id === selectedId ? "active-node" : undefined}
                onClick={() => onSelect?.(entry.id)}
              >
                <td className="mono">{entry.host}</td>
                <td>{entry.process === "" ? "—" : entry.process}</td>
                <td>
                  <em className="ui-badge tone-neutral">{entry.network}</em>
                </td>
                <td>{entry.rule === "" ? "—" : entry.rule}</td>
                <td>{entry.chain === "" ? "—" : entry.chain}</td>
                <td className="mono">{formatBytes(entry.uploadBytes)}</td>
                <td className="mono">{formatBytes(entry.downloadBytes)}</td>
                <td>{connectionAge(entry.start, now)}</td>
                <td className="node-actions">
                  <button
                    type="button"
                    aria-label={`为 ${entry.host} 添加规则`}
                    disabled={busy}
                    onClick={() => onAddRule(entry.host)}
                  >
                    {t("加规则")}
                  </button>
                  <button
                    type="button"
                    aria-label={`${t("关闭连接")} ${entry.host}`}
                    disabled={busy}
                    onClick={() => onClose(entry.id)}
                  >
                    {t("关闭")}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}
