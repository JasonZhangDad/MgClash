import { formatBytes, formatClock, latencyQuality } from "../../appHelpers";
import type { AppModel } from "../../hooks/useAppController";
import type { InspectTab } from "../../appHelpers";
import { IconClose, IconMore } from "../Icons";
import { Badge, StatusDot } from "../ui/Ui";

function Row({ label, value, mono = false }: { label: string; mono?: boolean; value: string }) {
  return (
    <div className="inspect-row">
      <span>{label}</span>
      <strong className={mono ? "mono" : undefined}>{value}</strong>
    </div>
  );
}

export function NodeInspector({ app }: { app: AppModel }) {
  const {
    t,
    inspected,
    inspectDraft,
    inspectTab,
    setInspectTab,
    setInspectedId,
    node,
    nodeTests,
    nodeTraffic,
    connected,
    busy,
    onActivateNode,
    onEditNode,
    onTestNode,
    onSpeedTest,
    onShowNodeQrCode,
    onExportNodeLink,
    openNodeMenu,
    settings,
    nodeGroupNames,
  } = app;
  if (inspected === null) {
    return null;
  }
  const test = nodeTests[inspected.id];
  const ms =
    test?.status === "success" ? test.latencyMs : inspected.latencyMs;
  const traffic = nodeTraffic[inspected.id];
  const cred = inspectDraft?.credential;
  const tabs: { id: InspectTab; label: string }[] = [
    { id: "general", label: t("概览") },
    { id: "auth", label: t("认证") },
    { id: "transport", label: t("传输协议") },
    { id: "tls", label: "TLS" },
    { id: "advanced", label: t("高级") },
    { id: "chain", label: t("链路") },
    { id: "diagnostics", label: t("诊断") },
  ];

  return (
    <aside className="drawer" aria-label={t("节点详情")}>
      <header className="drawer-head">
        <div>
          <div className="drawer-title">
            <StatusDot tone={inspected.id === node?.id ? "ok" : "muted"} />
            <strong>{inspected.name}</strong>
            <Badge>{inspected.protocol}</Badge>
          </div>
          <p className="mono muted">
            {inspected.server}:{inspected.port}
          </p>
        </div>
        <button
          type="button"
          className="icon-btn"
          aria-label={t("关闭")}
          onClick={() => setInspectedId(null)}
        >
          <IconClose />
        </button>
      </header>
      <div className="drawer-actions">
        <button
          type="button"
          className="btn-primary"
          disabled={busy || inspected.id === node?.id || !inspected.enabled}
          onClick={() => void onActivateNode(inspected.id)}
        >
          {t("设为活动")}
        </button>
        <button
          type="button"
          className="btn-secondary"
          disabled={busy || (connected && inspected.id === node?.id)}
          onClick={() => void onEditNode(inspected)}
        >
          {t("编辑")}
        </button>
        <button
          type="button"
          className="icon-btn"
          aria-label={`操作 ${inspected.name}`}
          onClick={(event) => openNodeMenu(event, inspected.id)}
        >
          <IconMore />
        </button>
      </div>
      <div className="page-tabs drawer-tabs">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            className={inspectTab === tab.id ? "is-on" : undefined}
            onClick={() => setInspectTab(tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </div>
      <div className="drawer-body">
        {inspectTab === "general" ? (
          <>
            <Row label={t("协议")} value={inspected.protocol} />
            <Row label={t("地址")} value={`${inspected.server}:${inspected.port}`} mono />
            <Row label={t("传输协议")} value={inspected.transport} />
            <Row label="TLS" value={inspected.tls ?? "—"} />
            <Row
              label={t("分组")}
              value={
                inspected.groupId
                  ? (nodeGroupNames.get(inspected.groupId) ?? t("未知分组"))
                  : t("未分组")
              }
            />
            <Row
              label={t("启用")}
              value={inspected.enabled ? t("启用") : t("已禁用")}
            />
          </>
        ) : null}
        {inspectTab === "auth" && cred ? (
          <>
            {"userId" in cred ? <Row label="UUID" value={cred.userId} mono /> : null}
            {"flow" in cred ? <Row label="flow" value={cred.flow ?? "—"} /> : null}
            {"alterId" in cred ? <Row label="alterId" value={String(cred.alterId)} /> : null}
            {"security" in cred ? <Row label={t("加密方式")} value={cred.security} /> : null}
            {"method" in cred ? <Row label={t("加密方式")} value={cred.method} /> : null}
            {"password" in cred ? <Row label={t("密码")} value="••••••••" /> : null}
            {"username" in cred ? <Row label={t("用户名")} value={cred.username ?? "—"} /> : null}
            {"authentication" in cred ? (
              <Row label={t("认证密码")} value={cred.authentication ?? "—"} />
            ) : null}
            {"uuid" in cred ? <Row label="UUID" value={cred.uuid} mono /> : null}
            {"privateKey" in cred ? <Row label={t("私钥")} value="••••••••" /> : null}
            {"document" in cred ? <Row label={t("Core 类型")} value={cred.core} /> : null}
          </>
        ) : inspectTab === "auth" ? (
          <p className="hint">{t("正在读取设置…")}</p>
        ) : null}
        {inspectTab === "transport" ? (
          inspectDraft?.transport ? (
            <>
              <Row label={t("传输方式")} value={inspectDraft.transport.type} />
              {"path" in inspectDraft.transport ? (
                <Row label={t("路径")} value={inspectDraft.transport.path} mono />
              ) : null}
              {"host" in inspectDraft.transport ? (
                <Row label="Host" value={inspectDraft.transport.host ?? "—"} />
              ) : null}
              {inspectDraft.transport.type === "xhttp" ? (
                <Row label={t("模式")} value={inspectDraft.transport.mode} />
              ) : null}
              {"serviceName" in inspectDraft.transport ? (
                <Row label="serviceName" value={inspectDraft.transport.serviceName} />
              ) : null}
              {inspectDraft.transport.type === "grpc" ? (
                <>
                  <Row
                    label="authority"
                    value={inspectDraft.transport.authority ?? "—"}
                  />
                  <Row label={t("gRPC 模式")} value={inspectDraft.transport.mode} />
                </>
              ) : null}
              {inspectDraft.transport.type === "kcp" ? (
                <>
                  <Row label="seed" value={inspectDraft.transport.seed ?? "—"} mono />
                  <Row
                    label={t("伪装类型")}
                    value={inspectDraft.transport.headerType ?? "—"}
                  />
                  <Row
                    label="mtu"
                    value={
                      inspectDraft.transport.mtu === null
                        ? "—"
                        : String(inspectDraft.transport.mtu)
                    }
                  />
                  <Row
                    label="tti"
                    value={
                      inspectDraft.transport.tti === null
                        ? "—"
                        : String(inspectDraft.transport.tti)
                    }
                  />
                  <Row
                    label="uplinkCapacity"
                    value={
                      inspectDraft.transport.uplinkCapacity === null
                        ? "—"
                        : String(inspectDraft.transport.uplinkCapacity)
                    }
                  />
                  <Row
                    label="downlinkCapacity"
                    value={
                      inspectDraft.transport.downlinkCapacity === null
                        ? "—"
                        : String(inspectDraft.transport.downlinkCapacity)
                    }
                  />
                  <Row
                    label={t("启用拥塞控制")}
                    value={
                      inspectDraft.transport.congestion ? t("启用") : t("关闭")
                    }
                  />
                </>
              ) : null}
            </>
          ) : (
            <p className="hint">
              {inspected.protocol === "hysteria2" || inspected.protocol === "tuic"
                ? t("没有额外流传输设置")
                : inspected.protocol === "wireguard"
                  ? t("传输协议")
                  : inspected.transport}
            </p>
          )
        ) : null}
        {inspectTab === "tls" ? (
          inspectDraft?.tls ? (
            <>
              <Row label="type" value={inspectDraft.tls.type} />
              {"serverName" in inspectDraft.tls ? (
                <Row label="SNI" value={inspectDraft.tls.serverName ?? "—"} />
              ) : null}
              {"alpn" in inspectDraft.tls ? (
                <Row
                  label="ALPN"
                  value={
                    inspectDraft.tls.alpn.length === 0
                      ? "—"
                      : inspectDraft.tls.alpn.join(", ")
                  }
                />
              ) : null}
              {"fingerprint" in inspectDraft.tls ? (
                <Row
                  label={t("TLS 指纹")}
                  value={inspectDraft.tls.fingerprint ?? "—"}
                />
              ) : null}
              {"publicKey" in inspectDraft.tls ? (
                <Row label="publicKey" value={inspectDraft.tls.publicKey} mono />
              ) : null}
              {inspectDraft.tls.type === "reality" ? (
                <>
                  <Row
                    label="shortId"
                    value={inspectDraft.tls.shortId ?? "—"}
                    mono
                  />
                  <Row
                    label="spiderX"
                    value={inspectDraft.tls.spiderX ?? "—"}
                    mono
                  />
                </>
              ) : null}
              {"allowInsecure" in inspectDraft.tls ? (
                <Row
                  label={t("允许不安全证书")}
                  value={inspectDraft.tls.allowInsecure ? t("启用") : t("关闭")}
                />
              ) : null}
              {"pinnedSha256" in inspectDraft.tls ? (
                <Row
                  label={t("证书固定 pinSHA256")}
                  value={inspectDraft.tls.pinnedSha256 ?? "—"}
                  mono
                />
              ) : null}
            </>
          ) : (
            <p className="hint">{inspected.tls ?? "—"}</p>
          )
        ) : null}
        {inspectTab === "advanced" ? (
          <>
            <Row label={t("启用 Mux")} value={settings?.muxEnabled ? t("启用") : t("关闭")} />
            <Row label={t("启用 Fragment")} value={settings?.fragmentEnabled ? t("启用") : t("关闭")} />
            <Row label={t("启用 UDP Noise")} value={settings?.udpNoiseEnabled ? t("启用") : t("关闭")} />
            <Row
              label={t("Xray Finalmask JSON")}
              value={inspectDraft?.xrayFinalmaskJson ?? "—"}
              mono
            />
          </>
        ) : null}
        {inspectTab === "chain" ? (
          <>
            <Row
              label={t("前置代理")}
              value={
                inspected.frontNodeId
                  ? (app.nodes.find((item) => item.id === inspected.frontNodeId)?.name ??
                    inspected.frontNodeId)
                  : t("不使用")
              }
            />
            <Row
              label={t("分组策略")}
              value={
                nodeGroupNames.get(inspected.groupId ?? "") ?? t("手动选择")
              }
            />
          </>
        ) : null}
        {inspectTab === "diagnostics" ? (
          <>
            <Row
              label={t("延迟")}
              value={
                test?.status === "testing"
                  ? t("测试中")
                  : test?.status === "timeout"
                    ? t("超时")
                    : ms === null
                      ? "—"
                      : `${ms} ms`
              }
            />
            <Row label={t("URL 测试地址")} value={app.urlTestAddress} mono />
            <Row
              label={t("下载测速地址")}
              value={settings?.speedTestUrl ?? "—"}
              mono
            />
            <Row
              label={t("今日流量")}
              value={formatBytes(
                (traffic?.todayDownloadBytes ?? 0) + (traffic?.todayUploadBytes ?? 0),
              )}
            />
            <Row
              label={t("累计流量")}
              value={formatBytes(
                (traffic?.totalDownloadBytes ?? 0) + (traffic?.totalUploadBytes ?? 0),
              )}
            />
            <div className="drawer-actions">
              <button type="button" onClick={() => void onTestNode(inspected.id)}>
                {t("测试延迟")}
              </button>
              <button
                type="button"
                disabled={!connected || inspected.id !== node?.id}
                onClick={() => void onSpeedTest()}
              >
                {t("下载测速")}
              </button>
              <button type="button" onClick={() => void onShowNodeQrCode(inspected)}>
                {t("显示二维码")}
              </button>
              <button type="button" onClick={() => void onExportNodeLink(inspected.id)}>
                {t("导出分享链接")}
              </button>
            </div>
            {inspected.lastTestedAt ? (
              <p className="hint">{formatClock(inspected.lastTestedAt)}</p>
            ) : null}
            <span className="sr-only">
              {ms !== null ? latencyQuality(ms) : ""}
            </span>
          </>
        ) : null}
      </div>
    </aside>
  );
}