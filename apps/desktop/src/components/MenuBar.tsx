import { LOCALES, type Locale } from "../i18n";
import { FONT_SIZES, type FontSize } from "../appHelpers";
import type { ProxyProtocol } from "../session";

interface MenuBarProps {
  busy: boolean;
  t: (text: string) => string;
  onAddServer: (protocol: ProxyProtocol) => void;
  onOpenImport: () => void;
  onImportClipboard: () => void;
  onOpenSubscriptions: () => void;
  onRefreshSubscriptions: () => void;
  onRefreshSubscriptionsViaProxy: () => void;
  onRefreshCurrentSubscription: () => void;
  onOpenSettings: () => void;
  onOpenRouting: () => void;
  onOpenDns: () => void;
  onOpenHotkeys: () => void;
  onOpenGeo: () => void;
  onCheckUpdate: () => void;
  onCheckCoreUpdate: () => void;
  onOpenAbout: () => void;
  onReload: () => void;
  onClearTraffic: () => void;
  onExportPreferences: () => void;
  onExportProfile: () => void;
  onViewConfig: () => void;
  onImportProfile: () => void;
  onExit: () => void;
  layout: "horizontal" | "vertical" | "tab";
  onLayout: (layout: "horizontal" | "vertical" | "tab") => void;
  theme: "light" | "dark";
  onTheme: (theme: "light" | "dark") => void;
  fontSize: FontSize;
  onFontSize: (size: FontSize) => void;
  locale: Locale;
  onLocale: (locale: Locale) => void;
}

const ADD_SERVERS: { protocol: ProxyProtocol; label: string }[] = [
  { protocol: "vmess", label: "添加 [VMess]" },
  { protocol: "vless", label: "添加 [VLESS]" },
  { protocol: "shadowsocks", label: "添加 [Shadowsocks]" },
  { protocol: "trojan", label: "添加 [Trojan]" },
  { protocol: "hysteria2", label: "添加 [Hysteria2]" },
  { protocol: "wireguard", label: "添加 [WireGuard]" },
  { protocol: "socks", label: "添加 [SOCKS]" },
  { protocol: "http", label: "添加 [HTTP]" },
  { protocol: "tuic", label: "添加 [TUIC]" },
  { protocol: "anytls", label: "添加 [AnyTLS]" },
  { protocol: "naive", label: "添加 [Naive]" },
];

export function MenuBar({
  busy,
  t,
  onAddServer,
  onOpenImport,
  onImportClipboard,
  onOpenSubscriptions,
  onRefreshSubscriptions,
  onRefreshSubscriptionsViaProxy,
  onRefreshCurrentSubscription,
  onOpenSettings,
  onOpenRouting,
  onOpenDns,
  onOpenHotkeys,
  onOpenGeo,
  onCheckUpdate,
  onCheckCoreUpdate,
  onOpenAbout,
  onReload,
  onClearTraffic,
  onExportPreferences,
  onExportProfile,
  onViewConfig,
  onImportProfile,
  onExit,
  layout,
  onLayout,
  theme,
  onTheme,
  fontSize,
  onFontSize,
  locale,
  onLocale,
}: MenuBarProps) {
  return (
    <header className="menubar" role="menubar" aria-label={t("主菜单")}>
      <nav className="menubar-nav" aria-label={t("主菜单")}>
        <details className="menu">
          <summary>{t("配置项")}</summary>
          <div className="menu-panel" role="menu">
            <button type="button" role="menuitem" disabled={busy} onClick={onImportClipboard}>
              {t("从剪贴板导入分享链接")}
            </button>
            <button type="button" role="menuitem" disabled={busy} onClick={onOpenImport}>
              {t("扫描屏幕上的二维码")}
            </button>
            <button type="button" role="menuitem" disabled={busy} onClick={onOpenImport}>
              {t("扫描二维码图片")}
            </button>
            <hr />
            <button type="button" role="menuitem" disabled={busy} onClick={() => onAddServer("custom")}>
              {t("添加自定义配置")}
            </button>
            <hr />
            {ADD_SERVERS.slice(0, 8).map((entry) => (
              <button
                key={entry.protocol}
                type="button"
                role="menuitem"
                disabled={busy}
                onClick={() => onAddServer(entry.protocol)}
              >
                {t(entry.label)}
              </button>
            ))}
            <hr />
            {ADD_SERVERS.slice(8).map((entry) => (
              <button
                key={entry.protocol}
                type="button"
                role="menuitem"
                disabled={busy}
                onClick={() => onAddServer(entry.protocol)}
              >
                {t(entry.label)}
              </button>
            ))}
          </div>
        </details>

        <details className="menu">
          <summary>{t("订阅分组")}</summary>
          <div className="menu-panel" role="menu">
            <button type="button" role="menuitem" disabled={busy} onClick={onOpenSubscriptions}>
              {t("订阅分组设置")}
            </button>
            <hr />
            <button type="button" role="menuitem" disabled={busy} onClick={onRefreshSubscriptions}>
              {t("更新全部订阅 (不通过代理)")}
            </button>
            <button type="button" role="menuitem" disabled={busy} onClick={onRefreshSubscriptionsViaProxy}>
              {t("更新全部订阅 (通过代理)")}
            </button>
            <button type="button" role="menuitem" disabled={busy} onClick={onRefreshCurrentSubscription}>
              {t("更新当前订阅 (不通过代理)")}
            </button>
            <button type="button" role="menuitem" disabled={busy} onClick={onRefreshCurrentSubscription}>
              {t("更新当前订阅 (通过代理)")}
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
            <button type="button" role="menuitem" disabled={busy} onClick={onViewConfig}>
              {t("完整配置模板")}
            </button>
            <button type="button" role="menuitem" disabled={busy} onClick={onOpenHotkeys}>
              {t("全局热键设置")}
            </button>
            <hr />
            <button type="button" role="menuitem" disabled={busy} onClick={onClearTraffic}>
              {t("清除流量统计")}
            </button>
            <hr />
            <button type="button" role="menuitem" disabled={busy} onClick={onExportPreferences}>
              {t("导出设置")}
            </button>
            <button type="button" role="menuitem" disabled={busy} onClick={onExportProfile}>
              {t("备份和还原")}
            </button>
            <button type="button" role="menuitem" disabled={busy} onClick={onImportProfile}>
              {t("导入完整配置")}
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
            <button type="button" role="menuitem" disabled={busy} onClick={onOpenGeo}>
              {t("更新 Geo 文件")}
            </button>
            <hr />
            <button type="button" role="menuitem" onClick={onOpenAbout}>
              {t("关于")}
            </button>
          </div>
        </details>

        <button type="button" className="menu-top" disabled={busy} onClick={onReload}>
          {t("重启服务")}
        </button>
        <button type="button" className="menu-top" onClick={onExit}>
          {t("退出")}
        </button>
      </nav>

      <div className="menubar-actions">
        <details className="menu theme-menu">
          <summary className="theme-trigger" aria-label={t("主题")}>
            ⋮
          </summary>
          <div className="menu-panel theme-panel" role="menu">
            <label>
              {t("主题")}
              <select
                aria-label={t("主题")}
                value={theme}
                onChange={(event) => onTheme(event.target.value as "light" | "dark")}
              >
                <option value="light">{t("浅色主题")}</option>
                <option value="dark">{t("深色主题")}</option>
              </select>
            </label>
            <label>
              {t("字体大小")}
              <select
                aria-label={t("字体大小")}
                value={fontSize}
                onChange={(event) => onFontSize(Number(event.target.value) as FontSize)}
              >
                {FONT_SIZES.map((size) => (
                  <option key={size} value={size}>
                    {size}
                  </option>
                ))}
              </select>
            </label>
            <label>
              {t("语言")}
              <select
                aria-label={t("界面语言")}
                value={locale}
                onChange={(event) => onLocale(event.target.value as Locale)}
              >
                {LOCALES.map((entry) => (
                  <option key={entry.id} value={entry.id}>
                    {entry.label}
                  </option>
                ))}
              </select>
            </label>
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
      </div>
    </header>
  );
}
