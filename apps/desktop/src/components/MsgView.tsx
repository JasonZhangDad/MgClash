import { useMemo, useState } from "react";

import type { LogEntry, LogLevel, LogSource } from "../session";
import { IconExport, IconRefresh, IconTrash } from "./Icons";
import { EmptyState, SegmentedControl } from "./ui/Ui";

interface MsgViewProps {
  busy: boolean;
  logs: LogEntry[];
  logLevel: LogLevel;
  logSource: LogSource | "all";
  paused?: boolean;
  autoScroll?: boolean;
  t: (text: string) => string;
  onLogLevel: (level: LogLevel) => void;
  onLogSource: (source: LogSource | "all") => void;
  onRefresh: () => void;
  onClear: () => void;
  onExport: () => void;
  onPaused?: (paused: boolean) => void;
  onAutoScroll?: (autoScroll: boolean) => void;
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
  paused = false,
  autoScroll = true,
  onPaused,
  onAutoScroll,
}: MsgViewProps) {
  const [filter, setFilter] = useState("");
  const visibleLogs = useMemo(() => {
    const query = filter.trim().toLowerCase();
    if (query === "") {
      return logs;
    }
    return logs.filter((entry) => entry.message.toLowerCase().includes(query));
  }, [filter, logs]);

  return (
    <section className="msg-view" aria-label={t("消息窗口")}>
      <header className="msg-toolbar">
        <input
          className="toolbar-search"
          aria-label={t("过滤器")}
          placeholder={t("过滤器")}
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
        />
        <SegmentedControl
          ariaLabel={t("日志级别")}
          value={logLevel}
          onChange={onLogLevel}
          options={[
            { value: "trace", label: t("全部") },
            { value: "debug", label: "Debug" },
            { value: "info", label: "Info" },
            { value: "warn", label: "Warn", tone: "warn" },
            { value: "error", label: "Error", tone: "dang" },
          ]}
        />
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
        <label className="sb-inline">
          {t("自动滚动")}
          <input
            aria-label={t("自动滚动")}
            className="toggle"
            type="checkbox"
            checked={autoScroll}
            onChange={(event) => onAutoScroll?.(event.target.checked)}
          />
        </label>
        <button
          type="button"
          className="icon-btn"
          title={paused ? t("刷新日志") : t("暂停")}
          onClick={() => onPaused?.(!paused)}
        >
          {paused ? t("刷新日志") : t("暂停")}
        </button>
        <button type="button" className="icon-btn" title={t("刷新日志")} onClick={onRefresh}>
          <IconRefresh />
          <span className="sr-only">{t("刷新日志")}</span>
        </button>
        <button type="button" className="icon-btn" title={t("清空日志")} onClick={onClear}>
          <IconTrash />
          <span className="sr-only">{t("清空日志")}</span>
        </button>
        <button type="button" className="icon-btn" disabled={busy} title={t("导出诊断")} onClick={onExport}>
          <IconExport />
          <span className="sr-only">{t("导出诊断")}</span>
        </button>
      </header>
      {visibleLogs.length === 0 ? (
        <EmptyState title={t("暂无日志")} body={t("暂无日志")} />
      ) : (
        <ul className="log-list" aria-label={t("日志列表")}>
          {visibleLogs.map((entry, index) => (
            <li
              key={`${entry.timestampMs}-${index}`}
              className={`log-entry log-${entry.level}`}
            >
              <span className="log-time">
                {new Date(entry.timestampMs).toLocaleTimeString()}
              </span>
              <span className="log-level">{entry.level}</span>
              <span className="log-source">
                {entry.source === "core" ? "Core" : t("应用")}
              </span>
              <span className="log-message">{entry.message}</span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
