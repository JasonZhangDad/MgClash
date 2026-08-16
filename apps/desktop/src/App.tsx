import { isBackendMissing, isCoreStartFailure, TUN_NOTICE } from "./appHelpers";
import { IconWarn } from "./components/Icons";
import { CommandPalette } from "./components/CommandPalette";
import { AppStatusBar } from "./components/layout/AppStatusBar";
import { Sidebar } from "./components/layout/Sidebar";
import { TopBar } from "./components/layout/TopBar";
import { NodeInspector } from "./components/nodes/NodeInspector";
import { NodesPage } from "./components/nodes/NodesPage";
import { AppDialogs } from "./components/overlays/AppDialogs";
import { useAppController } from "./hooks/useAppController";
import { ConnectionsPage } from "./pages/ConnectionsPage";
import { LogsPage } from "./pages/LogsPage";
import { OverviewPage } from "./pages/OverviewPage";
import { DnsPage } from "./pages/DnsPage";
import { RoutingPage } from "./pages/RoutingPage";
import { SettingsPage } from "./pages/SettingsPage";
import { ProxiesPage } from "./pages/ProxiesPage";
import { TrafficPage } from "./pages/TrafficPage";
import {
  dismissSystemProxyRecovery,
  recoverSystemProxy,
} from "./session";

export default function App() {
  const app = useAppController();
  const {
    t,
    page,
    theme,
    systemProxyStartup,
    exportedTo,
    error,
    coreReadiness,
    connected,
    busy,
    noticeDismissed,
    platform,
    dismissNotice,
    resolveSystemProxyStartup,
    onDownloadCore,
    inspected,
    onToggleConnect,
    goTo,
    openSettings,
    onRestartCore,
  } = app;
  const backendMissing = error !== null && isBackendMissing(error);
  const coreFailed =
    error !== null && !backendMissing && isCoreStartFailure(error);

  return (
    <main
      className="app-shell"
      data-theme={theme}
      data-page={page}
    >
      <Sidebar app={app} />
      <div className="app-main">
        <TopBar app={app} />
        <div className="workspace">
          {systemProxyStartup === "restoreRequired" && (
            <section className="notice" aria-label={t("系统代理恢复")}>
              <p>
                {t("检测到上次异常退出留下的系统代理设置。恢复原设置可以避免系统继续指向已经停止的本地代理。")}
              </p>
              <div className="actions">
                <button
                  type="button"
                  disabled={busy}
                  onClick={() =>
                    void resolveSystemProxyStartup(recoverSystemProxy)
                  }
                >
                  {t("恢复原设置")}
                </button>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() =>
                    void resolveSystemProxyStartup(dismissSystemProxyRecovery)
                  }
                >
                  {t("保留当前设置")}
                </button>
              </div>
            </section>
          )}

          {exportedTo !== null && (
            <p className="success" role="status">
              已导出到 {exportedTo}
            </p>
          )}
          {backendMissing ? (
            <section
              className="notice"
              role="alert"
              aria-label={t("需要桌面应用")}
            >
              <strong>{t("需要桌面应用")}</strong>
              <p>
                {t("节点和订阅来自桌面应用后端。请使用 Tauri 桌面程序启动，而不是单独打开 Vite 页面。")}
              </p>
              <p className="muted">
                {t("没有可用的后端连接，因此不会显示演示数据。")}
              </p>
            </section>
          ) : coreFailed ? (
            <section
              className="error-banner"
              role="alert"
              aria-label={t("内核启动失败")}
            >
              <strong>
                {t("内核启动失败")} · Core failed to start
              </strong>
              <p className="mono">{error}</p>
              <p className="muted">{app.status?.core ?? "—"}</p>
              <div className="actions">
                <button
                  type="button"
                  className="btn-danger"
                  disabled={busy}
                  onClick={connected ? onRestartCore : onToggleConnect}
                >
                  {t("重试")}
                </button>
                <button type="button" onClick={() => goTo("logs")}>
                  {t("查看日志")}
                </button>
                <button
                  type="button"
                  onClick={() => openSettings("network")}
                >
                  {t("修改端口")}
                </button>
              </div>
            </section>
          ) : error !== null ? (
            <p className="error" role="alert">
              {error}
            </p>
          ) : null}

          {coreReadiness?.singBox === false && coreReadiness.xray === false && (
            <div className="notice" role="note" aria-label={t("缺少 Core")}>
              <p>
                {t("还没有可用的 Core。MgClash 不捆绑 Core，请下载一个官方版本；下载后会按官方发布的摘要校验。")}
              </p>
              <div className="actions">
                <button
                  type="button"
                  disabled={busy || connected}
                  onClick={() => void onDownloadCore("sing-box")}
                >
                  {t("下载 sing-box")}
                </button>
                <button
                  type="button"
                  disabled={busy || connected}
                  onClick={() => void onDownloadCore("xray")}
                >
                  {t("下载 Xray")}
                </button>
              </div>
            </div>
          )}

          {!noticeDismissed && (
            <div
              className="alert-banner"
              role="note"
              aria-label={t("未签名提示")}
            >
              <IconWarn />
              <div>
                <strong>
                  {t("未签名")} · TUN
                </strong>
                <p>
                  {t("这是未签名版本：macOS Gatekeeper 与 Windows SmartScreen 会在首次打开时提示，需要你手动确认后才能运行。")}
                </p>
                <p>{platform ? TUN_NOTICE[platform.tunAvailability] : ""}</p>
              </div>
              <button type="button" onClick={dismissNotice}>
                {t("我知道了")}
              </button>
            </div>
          )}

          <div className="page-host">
            <section hidden={page !== "overview"} aria-label={t("概览")}>
              <OverviewPage app={app} />
            </section>
            <section hidden={page !== "nodes"} aria-label={t("节点")}>
              <NodesPage app={app} />
            </section>
            <section hidden={page !== "proxies"} aria-label={t("当前代理")}>
              <ProxiesPage app={app} />
            </section>
            <section hidden={page !== "connections"} aria-label={t("当前连接")}>
              <ConnectionsPage app={app} />
            </section>
            <section hidden={page !== "traffic"} aria-label={t("流量")}>
              <TrafficPage app={app} />
            </section>
            <section hidden={page !== "logs"} aria-label={t("消息窗口")}>
              <LogsPage app={app} />
            </section>
            <section
              hidden={page !== "routing"}
              aria-label={t("路由规则")}
              role="dialog"
            >
              <RoutingPage app={app} />
            </section>
            <section hidden={page !== "dns"} aria-label="DNS" role="dialog">
              <DnsPage app={app} />
            </section>
            <section
              hidden={page !== "settings"}
              aria-label={t("设置")}
              role="dialog"
            >
              <SettingsPage app={app} />
            </section>
            {inspected !== null ? <NodeInspector app={app} /> : null}
          </div>
        </div>
        <AppStatusBar app={app} />
      </div>
      <AppDialogs app={app} />
      <CommandPalette app={app} />
    </main>
  );
}