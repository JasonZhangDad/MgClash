import type { LogEntry, LogLevel, LogSource } from "../session";

interface MsgViewProps {
  busy: boolean;
  logs: LogEntry[];
  logLevel: LogLevel;
  logSource: LogSource | "all";
  t: (text: string) => string;
  onLogLevel: (level: LogLevel) => void;
  onLogSource: (source: LogSource | "all") => void;
  onRefresh: () => void;
  onClear: () => void;
  onExport: () => void;
}

export function MsgView({
  busy,
  logs,
  logLevel,
  logSource,
  t,
  onLogLevel,
  onLogSource,
  onRefresh,
  onClear,
  onExport,
}: MsgViewProps) {
  return (
    <section className="msg-view" aria-label={t("消息窗口")}>
      <header className="msg-toolbar">
        <strong>{t("消息")}</strong>
        <label>
          {t("级别")}
          <select
            aria-label={t("日志级别")}
            value={logLevel}
            onChange={(event) => onLogLevel(event.target.value as LogLevel)}
          >
            <option value="error">error</option>
            <option value="warn">warn</option>
            <option value="info">info</option>
            <option value="debug">debug</option>
            <option value="trace">trace</option>
          </select>
        </label>
        <label>
          {t("来源")}
          <select
            aria-label={t("日志来源")}
            value={logSource}
            onChange={(event) =>
              onLogSource(event.target.value as LogSource | "all")
            }
          >
            <option value="all">{t("全部")}</option>
            <option value="app">{t("应用")}</option>
            <option value="core">Core</option>
          </select>
        </label>
        <button type="button" onClick={onRefresh}>
          {t("刷新日志")}
        </button>
        <button type="button" onClick={onClear}>
          {t("清空日志")}
        </button>
        <button type="button" disabled={busy} onClick={onExport}>
          {t("导出诊断")}
        </button>
      </header>
      {logs.length === 0 ? (
        <p className="hint">{t("暂无日志")}</p>
      ) : (
        <ul className="log-list" aria-label={t("日志列表")}>
          {logs.map((entry, index) => (
            <li
              key={`${entry.timestampMs}-${index}`}
              className={`log-entry log-${entry.level}`}
            >
              <span className="log-time">
                {new Date(entry.timestampMs).toLocaleTimeString()}
              </span>
              <span className="log-source">
                {entry.source === "core" ? "Core" : t("应用")}
              </span>
              <span className="log-level">{entry.level}</span>
              <span className="log-message">{entry.message}</span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
