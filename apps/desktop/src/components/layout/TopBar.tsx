import { describeFailure } from "../../appHelpers";
import type { AppModel } from "../../hooks/useAppController";
import { previewCoreConfig, requestAppExit } from "../../session";
import { MenuBar } from "../MenuBar";
import { IconMoon, IconSearch, IconSun } from "../Icons";
import { IconButton, StatusDot } from "../ui/Ui";

const TITLES: Record<
  AppModel["page"],
  { en: string; zh: string }
> = {
  overview: { zh: "概览", en: "Overview" },
  nodes: { zh: "节点", en: "Nodes" },
  proxies: { zh: "当前代理", en: "Proxies" },
  connections: { zh: "当前连接", en: "Connections" },
  traffic: { zh: "流量", en: "Traffic" },
  logs: { zh: "信息", en: "Logs" },
  routing: { zh: "路由", en: "Routing" },
  dns: { zh: "DNS", en: "Resolver" },
  settings: { zh: "设置", en: "Settings" },
};

export function TopBar({ app }: { app: AppModel }) {
  const {
    t,
    page,
    connected,
    busy,
    node,
    theme,
    setTheme,
    setPaletteOpen,
    onToggleConnect,
    openCreateForm,
    onImportClipboard,
    setDialog,
    onRefreshAllSubscriptions,
    onRefreshSubscription,
    nodeGroupFilter,
    nodeGroups,
    subscriptions,
    openSettings,
    goTo,
    onCheckUpdate,
    onCheckCoreUpdate,
    onReload,
    onClearTraffic,
    onExportPreferences,
    onExportProfile,
    onImportProfile,
    setCoreConfig,
    setError,
    fontSize,
    setFontSize,
    locale,
    setLocale,
    onChangeSettings,
  } = app;

  const title = TITLES[page];
  const isMac = typeof navigator !== "undefined" && /Mac/u.test(navigator.platform);

  return (
    <header className="app-topbar">
      <div className="topbar-title">
        <strong>{t(title.zh)}</strong>
        <span>{title.en}</span>
      </div>
      <button
        type="button"
        className="palette-trigger"
        onClick={() => setPaletteOpen(true)}
      >
        <IconSearch />
        <span>{t("搜索节点 · 执行命令")}</span>
        <kbd>{isMac ? "⌘K" : "Ctrl+K"}</kbd>
      </button>
      <div className="topbar-actions">
        <span className={connected ? "connect-chip is-on" : "connect-chip"}>
          <StatusDot tone={connected ? "ok" : "muted"} />
          {connected ? t("已连接") : t("未连接")}
        </span>
        <button
          type="button"
          className={connected ? "btn-ghost connect-action" : "btn-primary connect-action"}
          disabled={busy || (!connected && node === null)}
          onClick={onToggleConnect}
        >
          {connected ? t("断开") : t("连接")}
        </button>
        <IconButton
          label={theme === "dark" ? t("浅色主题") : t("深色主题")}
          onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
        >
          {theme === "dark" ? <IconSun /> : <IconMoon />}
        </IconButton>
        <div className="overflow-menu" aria-label={t("主菜单")}>
        <MenuBar
          busy={busy}
          t={t}
          onAddServer={(protocol) => openCreateForm(protocol)}
          onOpenImport={() => setDialog("import")}
          onImportClipboard={() => void onImportClipboard()}
          onOpenSubscriptions={() => {
            goTo("nodes");
            app.setNodesTab("subs");
            setDialog("subscriptions");
          }}
          onRefreshSubscriptions={() => void onRefreshAllSubscriptions()}
          onRefreshSubscriptionsViaProxy={() => void onRefreshAllSubscriptions()}
          onRefreshCurrentSubscription={() => {
            const groupName =
              nodeGroupFilter === "all" || nodeGroupFilter === "ungrouped"
                ? null
                : (nodeGroups.find((group) => group.id === nodeGroupFilter)?.name ??
                  null);
            const current =
              subscriptions.find((item) => item.name === groupName) ??
              subscriptions.find((item) => item.enabled) ??
              subscriptions[0];
            if (current !== undefined) {
              void onRefreshSubscription(current.id);
            }
          }}
          onOpenSettings={() => openSettings("general")}
          onOpenRouting={() => {
            goTo("routing");
            void app.onOpenRouting();
          }}
          onOpenDns={() => goTo("dns")}
          onOpenHotkeys={() => {
            openSettings("hotkeys");
            setDialog("hotkeys");
          }}
          onOpenGeo={() => void app.onOpenGeo()}
          onCheckUpdate={() => void onCheckUpdate()}
          onCheckCoreUpdate={() => void onCheckCoreUpdate()}
          onOpenAbout={() => {
            openSettings("about");
            setDialog("about");
          }}
          onReload={() => void onReload()}
          onClearTraffic={() => void onClearTraffic()}
          onExportPreferences={() => void onExportPreferences()}
          onExportProfile={() => setDialog("backup")}
          onViewConfig={() => {
            setDialog("config");
            setCoreConfig("");
            void previewCoreConfig().then(setCoreConfig, (failure: unknown) =>
              setError(describeFailure(failure)),
            );
          }}
          onImportProfile={() => void onImportProfile()}
          onExit={() => {
            void requestAppExit();
          }}
          theme={theme}
          onTheme={setTheme}
          fontSize={fontSize}
          onFontSize={setFontSize}
          locale={locale}
          onLocale={(next) => {
            setLocale(next);
            void onChangeSettings({ locale: next });
          }}
        />
        </div>
      </div>
    </header>
  );
}