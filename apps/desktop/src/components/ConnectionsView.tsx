import { formatBytes } from "../appHelpers";
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
}

/** How long a connection has been open, as the table shows it. */
function age(start: string, now: number): string {
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
}: ConnectionsViewProps) {
  const needle = query.trim().toLowerCase();
  const connections = (snapshot?.connections ?? []).filter(
    (entry) =>
      needle === "" ||
      [entry.host, entry.destination, entry.process, entry.rule, entry.chain]
        .join(" ")
        .toLowerCase()
        .includes(needle),
  );
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
        <span>
          {t("连接数")} {connections.length}
        </span>
        <span>
          ↑ {formatBytes(snapshot?.uploadTotalBytes ?? 0)} ↓{" "}
          {formatBytes(snapshot?.downloadTotalBytes ?? 0)}
        </span>
        <button
          type="button"
          disabled={busy || !connected || !supported}
          onClick={onRefresh}
        >
          {t("刷新")}
        </button>
        <button
          type="button"
          disabled={
            busy || !connected || !supported || connections.length === 0
          }
          onClick={onCloseAll}
        >
          {t("全部关闭")}
        </button>
      </div>

      {!supported ? (
        <p className="hint">{t("Xray 不提供连接列表，请改用 sing-box。")}</p>
      ) : !connected ? (
        <p className="hint">{t("连接后才会有连接记录")}</p>
      ) : connections.length === 0 ? (
        <p className="hint">{t("当前没有连接")}</p>
      ) : (
        <table className="node-list" aria-label={t("连接列表")}>
          <thead>
            <tr>
              <th>{t("主机")}</th>
              <th>{t("目标")}</th>
              <th>{t("网络")}</th>
              <th>{t("进程")}</th>
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
              <tr key={entry.id}>
                <td>{entry.host}</td>
                <td>{entry.destination}</td>
                <td>{entry.network}</td>
                <td>{entry.process === "" ? "—" : entry.process}</td>
                <td>{entry.rule === "" ? "—" : entry.rule}</td>
                <td>{entry.chain === "" ? "—" : entry.chain}</td>
                <td>{formatBytes(entry.uploadBytes)}</td>
                <td>{formatBytes(entry.downloadBytes)}</td>
                <td>{age(entry.start, now)}</td>
                <td>
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
