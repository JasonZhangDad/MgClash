import type { ProxyProtocol } from "../session";

interface MenuBarProps {
  busy: boolean;
  connected: boolean;
  t: (text: string) => string;
  onAddServer: (protocol: ProxyProtocol) => void;
  onOpenImport: () => void;
  onOpenCreate: () => void;
  onOpenSubscriptions: () => void;
  onRefreshSubscriptions: () => void;
  onOpenSettings: () => void;
  onOpenRouting: () => void;
  onOpenDns: () => void;
  onOpenGeo: () => void;
  onCheckUpdate: () => void;
  onCheckCoreUpdate: () => void;
  onOpenAbout: () => void;
  onReload: () => void;
  onClearTraffic: () => void;
  onExportAllLinks: () => void;
  onExportPreferences: () => void;
  onImportPreferences: () => void;
  onExportProfile: () => void;
  onImportProfile: () => void;
  onPreviousNode: () => void;
  onNextNode: () => void;
  onConnect: () => void;
  onDisconnect: () => void;
  canConnect: boolean;
  canStepNode: boolean;
  layout: "horizontal" | "vertical" | "tab";
  onLayout: (layout: "horizontal" | "vertical" | "tab") => void;
  theme: "light" | "dark";
  onTheme: (theme: "light" | "dark") => void;
  msgVisible: boolean;
  onToggleMsg: () => void;
}

export function MenuBar({
  busy,
  connected,
  t,
  onAddServer,
  onOpenImport,
  onOpenCreate,
  onOpenSubscriptions,
  onRefreshSubscriptions,
  onOpenSettings,
  onOpenRouting,
  onOpenDns,
  onOpenGeo,
  onCheckUpdate,
  onCheckCoreUpdate,
  onOpenAbout,
  onReload,
  onClearTraffic,
  onExportAllLinks,
  onExportPreferences,
  onImportPreferences,
  onExportProfile,
  onImportProfile,
  onPreviousNode,
  onNextNode,
  onConnect,
  onDisconnect,
  canConnect,
  canStepNode,
  layout,
  onLayout,
  theme,
  onTheme,
  msgVisible,
  onToggleMsg,
}: MenuBarProps) {
  return (
    <header className="menubar" role="menubar" aria-label={t("主菜单")}>
      <div className="menubar-brand" aria-hidden="true">
        MgClash
      </div>

      <nav className="menubar-nav" aria-label={t("主菜单")}>
        <details className="menu">
          <summary>{t("服务器")}</summary>
          <div className="menu-panel" role="menu">
            <button type="button" role="menuitem" disabled={busy || connected} onClick={() => onAddServer("vless")}>
              {t("添加")} VLESS {t("服务器")}
            </button>
            <button type="button" role="menuitem" disabled={busy || connected} onClick={() => onAddServer("vmess")}>
              {t("添加")} VMess {t("服务器")}
            </button>
            <button type="button" role="menuitem" disabled={busy || connected} onClick={() => onAddServer("shadowsocks")}>
              {t("添加")} Shadowsocks {t("服务器")}
            </button>
            <button type="button" role="menuitem" disabled={busy || connected} onClick={() => onAddServer("trojan")}>
              {t("添加")} Trojan {t("服务器")}
            </button>
            <button type="button" role="menuitem" disabled={busy || connected} onClick={() => onAddServer("hysteria2")}>
              {t("添加")} Hysteria2 {t("服务器")}
            </button>
            <button type="button" role="menuitem" disabled={busy || connected} onClick={() => onAddServer("tuic")}>
              {t("添加")} TUIC {t("服务器")}
            </button>
            <button type="button" role="menuitem" disabled={busy || connected} onClick={() => onAddServer("socks")}>
              {t("添加")} SOCKS5 {t("服务器")}
            </button>
            <button type="button" role="menuitem" disabled={busy || connected} onClick={() => onAddServer("http")}>
              {t("添加")} HTTP {t("服务器")}
            </button>
            <button type="button" role="menuitem" disabled={busy || connected} onClick={() => onAddServer("wireguard")}>
              {t("添加")} WireGuard {t("服务器")}
            </button>
            <button type="button" role="menuitem" disabled={busy || connected} onClick={() => onAddServer("anytls")}>
              {t("添加")} AnyTLS {t("服务器")}
            </button>
            <button type="button" role="menuitem" disabled={busy || connected} onClick={() => onAddServer("naive")}>
              {t("添加")} Naive {t("服务器")}
            </button>
            <button type="button" role="menuitem" disabled={busy || connected} onClick={() => onAddServer("custom")}>
              {t("添加自定义配置")}
            </button>
            <hr />
            <button type="button" role="menuitem" disabled={busy || connected} onClick={onOpenCreate}>
              {t("手动创建")}
            </button>
            <button type="button" role="menuitem" disabled={busy || connected} onClick={onOpenImport}>
              {t("导入节点")}
            </button>
            <button type="button" role="menuitem" disabled={busy} onClick={onExportAllLinks}>
              {t("导出全部分享链接")}
            </button>
            <button type="button" role="menuitem" disabled={busy || !canStepNode} onClick={onPreviousNode}>
              {t("上一节点")}
            </button>
            <button type="button" role="menuitem" disabled={busy || !canStepNode} onClick={onNextNode}>
              {t("下一节点")}
            </button>
            <button type="button" role="menuitem" disabled={busy} onClick={onReload}>
              {t("重新加载")}
            </button>
          </div>
        </details>

        <details className="menu">
          <summary>{t("订阅")}</summary>
          <div className="menu-panel" role="menu">
            <button type="button" role="menuitem" disabled={busy} onClick={onOpenSubscriptions}>
              {t("订阅设置")}
            </button>
            <button type="button" role="menuitem" disabled={busy || connected} onClick={onRefreshSubscriptions}>
              {t("全部更新")}
            </button>
          </div>
        </details>

        <details className="menu">
          <summary>{t("设置")}</summary>
          <div className="menu-panel" role="menu">
            <button type="button" role="menuitem" disabled={busy} onClick={onOpenSettings}>
              {t("参数设置")}
            </button>
            <button type="button" role="menuitem" disabled={busy} onClick={onOpenRouting}>
              {t("路由设置")}
            </button>
            <button type="button" role="menuitem" disabled={busy} onClick={onOpenDns}>
              DNS
            </button>
            <button type="button" role="menuitem" disabled={busy} onClick={onOpenGeo}>
              {t("更新 Geo 文件")}
            </button>
            <button type="button" role="menuitem" disabled={busy} onClick={onClearTraffic}>
              {t("清除流量统计")}
            </button>
            <button type="button" role="menuitem" disabled={busy} onClick={onExportPreferences}>
              {t("导出设置")}
            </button>
            <button
              type="button"
              role="menuitem"
              disabled={busy || connected}
              onClick={onImportPreferences}
            >
              {t("导入设置")}
            </button>
            <button type="button" role="menuitem" disabled={busy} onClick={onExportProfile}>
              {t("导出完整配置")}
            </button>
            <button
              type="button"
              role="menuitem"
              disabled={busy || connected}
              onClick={onImportProfile}
            >
              {t("导入完整配置")}
            </button>
            <hr />
            <button type="button" role="menuitem" onClick={() => onTheme(theme === "light" ? "dark" : "light")}>
              {theme === "light" ? t("深色主题") : t("浅色主题")}
            </button>
            <button type="button" role="menuitem" onClick={onToggleMsg}>
              {msgVisible ? t("隐藏消息窗口") : t("显示消息窗口")}
            </button>
            <div className="menu-group-label">{t("主窗布局")}</div>
            <button type="button" role="menuitem" className={layout === "horizontal" ? "active" : undefined} onClick={() => onLayout("horizontal")}>
              {t("左右分栏")}
            </button>
            <button type="button" role="menuitem" className={layout === "vertical" ? "active" : undefined} onClick={() => onLayout("vertical")}>
              {t("上下分栏")}
            </button>
            <button type="button" role="menuitem" className={layout === "tab" ? "active" : undefined} onClick={() => onLayout("tab")}>
              {t("标签页")}
            </button>
          </div>
        </details>

        <details className="menu">
          <summary>{t("帮助")}</summary>
          <div className="menu-panel" role="menu">
            <button type="button" role="menuitem" disabled={busy} onClick={onCheckUpdate}>
              {t("检查更新")}
            </button>
            <button type="button" role="menuitem" disabled={busy} onClick={onCheckCoreUpdate}>
              {t("检查 Core 更新")}
            </button>
            <button type="button" role="menuitem" onClick={onOpenAbout}>
              {t("关于")}
            </button>
          </div>
        </details>
      </nav>

      <div className="menubar-actions">
        <button
          type="button"
          className={connected ? "connect-btn on" : "connect-btn"}
          disabled={busy || !canConnect}
          onClick={() => void (connected ? onDisconnect() : onConnect())}
        >
          {connected ? t("断开") : t("连接")}
        </button>
      </div>
    </header>
  );
}
