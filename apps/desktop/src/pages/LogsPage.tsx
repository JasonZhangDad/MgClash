import { MsgView } from "../components/MsgView";
import type { AppModel } from "../hooks/useAppController";

export function LogsPage({ app }: { app: AppModel }) {
  const {
    t,
    busy,
    logs,
    logLevel,
    logSource,
    setLogLevel,
    setLogSource,
    refreshLogs,
    onClearLogs,
    onExport,
    logPaused,
    setLogPaused,
    logAutoScroll,
    setLogAutoScroll,
  } = app;
  return (
    <div className="page-fill">
      <MsgView
        busy={busy}
        logs={logs}
        logLevel={logLevel}
        logSource={logSource}
        paused={logPaused}
        autoScroll={logAutoScroll}
        t={t}
        onLogLevel={setLogLevel}
        onLogSource={setLogSource}
        onRefresh={() => void refreshLogs()}
        onClear={() => void onClearLogs()}
        onExport={() => void onExport()}
        onPaused={setLogPaused}
        onAutoScroll={setLogAutoScroll}
      />
    </div>
  );
}