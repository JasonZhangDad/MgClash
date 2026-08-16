import { LOCALES } from "../i18n";
import {
  DEFAULT_SPEED_TEST_URL,
  DEFAULT_URL_TEST_ADDRESS,
  FONT_SIZES,
  TUN_LABEL,
  TUN_NOTICE,
  describeFailure,
  type FontSize,
  type SettingsSection,
  type ThemeMode,
} from "../appHelpers";
import type { AppModel } from "../hooks/useAppController";
import {
  previewCoreConfig,
  type CorePreference,
  type LogLevel,
  type SystemProxyMode,
} from "../session";
import type { ReactNode } from "react";
import { Card } from "../components/ui/Ui";
import { IconInfo } from "../components/Icons";

function SettingsRow({
  label,
  hint,
  children,
}: {
  children: ReactNode;
  hint?: string;
  label: string;
}) {
  return (
    <div className="settings-row">
      <div>
        <strong>{label}</strong>
        {hint ? <span>{hint}</span> : null}
      </div>
      <div>{children}</div>
    </div>
  );
}

const SECTIONS: { id: SettingsSection; en: string; zh: string }[] = [
  { id: "general", zh: "通用", en: "General" },
  { id: "network", zh: "网络", en: "Network" },
  { id: "tun", zh: "TUN", en: "Tunnel" },
  { id: "core", zh: "内核", en: "Core" },
  { id: "dns", zh: "DNS", en: "Resolver" },
  { id: "routing", zh: "路由", en: "Routing" },
  { id: "appearance", zh: "外观", en: "Appearance" },
  { id: "language", zh: "语言", en: "Language" },
  { id: "hotkeys", zh: "热键", en: "Hotkeys" },
  { id: "data", zh: "数据与备份", en: "Data" },
  { id: "updates", zh: "更新", en: "Updates" },
  { id: "advanced", zh: "高级", en: "Advanced" },
  { id: "about", zh: "关于", en: "About" },
];

export function SettingsPage({ app }: { app: AppModel }) {
  const {
    t,
    busy,
    connected,
    settings,
    settingsSection,
    setSettingsSection,
    onChangeSettings,
    platform,
    platformError,
    configTemplate,
    setConfigTemplate,
    theme,
    setTheme,
    fontSize,
    setFontSize,
    locale,
    setLocale,
    goTo,
    onExportPreferences,
    onImportPreferences,
    onExportProfile,
    onImportProfile,
    onClearTraffic,
    onCheckUpdate,
    onCheckCoreUpdate,
    setDialog,
    setCoreConfig,
    setError,
    coreConfig,
  } = app;

  const hiddenIf = (id: SettingsSection) => settingsSection !== id;

  return (
    <div className="settings-page" aria-label={t("设置")}>
      <nav className="settings-nav" aria-label={t("设置")}>
        {SECTIONS.map((section) => (
          <button
            key={section.id}
            type="button"
            className={
              settingsSection === section.id ? "settings-nav-item is-on" : "settings-nav-item"
            }
            onClick={() => setSettingsSection(section.id)}
          >
            <span>{t(section.zh)}</span>
            <em data-en={section.en} className="nav-en" />
          </button>
        ))}
      </nav>
      <div className="settings-main">
        <header className="settings-head">
          <strong>
            {t(SECTIONS.find((item) => item.id === settingsSection)?.zh ?? "设置")}
          </strong>
          <span>
            {SECTIONS.find((item) => item.id === settingsSection)?.en ?? "Settings"}
          </span>
        </header>

        {settings === null ? (
          <p className="hint">{t("正在读取设置…")}</p>
        ) : (
          <div className="settings-form form-grid" aria-label={t("应用设置")}>
            <div data-settings-section="tun" hidden={hiddenIf("tun")}>
              <label className="checkbox-label">
                <input
                  aria-label={t("启用 TUN")}
                  type="checkbox"
                  checked={settings.tunEnabled}
                  disabled={
                    busy ||
                    connected ||
                    platform?.tunAvailability === "unavailableInUnsignedBuild"
                  }
                  onChange={(event) =>
                    void onChangeSettings({ tunEnabled: event.target.checked })
                  }
                />
                {t("使用 TUN 接管全局流量")}
              </label>
              <p className="hint">
                TUN：{platform ? TUN_LABEL[platform.tunAvailability] : "—"}
              </p>
              <p className="hint">
                {platform ? TUN_NOTICE[platform.tunAvailability] : ""}
                TUN 与系统代理互斥，启用后本次会话不会修改系统代理。
              </p>
              <p className="settings-note">
                <IconInfo /> {platform ? TUN_NOTICE[platform.tunAvailability] : ""}
              </p>
            </div>

            <div data-settings-section="general" hidden={hiddenIf("general")}>
              <SettingsRow
                label={t("开机启动")}
                hint={t("登录系统时自动启动 MgClash")}
              >
                <input
                  aria-label={t("开机启动")}
                  type="checkbox"
                  className="toggle"
                  checked={settings.launchAtLogin}
                  disabled={busy}
                  onChange={(event) =>
                    void onChangeSettings({ launchAtLogin: event.target.checked })
                  }
                />
              </SettingsRow>
              <SettingsRow
                label={t("启动时自动连接")}
                hint={t("启动时自动连接上次选中的节点")}
              >
                <input
                  aria-label={t("启动时自动连接")}
                  type="checkbox"
                  className="toggle"
                  checked={settings.connectOnLaunch}
                  disabled={busy}
                  onChange={(event) =>
                    void onChangeSettings({
                      connectOnLaunch: event.target.checked,
                    })
                  }
                />
              </SettingsRow>
              <SettingsRow
                label={t("关闭时最小化到托盘")}
                hint={t("关闭窗口时最小化到托盘，而不是退出")}
              >
                <input
                  aria-label={t("关闭时最小化到托盘")}
                  type="checkbox"
                  className="toggle"
                  checked={settings.closeToTray}
                  disabled={busy}
                  onChange={(event) =>
                    void onChangeSettings({ closeToTray: event.target.checked })
                  }
                />
              </SettingsRow>
            </div>

            <div data-settings-section="core-type" hidden={hiddenIf("core")}>
              <label>
                Core
                <select
                  aria-label={t("Core 选择")}
                  value={settings.corePreference}
                  disabled={busy}
                  onChange={(event) =>
                    void onChangeSettings({
                      corePreference: event.target.value as CorePreference,
                    })
                  }
                >
                  <option value="auto">{t("自动")}</option>
                  <option value="sing-box">sing-box</option>
                  <option value="xray">Xray</option>
                </select>
              </label>
              <p className="hint">
                {t("自动模式按节点协议和能力矩阵决定。Xray 不支持 Hysteria2 / TUIC，选中后遇到该协议的节点会提示原因。")}
              </p>
            </div>

            <div data-settings-section="core" hidden={hiddenIf("core")}>
              <label>
                {t("默认日志级别")}
                <select
                  aria-label={t("默认日志级别")}
                  value={settings.logLevel}
                  disabled={busy}
                  onChange={(event) =>
                    void onChangeSettings({
                      logLevel: event.target.value as LogLevel,
                    })
                  }
                >
                  <option value="error">error</option>
                  <option value="warn">warn</option>
                  <option value="info">info</option>
                  <option value="debug">debug</option>
                  <option value="trace">trace</option>
                </select>
              </label>
              <label className="checkbox-label">
                <input
                  aria-label={t("启用 Mux")}
                  type="checkbox"
                  checked={settings.muxEnabled}
                  disabled={busy}
                  onChange={(event) =>
                    void onChangeSettings({ muxEnabled: event.target.checked })
                  }
                />
                {t("启用 Mux 多路复用（下次连接生效）")}
              </label>
              <p className="hint">
                {t("sing-box 使用 h2mux；Xray 使用 mux。含 Vision flow 的 VLESS 与 Hysteria2 / TUIC 会自动跳过。")}
              </p>
              <label className="checkbox-label">
                <input
                  aria-label={t("启用 Fragment")}
                  type="checkbox"
                  checked={settings.fragmentEnabled}
                  disabled={busy}
                  onChange={(event) =>
                    void onChangeSettings({
                      fragmentEnabled: event.target.checked,
                    })
                  }
                />
                {t("启用 Fragment 反检测（下次连接生效）")}
              </label>
              <p className="hint">
                {t("将 TLS ClientHello 拆分发送以规避基于明文特征的检测；sing-box 使用 TLS fragment/record_fragment，Xray 使用 freedom fragment 出站。仅对含 TLS 握手的节点生效。")}
              </p>
              <label className="checkbox-label">
                <input
                  aria-label={t("启用 Final Fragment")}
                  type="checkbox"
                  checked={settings.finalFragmentEnabled}
                  disabled={busy}
                  onChange={(event) =>
                    void onChangeSettings({
                      finalFragmentEnabled: event.target.checked,
                    })
                  }
                />
                {t("启用 Final Fragment 尾部分片（下次连接生效）")}
              </label>
              <p className="hint">
                {t("在最终落地阶段拆分 TLS 记录；sing-box 使用 route-options tls_record_fragment，Xray 使用 freedom finalmask 包装代理出站。")}
              </p>
              <label className="checkbox-label">
                <input
                  aria-label={t("启用 UDP Noise")}
                  type="checkbox"
                  checked={settings.udpNoiseEnabled}
                  disabled={busy}
                  onChange={(event) =>
                    void onChangeSettings({
                      udpNoiseEnabled: event.target.checked,
                    })
                  }
                />
                {t("启用 UDP Noise 反检测（下次连接生效）")}
              </label>
              <p className="hint">
                {t("在真实 UDP 数据前发送随机噪声包以规避嗅探；仅 Xray 生效（freedom noises），sing-box 无对应能力。默认 length 10-20、delay 10-16。")}
              </p>
              <label className="checkbox-label">
                <input
                  aria-label={t("测速后自动选择最低延迟")}
                  type="checkbox"
                  checked={settings.autoSelectLowestLatency}
                  disabled={busy}
                  onChange={(event) =>
                    void onChangeSettings({
                      autoSelectLowestLatency: event.target.checked,
                    })
                  }
                />
                {t("全部测速后自动选择延迟最低的节点")}
              </label>
              <label>
                {t("URL 测试地址")}
                <input
                  aria-label={t("设置中的 URL 测试地址")}
                  value={settings.urlTestAddress}
                  disabled={busy}
                  onChange={(event) => {
                    const urlTestAddress = event.target.value;
                    app.setUrlTestAddress(urlTestAddress);
                    app.setSettings({ ...settings, urlTestAddress });
                  }}
                  onBlur={() => {
                    void onChangeSettings({
                      urlTestAddress:
                        settings.urlTestAddress.trim() || DEFAULT_URL_TEST_ADDRESS,
                    });
                  }}
                />
              </label>
              <label>
                {t("自动测速间隔（秒）")}
                <input
                  aria-label={t("自动测速间隔（秒）")}
                  type="number"
                  min={10}
                  max={86400}
                  value={settings.urlTestIntervalSeconds}
                  disabled={busy}
                  onChange={(event) =>
                    app.setSettings({
                      ...settings,
                      urlTestIntervalSeconds: Number(event.target.value),
                    })
                  }
                  onBlur={() => {
                    void onChangeSettings({
                      urlTestIntervalSeconds: settings.urlTestIntervalSeconds,
                    });
                  }}
                />
              </label>
              <label>
                {t("切换容差（毫秒）")}
                <input
                  aria-label={t("切换容差（毫秒）")}
                  type="number"
                  min={0}
                  max={5000}
                  value={settings.urlTestToleranceMs}
                  disabled={busy}
                  onChange={(event) =>
                    app.setSettings({
                      ...settings,
                      urlTestToleranceMs: Number(event.target.value),
                    })
                  }
                  onBlur={() => {
                    void onChangeSettings({
                      urlTestToleranceMs: settings.urlTestToleranceMs,
                    });
                  }}
                />
              </label>
              <p className="hint">
                {t("策略组每隔这么久重测一次成员；只有比当前成员快出容差值，才会切换。")}
              </p>
              <label>
                {t("下载测速地址")}
                <input
                  aria-label={t("下载测速地址")}
                  value={settings.speedTestUrl}
                  disabled={busy}
                  onChange={(event) =>
                    app.setSettings({ ...settings, speedTestUrl: event.target.value })
                  }
                  onBlur={() => {
                    void onChangeSettings({
                      speedTestUrl:
                        settings.speedTestUrl.trim() || DEFAULT_SPEED_TEST_URL,
                    });
                  }}
                />
              </label>
              <label className="checkbox-label">
                <input
                  aria-label={t("默认允许不安全证书")}
                  type="checkbox"
                  checked={settings.defAllowInsecure}
                  disabled={busy}
                  onChange={(event) =>
                    void onChangeSettings({
                      defAllowInsecure: event.target.checked,
                    })
                  }
                />
                {t("新建节点默认允许不安全证书")}
              </label>
              <label>
                {t("默认 TLS 指纹")}
                <input
                  aria-label={t("默认 TLS 指纹")}
                  value={settings.defFingerprint}
                  disabled={busy}
                  placeholder="chrome"
                  onChange={(event) =>
                    app.setSettings({
                      ...settings,
                      defFingerprint: event.target.value,
                    })
                  }
                  onBlur={() => {
                    void onChangeSettings({
                      defFingerprint: settings.defFingerprint.trim(),
                    });
                  }}
                />
              </label>
              <p className="hint">
                {t("用于手动创建节点时的 TLS 默认值，不影响已有节点。")}
              </p>
              <label>
                {t("Core 配置模板")}
                <textarea
                  aria-label={t("Core 配置模板")}
                  disabled={busy}
                  rows={6}
                  placeholder={'{"log":{"level":"debug"}}'}
                  value={configTemplate}
                  onChange={(event) => setConfigTemplate(event.target.value)}
                />
              </label>
              <p className="hint">
                {t("以 JSON Merge Patch 的形式叠加在生成的配置之上：可以新增本应用不涉及的字段、改写已生成的字段，或用 null 删除它。留空表示不使用模板。保存时校验，连接时由 Core 再次校验。")}
              </p>
              <button
                type="button"
                disabled={busy}
                onClick={() => void onChangeSettings({ configTemplate })}
              >
                {t("保存配置模板")}
              </button>
            </div>

            <div data-settings-section="network" hidden={hiddenIf("network")}>
              <label className="checkbox-label">
                <input
                  aria-label={t("允许来自局域网的连接")}
                  type="checkbox"
                  checked={settings.allowLan}
                  disabled={busy}
                  onChange={(event) =>
                    void onChangeSettings({ allowLan: event.target.checked })
                  }
                />
                {t("允许来自局域网的连接")}
              </label>
              <p className="hint">
                {t("开启后本地 SOCKS/HTTP 监听 0.0.0.0，局域网设备可使用本机代理；Clash API 仍仅本机可访问。下次连接生效。")}
              </p>
              <label className="checkbox-label">
                <input
                  aria-label={t("启用入站 UDP")}
                  type="checkbox"
                  checked={settings.inboundUdpEnabled}
                  disabled={busy}
                  onChange={(event) =>
                    void onChangeSettings({
                      inboundUdpEnabled: event.target.checked,
                    })
                  }
                />
                {t("启用 SOCKS 入站 UDP（Xray，下次连接生效）")}
              </label>
              <label>
                {t("系统代理")}
                <select
                  aria-label={t("设置系统代理")}
                  disabled={busy}
                  value={settings.systemProxyMode}
                  onChange={(event) =>
                    void onChangeSettings({
                      systemProxyMode: event.target.value as SystemProxyMode,
                    })
                  }
                >
                  <option value="cleared">{t("清除系统代理")}</option>
                  <option value="managed">{t("自动配置系统代理")}</option>
                  <option value="unchanged">{t("不改变系统代理")}</option>
                  <option value="pac">{t("Pac 模式")}</option>
                </select>
              </label>
              <p className="hint">
                {t("PAC 模式会启动本地 PAC 服务并写入系统代理（全局脚本，不等价于规则模式）。")}
              </p>
              <label>
                {t("SOCKS 端口")}
                <input
                  aria-label={t("SOCKS 端口")}
                  type="number"
                  min="1"
                  max="65535"
                  disabled={busy}
                  value={settings.socksPort}
                  onChange={(event) =>
                    void onChangeSettings({
                      socksPort: Number(event.target.value),
                    })
                  }
                />
              </label>
              <label>
                {t("HTTP 端口")}
                <input
                  aria-label={t("HTTP 端口")}
                  type="number"
                  min="1"
                  max="65535"
                  disabled={busy}
                  value={settings.httpPort}
                  onChange={(event) =>
                    void onChangeSettings({
                      httpPort: Number(event.target.value),
                    })
                  }
                />
              </label>
              <label>
                {t("Clash API 端口")}
                <input
                  aria-label={t("Clash API 端口")}
                  type="number"
                  min="1"
                  max="65535"
                  disabled={busy}
                  value={settings.clashApiPort}
                  onChange={(event) =>
                    void onChangeSettings({
                      clashApiPort: Number(event.target.value),
                    })
                  }
                />
              </label>
              <p className="hint" aria-label={t("本地代理端口")}>
                {t("本地代理端口在下次连接时生效；SOCKS、HTTP 与 Clash API 不能相同。")}
              </p>
            </div>

            <div data-settings-section="hotkeys" hidden={hiddenIf("hotkeys")}>
              <label>
                {t("热键：连接/断开")}
                <input
                  aria-label={t("热键：连接/断开")}
                  value={settings.hotkeyConnect}
                  disabled={busy}
                  placeholder="Ctrl+Enter"
                  onChange={(event) =>
                    app.setSettings({
                      ...settings,
                      hotkeyConnect: event.target.value,
                    })
                  }
                  onBlur={() => {
                    void onChangeSettings({
                      hotkeyConnect: settings.hotkeyConnect.trim(),
                    });
                  }}
                />
              </label>
              <label>
                {t("热键：上一节点")}
                <input
                  aria-label={t("热键：上一节点")}
                  value={settings.hotkeyPrevious}
                  disabled={busy}
                  placeholder="Ctrl+["
                  onChange={(event) =>
                    app.setSettings({
                      ...settings,
                      hotkeyPrevious: event.target.value,
                    })
                  }
                  onBlur={() => {
                    void onChangeSettings({
                      hotkeyPrevious: settings.hotkeyPrevious.trim(),
                    });
                  }}
                />
              </label>
              <label>
                {t("热键：下一节点")}
                <input
                  aria-label={t("热键：下一节点")}
                  value={settings.hotkeyNext}
                  disabled={busy}
                  placeholder="Ctrl+]"
                  onChange={(event) =>
                    app.setSettings({
                      ...settings,
                      hotkeyNext: event.target.value,
                    })
                  }
                  onBlur={() => {
                    void onChangeSettings({
                      hotkeyNext: settings.hotkeyNext.trim(),
                    });
                  }}
                />
              </label>
              <p className="hint">
                {t("热键在系统全局生效；窗口未聚焦时也可使用。留空表示禁用。")}
              </p>
              <p className="hint">
                {t("窗口内生效；留空表示禁用。输入框获得焦点时不触发。")}
              </p>
            </div>

            <div data-settings-section="appearance" hidden={hiddenIf("appearance")}>
              <label>
                {t("主题")}
                <select
                  aria-label={t("主题")}
                  value={theme}
                  onChange={(event) =>
                    setTheme(event.target.value as ThemeMode)
                  }
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
                  onChange={(event) =>
                    setFontSize(Number(event.target.value) as FontSize)
                  }
                >
                  {FONT_SIZES.map((size) => (
                    <option key={size} value={size}>
                      {size}
                    </option>
                  ))}
                </select>
              </label>
            </div>

            <div data-settings-section="language" hidden={hiddenIf("language")}>
              <label>
                {t("语言")}
                <select
                  aria-label={t("界面语言")}
                  value={locale}
                  onChange={(event) => {
                    const next = event.target.value as typeof locale;
                    setLocale(next);
                    void onChangeSettings({ locale: next });
                  }}
                >
                  {LOCALES.map((entry) => (
                    <option key={entry.id} value={entry.id}>
                      {entry.label}
                    </option>
                  ))}
                </select>
              </label>
            </div>

            <div data-settings-section="dns" hidden={hiddenIf("dns")}>
              <p className="settings-note">
                <IconInfo /> {t("完整配置在左侧 DNS / 路由页面中管理")}
              </p>
              <button type="button" className="btn-secondary" onClick={() => goTo("dns")}>
                DNS
              </button>
            </div>

            <div data-settings-section="routing" hidden={hiddenIf("routing")}>
              <p className="settings-note">
                <IconInfo /> {t("完整配置在左侧 DNS / 路由页面中管理")}
              </p>
              <button
                type="button"
                className="btn-secondary"
                onClick={() => goTo("routing")}
              >
                {t("路由设置")}
              </button>
            </div>

            <div data-settings-section="data" hidden={hiddenIf("data")}>
              <button type="button" disabled={busy} onClick={() => void onExportPreferences()}>
                {t("导出设置")}
              </button>
              <button type="button" disabled={busy} onClick={() => void onImportPreferences()}>
                {t("导入完整配置")}
              </button>
              <button type="button" disabled={busy} onClick={() => void onExportProfile()}>
                {t("备份到本地")}
              </button>
              <button
                type="button"
                disabled={busy || connected}
                onClick={() => void onImportProfile()}
              >
                {t("从本地还原")}
              </button>
              <button type="button" disabled={busy} onClick={() => void onClearTraffic()}>
                {t("清除流量统计")}
              </button>
            </div>

            <div data-settings-section="updates" hidden={hiddenIf("updates")}>
              <button type="button" disabled={busy} onClick={() => void onCheckUpdate()}>
                {t("检查更新")}
              </button>
              <button type="button" disabled={busy} onClick={() => void onCheckCoreUpdate()}>
                {t("检查 Core 更新")}
              </button>
              <button type="button" disabled={busy} onClick={() => void app.onOpenGeo()}>
                {t("更新 Geo 文件")}
              </button>
            </div>

            <div data-settings-section="advanced" hidden={hiddenIf("advanced")}>
              <p className="settings-note">
                <IconInfo />{" "}
                {t("界面不会自行读取或生成内核配置，所有操作仍通过现有的 Tauri 命令完成")}
              </p>
              <button
                type="button"
                disabled={busy}
                onClick={() => {
                  setDialog("config");
                  setCoreConfig("");
                  void previewCoreConfig().then(setCoreConfig, (failure: unknown) =>
                    setError(describeFailure(failure)),
                  );
                }}
              >
                {t("查看配置")}
              </button>
              <p className="hint">
                {t("构建目标")}：{platform ? platform.artifactIdentifier : platformError}
              </p>
              {coreConfig ? (
                <textarea
                  aria-label={t("生成的 Core 配置")}
                  rows={8}
                  value={coreConfig}
                  onChange={(event) => setCoreConfig(event.target.value)}
                />
              ) : null}
            </div>

            <div data-settings-section="about" hidden={hiddenIf("about")}>
              <Card>
                <p>MgClash</p>
                <p>{platform?.artifactIdentifier}</p>
                <p>
                  {t("界面以 v2rayN Avalonia 为模板重新实现，未使用其 GPL 源码。")}
                </p>
                <p>
                  {t("关闭窗口时最小化到托盘")}:{" "}
                  {settings.closeToTray ? t("已启用") : t("未启用")}
                </p>
                <p className="hint">
                  {t("这是未签名版本：macOS Gatekeeper 与 Windows SmartScreen 会在首次打开时提示，需要你手动确认后才能运行。")}
                </p>
                <p className="hint">
                  {t("托盘菜单支持打开窗口、连接/断开、切换路由模式与选择节点。")}
                </p>
              </Card>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}