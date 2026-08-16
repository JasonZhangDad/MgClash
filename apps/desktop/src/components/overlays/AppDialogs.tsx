import {
  blankManualNodeForm,
  SHADOWSOCKS_METHODS,
  usesStreamTransport,
  type GrpcMode,
  type KcpHeaderType,
  type ManualNodeForm,
  type TransportKind,
  type XhttpMode,
} from "../../manualNode";
import { CameraScanner } from "../CameraScanner";
import { Dialog } from "../Dialog";
import {
  formatBytes,
  formatClock,
  describeRuleSetCache,
  isGeoRule,
  runtimeOrderedRoute,
  ROUTE_KIND_LABEL,
  ROUTE_OUTBOUND_LABEL,
  TUN_LABEL,
  TUN_NOTICE,
  DEFAULT_SPEED_TEST_URL,
  DEFAULT_URL_TEST_ADDRESS,
} from "../../appHelpers";
import {
  createRouteScheme,
  deleteRouteScheme,
  loadNodeDraft,
  previewCoreConfig,
  readQrCode,
  setRouteScheme,
  type CorePreference,
  type DnsMode,
  type DnsStrategy,
  type DnsTemplate,
  type LogLevel,
  type NodeGroupStrategy,
  type ObfuscationMethod,
  type ProxyProtocol,
  type RouteOutbound,
  type RouteRuleKind,
  type RuleProviderFormat,
  type VmessSecurity,
} from "../../session";
import type { AppModel } from "../../hooks/useAppController";
import { describeFailure } from "../../appHelpers";

export function AppDialogs({ app }: { app: AppModel }) {
  const {
    bulkReport,
    bulkText,
    busy,
    configTemplate,
    connected,
    coreConfig,
    coreUpdate,
    createForm,
    dialog,
    dnsDirty,
    dnsDraft,
    dnsTab,
    editingNodeId,
    editingSubscriptionId,
    fontSize,
    formatCoreVersionLine,
    formatInstalledCoreLine,
    geoStatus,
    locale,
    nodeGroupName,
    nodeGroupStrategy,
    nodeGroups,
    nodeTestInProgress,
    nodes,
    onAddRouteRule,
    onAddRuleProvider,
    onChangeSettings,
    onCreateNode,
    onDeleteSubscription,
    onDownloadCore,
    onEditSubscription,
    onExportProfile,
    onImport,
    onImportClipboard,
    onImportFile,
    onImportProfile,
    onImportQrCode,
    onMoveRouteRule,
    onOpenRouting,
    onRefreshAllSubscriptions,
    onRefreshSubscription,
    onSaveDns,
    onSaveRoute,
    onSaveSubscription,
    onUpdateGeo,
    onUpdateRuleSet,
    onUpdateRuleSets,
    platform,
    providerFormat,
    providerName,
    providerOutbound,
    providerUrl,
    qrCode,
    resetNodeForm,
    resetSubscriptionForm,
    routeDirty,
    routeDraft,
    routeRuleKind,
    routeRuleOutbound,
    routeRuleValue,
    ruleSets,
    runBulkImport,
    setBusy,
    setBulkText,
    setConfigTemplate,
    setCoreConfig,
    setCoreUpdate,
    setCreateForm,
    setDialog,
    setDnsDirty,
    setDnsDraft,
    setDnsTab,
    setError,
    setProviderFormat,
    setProviderName,
    setProviderOutbound,
    setProviderUrl,
    setQrCode,
    setRouteDirty,
    setRouteDraft,
    setRouteRuleKind,
    setRouteRuleOutbound,
    setRouteRuleValue,
    setSettings,
    setStatus,
    setSubscriptionAutoUpdate,
    setSubscriptionEnabled,
    setSubscriptionExclude,
    setSubscriptionInclude,
    setSubscriptionInterval,
    setSubscriptionName,
    setSubscriptionSubconverter,
    setSubscriptionUrl,
    setSubscriptionUserAgent,
    setUpdate,
    setUri,
    settings,
    settingsTab,
    setSettingsTab,
    status,
    subscriptionAutoUpdate,
    subscriptionEnabled,
    subscriptionExclude,
    subscriptionInclude,
    subscriptionInterval,
    subscriptionName,
    subscriptionSubconverter,
    subscriptionUrl,
    subscriptionUserAgent,
    subscriptions,
    t,
    update,
    updateCreateForm,
    uri,
    urlTestAddress,
    setUrlTestAddress,
    platformError,
  } = app;

  void setSettings;
  void fontSize;
  void locale;

  return (
    <>
      {update !== null && (
          <div className="dialog-backdrop" onClick={() => setUpdate(null)}>
            <div
              className="dialog qr-dialog"
              role="dialog"
              aria-label={t("检查更新结果")}
              onClick={(event) => event.stopPropagation()}
            >
              <header className="dialog-head">
                <strong>{t("检查更新")}</strong>
                <button type="button" onClick={() => setUpdate(null)}>
                  {t("关闭")}
                </button>
              </header>
              <ul className="update-list">
                <li>
                  <strong>MgClash</strong>
                  <span>
                    {update.updateAvailable
                      ? `有新版本 ${update.latest}，当前 ${update.current}`
                      : `已是最新版本 ${update.current}`}
                  </span>
                  <p className="hint">{update.url}</p>
                </li>
              </ul>
            </div>
          </div>
        )}
      {coreUpdate !== null && (
          <div className="dialog-backdrop" onClick={() => setCoreUpdate(null)}>
            <div
              className="dialog qr-dialog"
              role="dialog"
              aria-label={t("检查 Core 更新结果")}
              onClick={(event) => event.stopPropagation()}
            >
              <header className="dialog-head">
                <strong>{t("检查 Core 更新")}</strong>
                <button type="button" onClick={() => setCoreUpdate(null)}>
                  {t("关闭")}
                </button>
              </header>
              <ul className="update-list">
                <li>
                  <strong>sing-box</strong>
                  <span>{formatCoreVersionLine(coreUpdate.singBox)}</span>
                  <p className="hint">{formatInstalledCoreLine("sing-box", coreUpdate.install.singBox)}</p>
                  <p className="hint">{coreUpdate.singBox.url}</p>
                </li>
                <li>
                  <strong>Xray</strong>
                  <span>{formatCoreVersionLine(coreUpdate.xray)}</span>
                  <p className="hint">{formatInstalledCoreLine("Xray", coreUpdate.install.xray)}</p>
                  <p className="hint">{coreUpdate.xray.url}</p>
                </li>
              </ul>
              <p className="hint">
                {t("Core 更新说明")}
              </p>
              <div className="actions">
                <button
                  type="button"
                  disabled={busy || connected || !coreUpdate.singBox.updateAvailable}
                  onClick={() => void onDownloadCore("sing-box")}
                >
                  {t("下载 sing-box")}
                </button>
                <button
                  type="button"
                  disabled={busy || connected || !coreUpdate.xray.updateAvailable}
                  onClick={() => void onDownloadCore("xray")}
                >
                  {t("下载 Xray")}
                </button>
              </div>
              {connected ? (
                <p className="hint">{t("请先断开连接再更新 Core")}</p>
              ) : null}
            </div>
          </div>
        )}
        {qrCode !== null && (
          <div className="dialog-backdrop" onClick={() => setQrCode(null)}>
            <div
              className="dialog qr-dialog"
              role="dialog"
              aria-label={`二维码 ${qrCode.name}`}
              onClick={(event) => event.stopPropagation()}
            >
              <header className="dialog-head">
                <strong>{qrCode.name}</strong>
                <button type="button" onClick={() => setQrCode(null)}>
                  {t("关闭")}
                </button>
              </header>
              {/* The markup comes from the Rust renderer, never from a node's
                  own fields, so there is nothing here a node name could inject. */}
              <div
                className="qr-code"
                dangerouslySetInnerHTML={{ __html: qrCode.svg }}
              />
              <p className="hint">
                {t("扫描即导入该节点。二维码包含凭据，请勿分享给他人。")}
              </p>
            </div>
          </div>
        )}
      <Dialog
        hidden={dialog !== "import"}
        title={t("导入节点")}
        ariaLabel={t("导入节点")}
        onClose={() => setDialog(null)}
        wide
      >
        <h2>{t("导入节点")}</h2>

        <textarea
          aria-label={t("分享链接")}
          rows={3}
          value={uri}
          disabled={busy}
          placeholder="vless:// vmess:// trojan:// ss:// hysteria2:// tuic://"
          onChange={(event) => setUri(event.target.value)}
        />

        <div className="actions">
          <button
            type="button"
            disabled={busy}
            onClick={() => void onImport()}
          >
            {t("导入")}
          </button>
        </div>

        <h2>{t("批量导入")}</h2>

        <p className="hint">
          {t("可粘贴多行分享链接，或整体 Base64 的订阅正文。批量导入不会改变当前选中的节点。")}
        </p>

        <textarea
          aria-label={t("批量节点列表")}
          rows={4}
          value={bulkText}
          disabled={busy}
          placeholder={t("每行一个链接，或粘贴 Base64 订阅正文")}
          onChange={(event) => setBulkText(event.target.value)}
        />

        <div className="actions">
          <button
            type="button"
            disabled={busy}
            onClick={() => void runBulkImport(bulkText)}
          >
            {t("批量导入")}
          </button>
          <label className="file-import">
            {t("从文件导入")}
            <input
              aria-label={t("从文件导入节点")}
              type="file"
              accept=".txt,.text,text/plain"
              disabled={busy}
              onChange={(event) => {
                const file = event.target.files?.[0];
                // Clear the value so picking the same file twice still fires.
                event.target.value = "";
                void onImportFile(file);
              }}
            />
          </label>
          <CameraScanner
            busy={busy}
            t={t}
            decode={readQrCode}
            onLink={(link) => void runBulkImport(link)}
            onError={setError}
          />
          <label className="file-import">
            {t("扫描二维码图片")}
            <input
              aria-label={t("从二维码图片导入节点")}
              type="file"
              accept="image/png,image/jpeg"
              disabled={busy}
              onChange={(event) => {
                const file = event.target.files?.[0];
                event.target.value = "";
                void onImportQrCode(file);
              }}
            />
          </label>
        </div>

        {bulkReport !== null && (
          <div className="bulk-report" role="status" aria-label={t("批量导入结果")}>
            <p>
              成功导入 {bulkReport.imported} 个
              {bulkReport.duplicates > 0 &&
                `，跳过 ${bulkReport.duplicates} 个重复`}
              {bulkReport.failures.length > 0 &&
                `，${bulkReport.failures.length} 行失败`}
            </p>
            {bulkReport.failures.length > 0 && (
              <ul>
                {bulkReport.failures.map((failure, index) => (
                  <li key={`${failure.line ?? "none"}-${index}`}>
                    {failure.line === null
                      ? failure.message
                      : `第 ${failure.line} 行：${failure.message}`}
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}

      </Dialog>

      <Dialog
        hidden={dialog !== "create"}
        title={editingNodeId === null ? t("添加节点") : t("编辑节点")}
        ariaLabel={editingNodeId === null ? t("手动创建节点") : t("编辑节点")}
        onClose={() => {
          setDialog(null);
          resetNodeForm();
        }}
        onConfirm={() => void onCreateNode()}
        wide
      >
        <div
          className="settings-form form-grid node-form"
          aria-label={editingNodeId === null ? t("手动创建节点") : t("编辑节点")}
        >
          <p className="form-section">{t("按协议只显示相关字段")}</p>
          <label className="sr-only">
            {t("协议")}
            <select
              aria-label={t("节点协议")}
              value={createForm.protocol}
              disabled={busy}
              onChange={(event) =>
                updateCreateForm({
                  protocol: event.target.value as ProxyProtocol,
                })
              }
            >
              <option value="vless">VLESS</option>
              <option value="vmess">VMess</option>
              <option value="trojan">Trojan</option>
              <option value="shadowsocks">Shadowsocks</option>
              <option value="socks">SOCKS5</option>
              <option value="http">HTTP</option>
              <option value="hysteria2">Hysteria2</option>
              <option value="tuic">TUIC</option>
              <option value="wireguard">WireGuard</option>
              <option value="anytls">AnyTLS</option>
              <option value="naive">Naive</option>
              <option value="custom">Custom</option>
            </select>
          </label>
          <div className="protocol-chips" role="group" aria-label={t("协议")}>
            {(
              [
                ["vless", "VLESS"],
                ["vmess", "VMess"],
                ["shadowsocks", "Shadowsocks"],
                ["trojan", "Trojan"],
                ["hysteria2", "Hysteria2"],
                ["tuic", "TUIC"],
                ["wireguard", "WireGuard"],
                ["anytls", "AnyTLS"],
                ["naive", "Naive"],
                ["socks", "SOCKS"],
                ["http", "HTTP"],
                ["custom", t("自定义")],
              ] as const
            ).map(([value, label]) => (
              <button
                key={value}
                type="button"
                className={createForm.protocol === value ? "chip is-on" : "chip"}
                disabled={busy}
                onClick={() =>
                  updateCreateForm({
                    protocol: value,
                    ...(value === "anytls" || value === "naive"
                      ? { tlsEnabled: true }
                      : {}),
                  })
                }
              >
                {label}
              </button>
            ))}
          </div>

          <label>
            {t("别名")}
            <input
              aria-label={t("新建节点名称")}
              value={createForm.name}
              disabled={busy}
              onChange={(event) => updateCreateForm({ name: event.target.value })}
            />
          </label>

          {createForm.protocol !== "custom" && (
            <>
          <label>
            {t("地址")}
            <input
              aria-label={t("新建节点服务器")}
              value={createForm.server}
              disabled={busy}
              onChange={(event) =>
                updateCreateForm({ server: event.target.value })
              }
            />
          </label>

          <label>
            {t("端口")}
            <input
              aria-label={t("新建节点端口")}
              inputMode="numeric"
              value={createForm.port}
              disabled={busy}
              onChange={(event) => updateCreateForm({ port: event.target.value })}
            />
          </label>
            </>
          )}

          {createForm.protocol === "custom" && (
            <>
              <p className="form-hint">
                {t("自定义节点使用完整 Core JSON，忽略 Mux、Fragment、DNS 与路由设置；请自行保证 SOCKS/HTTP 端口与设置页一致。")}
              </p>
              <label>
                {t("Core 类型")}
                <select
                  aria-label={t("自定义 Core 类型")}
                  value={createForm.customCore}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({
                      customCore: event.target.value as "sing-box" | "xray",
                    })
                  }
                >
                  <option value="sing-box">sing-box</option>
                  <option value="xray">Xray</option>
                </select>
              </label>
              <label>
                {t("Core JSON 配置")}
                <textarea
                  aria-label={t("Core JSON 配置")}
                  rows={16}
                  spellCheck={false}
                  value={createForm.customDocument}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({ customDocument: event.target.value })
                  }
                />
              </label>
            </>
          )}

          {(createForm.protocol === "vless" ||
            createForm.protocol === "vmess" ||
            createForm.protocol === "tuic") && (
            <label>
              UUID
              <span className="inline-field">
                <input
                  aria-label={t("节点 UUID")}
                  value={createForm.userId}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({ userId: event.target.value })
                  }
                />
                <button
                  type="button"
                  disabled={busy}
                  onClick={() =>
                    updateCreateForm({ userId: crypto.randomUUID() })
                  }
                >
                  {t("生成")}
                </button>
              </span>
            </label>
          )}

          {createForm.protocol === "vless" && (
            <label>
              flow
              <input
                aria-label="VLESS flow"
                placeholder={t("留空表示不使用")}
                value={createForm.flow}
                disabled={busy}
                onChange={(event) =>
                  updateCreateForm({ flow: event.target.value })
                }
              />
            </label>
          )}

          {createForm.protocol === "vmess" && (
            <>
              <label>
                {t("加密方式")}
                <select
                  aria-label={t("VMess 加密方式")}
                  value={createForm.security}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({
                      security: event.target.value as VmessSecurity,
                    })
                  }
                >
                  <option value="Auto">auto</option>
                  <option value="Aes128Gcm">aes-128-gcm</option>
                  <option value="Chacha20Poly1305">chacha20-poly1305</option>
                  <option value="None">none</option>
                  <option value="Zero">zero</option>
                </select>
              </label>
              <label>
                alterId
                <input
                  aria-label="VMess alterId"
                  inputMode="numeric"
                  value={createForm.alterId}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({ alterId: event.target.value })
                  }
                />
              </label>
            </>
          )}

          {createForm.protocol === "shadowsocks" && (
            <label>
              {t("加密方式")}
              <select
                aria-label={t("Shadowsocks 加密方式")}
                value={createForm.method}
                disabled={busy}
                onChange={(event) =>
                  updateCreateForm({ method: event.target.value })
                }
              >
                {SHADOWSOCKS_METHODS.map((method) => (
                  <option key={method} value={method}>
                    {method}
                  </option>
                ))}
              </select>
            </label>
          )}

          {(createForm.protocol === "socks" ||
            createForm.protocol === "http" ||
            createForm.protocol === "naive") && (
            <label>
              {t("用户名")}
              <input
                aria-label={t("代理用户名")}
                placeholder={t("留空表示不使用")}
                value={createForm.username}
                disabled={busy}
                onChange={(event) =>
                  updateCreateForm({ username: event.target.value })
                }
              />
            </label>
          )}

          {(createForm.protocol === "trojan" ||
            createForm.protocol === "shadowsocks" ||
            createForm.protocol === "socks" ||
            createForm.protocol === "http" ||
            createForm.protocol === "anytls" ||
            createForm.protocol === "naive") && (
            <label>
              {t("密码")}
              <input
                aria-label={
                  createForm.protocol === "anytls"
                    ? t("AnyTLS 密码")
                    : t("节点密码")
                }
                type="password"
                placeholder={
                  createForm.protocol === "socks" ||
                  createForm.protocol === "http" ||
                  createForm.protocol === "naive"
                    ? t("留空表示不使用")
                    : undefined
                }
                value={createForm.password}
                disabled={busy}
                onChange={(event) =>
                  updateCreateForm({ password: event.target.value })
                }
              />
            </label>
          )}

          {createForm.protocol === "naive" && (
            <>
              <label>
                <input
                  aria-label={t("启用 QUIC")}
                  type="checkbox"
                  checked={createForm.quic}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({ quic: event.target.checked })
                  }
                />
                {t("启用 QUIC")}
              </label>
              {createForm.quic && (
                <label>
                  {t("拥塞控制")}
                  <select
                    aria-label={t("Naive 拥塞控制")}
                    value={createForm.quicCongestionControl}
                    disabled={busy}
                    onChange={(event) =>
                      updateCreateForm({
                        quicCongestionControl: event.target
                          .value as ManualNodeForm["quicCongestionControl"],
                      })
                    }
                  >
                    <option value="">{t("默认")}</option>
                    <option value="bbr">bbr</option>
                    <option value="bbr2">bbr2</option>
                    <option value="cubic">cubic</option>
                    <option value="reno">reno</option>
                  </select>
                </label>
              )}
            </>
          )}

          {createForm.protocol === "hysteria2" && (
            <>
              <label>
                {t("认证密码")}
                <input
                  aria-label={t("Hysteria2 认证密码")}
                  type="password"
                  placeholder={t("留空表示不使用")}
                  value={createForm.authentication}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({ authentication: event.target.value })
                  }
                />
              </label>
              <label>
                <input
                  aria-label={t("启用混淆")}
                  type="checkbox"
                  checked={createForm.obfsEnabled}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({ obfsEnabled: event.target.checked })
                  }
                />
                {t("启用混淆")}
              </label>
              {createForm.obfsEnabled && (
                <>
                  <label>
                    {t("混淆方式")}
                    <select
                      aria-label={t("混淆方式")}
                      value={createForm.obfsMethod}
                      disabled={busy}
                      onChange={(event) =>
                        updateCreateForm({
                          obfsMethod: event.target.value as ObfuscationMethod,
                        })
                      }
                    >
                      <option value="Salamander">salamander</option>
                      <option value="Gecko">gecko</option>
                    </select>
                  </label>
                  <label>
                    {t("混淆密码")}
                    <input
                      aria-label={t("混淆密码")}
                      type="password"
                      value={createForm.obfsPassword}
                      disabled={busy}
                      onChange={(event) =>
                        updateCreateForm({ obfsPassword: event.target.value })
                      }
                    />
                  </label>
                </>
              )}
            </>
          )}

          {createForm.protocol === "tuic" && (
            <>
              <label>
                {t("TUIC 密码")}
                <input
                  aria-label={t("TUIC 密码")}
                  type="password"
                  placeholder={t("留空表示不使用")}
                  value={createForm.password}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({ password: event.target.value })
                  }
                />
              </label>
              <label>
                {t("拥塞控制")}
                <select
                  aria-label={t("拥塞控制")}
                  value={createForm.congestionControl}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({
                      congestionControl: event.target.value as
                        | ""
                        | "cubic"
                        | "new_reno"
                        | "bbr",
                    })
                  }
                >
                  <option value="">{t("默认")}</option>
                  <option value="cubic">cubic</option>
                  <option value="new_reno">new_reno</option>
                  <option value="bbr">bbr</option>
                </select>
              </label>
              <label>
                {t("UDP 中继模式")}
                <select
                  aria-label={t("UDP 中继模式")}
                  value={createForm.udpRelayMode}
                  disabled={busy || createForm.udpOverStream}
                  onChange={(event) =>
                    updateCreateForm({
                      udpRelayMode: event.target.value as
                        | ""
                        | "native"
                        | "quic",
                    })
                  }
                >
                  <option value="">{t("默认")}</option>
                  <option value="native">native</option>
                  <option value="quic">quic</option>
                </select>
              </label>
              <label className="checkbox-label">
                <input
                  aria-label={t("UDP over stream")}
                  type="checkbox"
                  checked={createForm.udpOverStream}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({
                      udpOverStream: event.target.checked,
                      udpRelayMode: event.target.checked
                        ? ""
                        : createForm.udpRelayMode,
                    })
                  }
                />
                UDP over stream
              </label>
              <label className="checkbox-label">
                <input
                  aria-label={t("0-RTT 握手")}
                  type="checkbox"
                  checked={createForm.zeroRttHandshake}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({
                      zeroRttHandshake: event.target.checked,
                    })
                  }
                />
                {t("0-RTT 握手")}
              </label>
            </>
          )}

          {createForm.protocol === "wireguard" && (
            <>
              <label>
                {t("私钥")}
                <input
                  aria-label={t("WireGuard 私钥")}
                  type="password"
                  value={createForm.privateKey}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({ privateKey: event.target.value })
                  }
                />
              </label>
              <label>
                {t("对端公钥")}
                <input
                  aria-label={t("WireGuard 对端公钥")}
                  value={createForm.peerPublicKey}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({ peerPublicKey: event.target.value })
                  }
                />
              </label>
              <label>
                {t("预共享密钥")}
                <input
                  aria-label={t("WireGuard 预共享密钥")}
                  type="password"
                  placeholder={t("留空表示不使用")}
                  value={createForm.preSharedKey}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({ preSharedKey: event.target.value })
                  }
                />
              </label>
              <label>
                {t("本地地址")}
                <input
                  aria-label={t("WireGuard 本地地址")}
                  placeholder={t("逗号分隔，如 10.0.0.2/32")}
                  value={createForm.localAddress}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({ localAddress: event.target.value })
                  }
                />
              </label>
              <label>
                MTU
                <input
                  aria-label="WireGuard MTU"
                  inputMode="numeric"
                  placeholder={t("留空表示不使用")}
                  value={createForm.mtu}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({ mtu: event.target.value })
                  }
                />
              </label>
              <label>
                Reserved
                <input
                  aria-label="WireGuard reserved"
                  placeholder={t("留空表示不使用，如 1,2,3")}
                  value={createForm.reserved}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({ reserved: event.target.value })
                  }
                />
              </label>
            </>
          )}

          {usesStreamTransport(createForm.protocol) && (
            <>
            <hr />
            <p className="form-section">{t("底层传输方式")}</p>
            <label>
              {t("传输方式")}
              <select
                aria-label={t("传输方式")}
                value={createForm.transport}
                disabled={
                  busy ||
                  connected ||
                  createForm.protocol === "shadowsocks" ||
                  createForm.protocol === "socks" ||
                  createForm.protocol === "http"
                }
                onChange={(event) =>
                  updateCreateForm({
                    transport: event.target.value as TransportKind,
                  })
                }
              >
                <option value="tcp">TCP</option>
                <option value="websocket">WebSocket</option>
                <option value="httpupgrade">HTTPUpgrade</option>
                <option value="xhttp">XHTTP</option>
                <option value="grpc">gRPC</option>
                <option value="kcp">mKCP</option>
              </select>
            </label>
            </>
          )}

          {usesStreamTransport(createForm.protocol) &&
            (createForm.transport === "websocket" ||
              createForm.transport === "httpupgrade" ||
              createForm.transport === "xhttp") && (
              <>
                <label>
                  {t("路径")}
                  <input
                    aria-label={
                      createForm.transport === "httpupgrade"
                        ? t("HTTPUpgrade 路径")
                        : createForm.transport === "xhttp"
                          ? t("XHTTP 路径")
                          : t("WebSocket 路径")
                    }
                    value={createForm.wsPath}
                    disabled={busy}
                    onChange={(event) =>
                      updateCreateForm({ wsPath: event.target.value })
                    }
                  />
                </label>
                <label>
                  Host
                  <input
                    aria-label={
                      createForm.transport === "httpupgrade"
                        ? "HTTPUpgrade Host"
                        : createForm.transport === "xhttp"
                          ? "XHTTP Host"
                          : "WebSocket Host"
                    }
                    placeholder={t("留空表示不使用")}
                    value={createForm.wsHost}
                    disabled={busy}
                    onChange={(event) =>
                      updateCreateForm({ wsHost: event.target.value })
                    }
                  />
                </label>
              </>
            )}

          {usesStreamTransport(createForm.protocol) &&
            createForm.transport === "xhttp" && (
              <label>
                {t("模式")}
                <select
                  aria-label={t("XHTTP 模式")}
                  value={createForm.xhttpMode}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({
                      xhttpMode: event.target.value as XhttpMode,
                    })
                  }
                >
                  <option value="auto">auto</option>
                  <option value="packet-up">packet-up</option>
                  <option value="stream-up">stream-up</option>
                  <option value="stream-one">stream-one</option>
                </select>
              </label>
            )}

          {usesStreamTransport(createForm.protocol) &&
            createForm.transport === "grpc" && (
              <>
                <label>
                  serviceName
                  <input
                    aria-label="gRPC serviceName"
                    value={createForm.grpcServiceName}
                    disabled={busy}
                    onChange={(event) =>
                      updateCreateForm({ grpcServiceName: event.target.value })
                    }
                  />
                </label>
                <label>
                  {t("模式")}
                  <select
                    aria-label={t("gRPC 模式")}
                    value={createForm.grpcMode}
                    disabled={busy}
                    onChange={(event) =>
                      updateCreateForm({
                        grpcMode: event.target.value as GrpcMode,
                      })
                    }
                  >
                    <option value="gun">gun</option>
                    <option value="multi">multi</option>
                    <option value="guna">guna</option>
                  </select>
                </label>
                <label>
                  authority
                  <input
                    aria-label="gRPC authority"
                    placeholder={t("留空表示不使用")}
                    value={createForm.grpcAuthority}
                    disabled={busy}
                    onChange={(event) =>
                      updateCreateForm({ grpcAuthority: event.target.value })
                    }
                  />
                </label>
              </>
            )}

          {usesStreamTransport(createForm.protocol) &&
            createForm.transport === "kcp" && (
              <>
                <label>
                  mtu
                  <input
                    aria-label={t("mKCP mtu")}
                    placeholder={t("留空表示使用默认")}
                    value={createForm.kcpMtu}
                    disabled={busy}
                    onChange={(event) =>
                      updateCreateForm({ kcpMtu: event.target.value })
                    }
                  />
                </label>
                <label>
                  tti
                  <input
                    aria-label={t("mKCP tti")}
                    placeholder={t("留空表示使用默认")}
                    value={createForm.kcpTti}
                    disabled={busy}
                    onChange={(event) =>
                      updateCreateForm({ kcpTti: event.target.value })
                    }
                  />
                </label>
                <label>
                  uplinkCapacity
                  <input
                    aria-label={t("mKCP uplinkCapacity")}
                    placeholder={t("留空表示使用默认")}
                    value={createForm.kcpUplinkCapacity}
                    disabled={busy}
                    onChange={(event) =>
                      updateCreateForm({ kcpUplinkCapacity: event.target.value })
                    }
                  />
                </label>
                <label>
                  downlinkCapacity
                  <input
                    aria-label={t("mKCP downlinkCapacity")}
                    placeholder={t("留空表示使用默认")}
                    value={createForm.kcpDownlinkCapacity}
                    disabled={busy}
                    onChange={(event) =>
                      updateCreateForm({
                        kcpDownlinkCapacity: event.target.value,
                      })
                    }
                  />
                </label>
                <label>
                  {t("伪装类型")}
                  <select
                    aria-label={t("mKCP 伪装类型")}
                    value={createForm.kcpHeaderType}
                    disabled={busy}
                    onChange={(event) =>
                      updateCreateForm({
                        kcpHeaderType: event.target.value as KcpHeaderType,
                      })
                    }
                  >
                    <option value="">{t("默认")}</option>
                    <option value="none">none</option>
                    <option value="srtp">srtp</option>
                    <option value="utp">utp</option>
                    <option value="wechat-video">wechat-video</option>
                    <option value="dtls">dtls</option>
                    <option value="wireguard">wireguard</option>
                  </select>
                </label>
                <label>
                  seed
                  <input
                    aria-label={t("mKCP seed")}
                    placeholder={t("留空表示不使用")}
                    value={createForm.kcpSeed}
                    disabled={busy}
                    onChange={(event) =>
                      updateCreateForm({ kcpSeed: event.target.value })
                    }
                  />
                </label>
                <label>
                  <input
                    aria-label={t("mKCP 拥塞控制")}
                    type="checkbox"
                    checked={createForm.kcpCongestion}
                    disabled={busy}
                    onChange={(event) =>
                      updateCreateForm({ kcpCongestion: event.target.checked })
                    }
                  />
                  {t("启用拥塞控制")}
                </label>
              </>
            )}

          {usesStreamTransport(createForm.protocol) &&
            createForm.protocol !== "shadowsocks" &&
            createForm.protocol !== "socks" && (
              <>
                <hr />
                <p className="form-section">TLS</p>
                <label>
                  <input
                    aria-label={t("启用 TLS")}
                    type="checkbox"
                    checked={createForm.tlsEnabled || createForm.realityEnabled}
                    disabled={busy}
                    onChange={(event) =>
                      updateCreateForm({
                        tlsEnabled: event.target.checked,
                        realityEnabled: event.target.checked
                          ? createForm.realityEnabled
                          : false,
                      })
                    }
                  />
                  {t("启用 TLS")}
                </label>
                {createForm.protocol !== "http" && (
                  <label>
                    <input
                      aria-label={t("启用 Reality")}
                      type="checkbox"
                      checked={createForm.realityEnabled}
                      disabled={busy}
                      onChange={(event) =>
                        updateCreateForm({
                          realityEnabled: event.target.checked,
                          tlsEnabled:
                            event.target.checked || createForm.tlsEnabled,
                        })
                      }
                    />
                    {t("启用 Reality")}
                  </label>
                )}
              </>
            )}

          {createForm.protocol === "anytls" && (
            <>
              <label>
                <input
                  aria-label={t("启用 TLS")}
                  type="checkbox"
                  checked={createForm.tlsEnabled || createForm.realityEnabled}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({
                      tlsEnabled: event.target.checked,
                      realityEnabled: event.target.checked
                        ? createForm.realityEnabled
                        : false,
                    })
                  }
                />
                {t("启用 TLS")}
              </label>
              <label>
                <input
                  aria-label={t("启用 Reality")}
                  type="checkbox"
                  checked={createForm.realityEnabled}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({
                      realityEnabled: event.target.checked,
                      tlsEnabled: event.target.checked || createForm.tlsEnabled,
                    })
                  }
                />
                {t("启用 Reality")}
              </label>
            </>
          )}

          {(createForm.tlsEnabled ||
            createForm.realityEnabled ||
            createForm.protocol === "hysteria2" ||
            createForm.protocol === "tuic" ||
            createForm.protocol === "anytls" ||
            createForm.protocol === "naive") &&
            createForm.protocol !== "shadowsocks" &&
            createForm.protocol !== "socks" && (
              <>
                <label>
                  SNI
                  <input
                    aria-label="TLS SNI"
                    placeholder={t("留空表示使用服务器地址")}
                    value={createForm.serverName}
                    disabled={busy}
                    onChange={(event) =>
                      updateCreateForm({ serverName: event.target.value })
                    }
                  />
                </label>
                {createForm.protocol !== "naive" && (
                  <>
                <label>
                  ALPN
                  <input
                    aria-label="TLS ALPN"
                    placeholder={t("逗号分隔，如 h2,http/1.1")}
                    value={createForm.alpn}
                    disabled={busy}
                    onChange={(event) =>
                      updateCreateForm({ alpn: event.target.value })
                    }
                  />
                </label>
                <label>
                  {t("指纹")}
                  <input
                    aria-label={t("TLS 指纹")}
                    placeholder={t("留空表示不使用")}
                    value={createForm.fingerprint}
                    disabled={busy}
                    onChange={(event) =>
                      updateCreateForm({ fingerprint: event.target.value })
                    }
                  />
                </label>
                {!createForm.realityEnabled && (
                  <label>
                    pinSHA256
                    <input
                      aria-label={t("证书固定 pinSHA256")}
                      placeholder={t("留空表示不使用")}
                      value={createForm.pinnedSha256}
                      disabled={busy}
                      onChange={(event) =>
                        updateCreateForm({ pinnedSha256: event.target.value })
                      }
                    />
                  </label>
                )}
                {createForm.realityEnabled ? (
                  <>
                    <label>
                      publicKey
                      <input
                        aria-label={t("Reality publicKey")}
                        value={createForm.publicKey}
                        disabled={busy}
                        onChange={(event) =>
                          updateCreateForm({ publicKey: event.target.value })
                        }
                      />
                    </label>
                    <label>
                      shortId
                      <input
                        aria-label={t("Reality shortId")}
                        placeholder={t("留空表示不使用")}
                        value={createForm.shortId}
                        disabled={busy}
                        onChange={(event) =>
                          updateCreateForm({ shortId: event.target.value })
                        }
                      />
                    </label>
                    <label>
                      spiderX
                      <input
                        aria-label={t("Reality spiderX")}
                        placeholder={t("留空表示不使用")}
                        value={createForm.spiderX}
                        disabled={busy}
                        onChange={(event) =>
                          updateCreateForm({ spiderX: event.target.value })
                        }
                      />
                    </label>
                  </>
                ) : (
                  <label>
                    <input
                      aria-label={t("允许不安全证书")}
                      type="checkbox"
                      checked={createForm.allowInsecure}
                      disabled={busy}
                      onChange={(event) =>
                        updateCreateForm({ allowInsecure: event.target.checked })
                      }
                    />
                    {t("允许不安全证书")}
                  </label>
                )}
                  </>
                )}
              </>
            )}

          <label>
            <input
              aria-label={t("启用 UDP")}
              type="checkbox"
              checked={createForm.udpEnabled}
              disabled={busy}
              onChange={(event) =>
                updateCreateForm({ udpEnabled: event.target.checked })
              }
            />
            {t("启用 UDP")}
          </label>

          {createForm.protocol !== "custom" && (
            <>
              <label>
                {t("Xray Finalmask JSON")}
                <textarea
                  aria-label={t("Xray Finalmask JSON")}
                  rows={4}
                  placeholder={t("留空使用全局 Final Fragment 默认 mask")}
                  value={createForm.xrayFinalmaskJson}
                  disabled={busy}
                  onChange={(event) =>
                    updateCreateForm({ xrayFinalmaskJson: event.target.value })
                  }
                />
              </label>
              <p className="hint">
                {t("仅 Xray 且在设置中启用 Final Fragment 时生效；可填 mask 条目或 {tcp:[...]} 对象。")}
              </p>
            </>
          )}

          <div className="node-form-foot">
            <button
              type="button"
              disabled={busy}
              onClick={() => void onImportClipboard()}
            >
              {t("从分享链接粘贴")}
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => setDialog("import")}
            >
              {t("扫描二维码")}
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() =>
                setCreateForm(
                  blankManualNodeForm({
                    allowInsecure: settings?.defAllowInsecure,
                    fingerprint: settings?.defFingerprint,
                  }),
                )
              }
            >
              {t("重置")}
            </button>
            <span className="node-form-foot-end">
              <button
                type="button"
                disabled={busy}
                onClick={() => {
                  setDialog(null);
                  resetNodeForm();
                }}
              >
                {t("取消")}
              </button>
              <button
                type="button"
                className="btn-primary"
                disabled={busy}
                onClick={() => void onCreateNode()}
              >
                {editingNodeId === null ? t("创建节点") : t("保存节点")}
              </button>
            </span>
          </div>
        </div>

      </Dialog>

      <Dialog
        hidden={dialog !== "subscriptions"}
        title={t("订阅")}
        ariaLabel={t("订阅")}
        onClose={() => setDialog(null)}
        wide
      >
        <div className="dialog-menu">
          <button
            type="button"
            disabled={busy}
            onClick={resetSubscriptionForm}
          >
            {t("添加")}
          </button>
          <button
            type="button"
            disabled={busy || nodeTestInProgress || subscriptions.length === 0}
            onClick={() => void onRefreshAllSubscriptions()}
          >
            {t("全部更新")}
          </button>
        </div>

        {subscriptions.length === 0 ? (
          <p className="hint">{t("尚未添加订阅")}</p>
        ) : (
          <table className="node-list" aria-label={t("订阅列表")}>
            <thead>
              <tr>
                <th>{t("别名")}</th>
                <th>{t("节点")}</th>
                <th>{t("更新")}</th>
                <th>{t("操作")}</th>
              </tr>
            </thead>
            <tbody>
              {subscriptions.map((item) => (
                <tr key={item.id}>
                  <td>{item.name}</td>
                  <td>{item.nodeCount}</td>
                  <td>
                    {item.lastError ??
                      (item.lastUpdatedAt === null ? "从未" : "已更新")}
                  </td>
                  <td className="node-actions">
                    <button
                      type="button"
                      aria-label={`编辑 ${item.name}`}
                      disabled={busy || connected || nodeTestInProgress}
                      onClick={() => onEditSubscription(item)}
                    >
                      {t("编辑")}
                    </button>
                    <button
                      type="button"
                      aria-label={`刷新 ${item.name}`}
                      disabled={busy || nodeTestInProgress || !item.enabled}
                      onClick={() => void onRefreshSubscription(item.id)}
                    >
                      {t("刷新")}
                    </button>
                    <button
                      type="button"
                      aria-label={`删除订阅 ${item.name}`}
                      disabled={busy || connected || nodeTestInProgress}
                      onClick={() => void onDeleteSubscription(item.id)}
                    >
                      {t("删除")}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}

        <div className="subscription-form form-grid">
          <label>
            {t("名称")}
            <input
              aria-label={t("订阅名称")}
              value={subscriptionName}
              disabled={busy}
              onChange={(event) => setSubscriptionName(event.target.value)}
            />
          </label>
          <label>
            {t("地址")}
            <textarea
              aria-label={t("订阅地址")}
              disabled={busy}
              rows={3}
              placeholder={
                editingSubscriptionId === null
                  ? "https://\n一行一个，或用 | 分隔"
                  : t("留空则不修改")
              }
              value={subscriptionUrl}
              onChange={(event) => setSubscriptionUrl(event.target.value)}
            />
          </label>
          <label>
            {t("Subconverter 地址")}
            <input
              aria-label={t("Subconverter 地址")}
              disabled={busy}
              placeholder="http://127.0.0.1:25500/sub"
              value={subscriptionSubconverter}
              onChange={(event) =>
                setSubscriptionSubconverter(event.target.value)
              }
            />
          </label>
          <label>
            {t("更新间隔（分钟）")}
            <input
              aria-label={t("更新间隔")}
              type="number"
              min="1"
              value={subscriptionInterval}
              disabled={busy}
              onChange={(event) => setSubscriptionInterval(event.target.value)}
            />
          </label>
          <label>
            User-Agent
            <input
              aria-label={t("订阅 User-Agent")}
              placeholder={t("留空使用默认")}
              value={subscriptionUserAgent}
              disabled={busy}
              onChange={(event) => setSubscriptionUserAgent(event.target.value)}
            />
          </label>
          <label>
            {t("包含关键词")}
            <input
              aria-label={t("包含关键词")}
              placeholder={t("用 | 分隔，留空表示全部")}
              value={subscriptionInclude}
              disabled={busy}
              onChange={(event) => setSubscriptionInclude(event.target.value)}
            />
          </label>
          <label>
            {t("排除关键词")}
            <input
              aria-label={t("排除关键词")}
              placeholder={t("用 | 分隔")}
              value={subscriptionExclude}
              disabled={busy}
              onChange={(event) => setSubscriptionExclude(event.target.value)}
            />
          </label>
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={subscriptionAutoUpdate}
              disabled={busy}
              onChange={(event) =>
                setSubscriptionAutoUpdate(event.target.checked)
              }
            />
            {t("自动更新")}
          </label>
          {editingSubscriptionId !== null && (
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={subscriptionEnabled}
                disabled={busy}
                onChange={(event) => setSubscriptionEnabled(event.target.checked)}
              />
              {t("启用订阅")}
            </label>
          )}
        </div>

        <div className="actions">
          <button
            type="button"
            disabled={busy}
            onClick={() => void onSaveSubscription()}
          >
            {editingSubscriptionId === null ? "添加订阅" : "保存修改"}
          </button>
          {editingSubscriptionId !== null && (
            <button
              type="button"
              disabled={busy}
              onClick={resetSubscriptionForm}
            >
              {t("取消")}
            </button>
          )}
        </div>

      </Dialog>

      <Dialog
        hidden={dialog !== "hotkeys"}
        title={t("全局热键设置")}
        ariaLabel={t("全局热键设置")}
        onClose={() => setDialog(null)}
      >
        {settings === null ? (
          <p className="hint">{t("正在读取设置…")}</p>
        ) : (
          <div className="form-grid">
            <label>
              {t("热键：连接/断开")}
              <input
                aria-label={t("热键：连接/断开")}
                placeholder="Ctrl+Enter"
                value={settings.hotkeyConnect}
                disabled={busy}
                onChange={(event) =>
                  void onChangeSettings({ hotkeyConnect: event.target.value })
                }
              />
            </label>
            <label>
              {t("热键：上一节点")}
              <input
                aria-label={t("热键：上一节点")}
                placeholder="Ctrl+["
                value={settings.hotkeyPrevious}
                disabled={busy}
                onChange={(event) =>
                  void onChangeSettings({ hotkeyPrevious: event.target.value })
                }
              />
            </label>
            <label>
              {t("热键：下一节点")}
              <input
                aria-label={t("热键：下一节点")}
                placeholder="Ctrl+]"
                value={settings.hotkeyNext}
                disabled={busy}
                onChange={(event) =>
                  void onChangeSettings({ hotkeyNext: event.target.value })
                }
              />
            </label>
            <p className="form-section">
              {t("热键在系统全局生效；窗口未聚焦时也可使用。留空表示禁用。")}
            </p>
          </div>
        )}
      </Dialog>

      <Dialog
        hidden={dialog !== "backup"}
        title={t("备份和还原")}
        ariaLabel={t("备份和还原")}
        onClose={() => setDialog(null)}
      >
        <div className="form-grid">
          <span className="form-label">{t("备份到本地")}</span>
          <button type="button" disabled={busy} onClick={() => void onExportProfile()}>
            {t("备份到本地")}
          </button>
          <span className="form-tip" />
          <span className="form-label">{t("从本地还原")}</span>
          <button
            type="button"
            disabled={busy || connected}
            onClick={() => void onImportProfile()}
          >
            {t("从本地还原")}
          </button>
          <span className="form-tip" />
        </div>
      </Dialog>



      <Dialog
        hidden={dialog !== "geo"}
        title={t("更新 Geo 文件")}
        ariaLabel={t("更新 Geo 文件")}
        onClose={() => setDialog(null)}
      >
        <h2>{t("更新 Geo 文件")}</h2>
        <p className="hint">
          {t("下载 Xray 使用的 geoip.dat / geosite.dat。sing-box 在连接时按需拉取远程规则集。")}
        </p>
        {geoStatus === null ? (
          <p className="hint">{t("正在读取 Geo 状态")}</p>
        ) : (
          <div className="settings-form">
            <p className="hint">
              {t("目录")}：{geoStatus.directory}
            </p>
            <p>
              geoip.dat —{" "}
              {geoStatus.geoip.present
                ? `${formatBytes(geoStatus.geoip.bytes)}${
                    geoStatus.geoip.modifiedAt === null
                      ? ""
                      : ` · ${formatClock(geoStatus.geoip.modifiedAt * 1000)}`
                  }`
                : t("未下载")}
            </p>
            <p>
              geosite.dat —{" "}
              {geoStatus.geosite.present
                ? `${formatBytes(geoStatus.geosite.bytes)}${
                    geoStatus.geosite.modifiedAt === null
                      ? ""
                      : ` · ${formatClock(geoStatus.geosite.modifiedAt * 1000)}`
                  }`
                : t("未下载")}
            </p>
            <p className="hint">
              {geoStatus.assetEnvApplied
                ? t("已准备就绪（下次启动 Xray 时生效）")
                : t("下载完成后，下次启动 Xray 时生效")}
            </p>
          </div>
        )}
        <div className="actions">
          <button
            type="button"
            disabled={busy}
            onClick={() => void onUpdateGeo()}
          >
            {t("立即更新")}
          </button>
        </div>
      </Dialog>


      <Dialog
        hidden={dialog !== "config"}
        title={t("查看配置")}
        ariaLabel={t("查看配置")}
        wide
        onClose={() => setDialog(null)}
      >
        <p className="hint">
          {t("这是下次连接将写给 Core 的完整配置，包含已生效的模板。编辑后保存为覆盖配置，连接时会原样使用它，不再重新生成——节点或路由改变时它不会跟着变。")}
        </p>
        <textarea
          aria-label={t("生成的 Core 配置")}
          rows={20}
          value={coreConfig}
          onChange={(event) => setCoreConfig(event.target.value)}
        />
        <div className="actions">
          <button
            type="button"
            disabled={busy}
            onClick={() => void onChangeSettings({ configOverride: coreConfig })}
          >
            {t("保存为覆盖配置")}
          </button>
          <button
            type="button"
            disabled={busy || (settings?.configOverride ?? "") === ""}
            onClick={() => void onChangeSettings({ configOverride: "" })}
          >
            {t("清除覆盖配置")}
          </button>
        </div>
        {(settings?.configOverride ?? "") !== "" && (
          <p className="hint">
            {t("覆盖配置已生效：连接使用的是这份文档，而不是生成的配置。")}
          </p>
        )}
      </Dialog>

      <Dialog
        hidden={dialog !== "about"}
        title={t("关于")}
        ariaLabel={t("关于")}
        onClose={() => setDialog(null)}
      >
        <p>MgClash</p>
        <p>{platform?.artifactIdentifier}</p>
        <p>{t("界面以 v2rayN Avalonia 为模板重新实现，未使用其 GPL 源码。")}</p>
        <p>
          {t("关闭窗口时最小化到托盘")}: {settings?.closeToTray ? t("已启用") : t("未启用")}
        </p>
        <p className="hint">
          {t("托盘菜单支持打开窗口、连接/断开、切换路由模式与选择节点。")}
        </p>
      </Dialog>

    </>
  );
}
