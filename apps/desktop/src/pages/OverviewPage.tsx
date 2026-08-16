import { formatBytes, formatRate, latencyQuality, TUN_NOTICE } from "../appHelpers";
import type { AppModel } from "../hooks/useAppController";
import { Sparkline } from "../components/traffic/Sparkline";
import { IconInfo, IconWarn } from "../components/Icons";
import {
  Badge,
  Card,
  SegmentedControl,
  StatCard,
  StatusDot,
  Toggle,
} from "../components/ui/Ui";
import type { RoutingMode, SystemProxyMode } from "../session";
import { setRoutingMode } from "../session";

const PROXY_LABEL: Record<SystemProxyMode, string> = {
  managed: "自动配置系统代理",
  cleared: "清除系统代理",
  unchanged: "不改变系统代理",
  pac: "Pac 模式",
};

/// The hero already labels the field "系统代理", so the value drops the words
/// it would otherwise repeat. The popover keeps `PROXY_LABEL`, where each entry
/// stands on its own.
const PROXY_VALUE: Record<SystemProxyMode, string> = {
  managed: "自动配置",
  cleared: "清除",
  unchanged: "不改变",
  pac: "Pac 模式",
};

export function OverviewPage({ app }: { app: AppModel }) {
  const {
    t,
    busy,
    connected,
    node,
    status,
    settings,
    platform,
    traffic,
    trafficHistory,
    nodeTests,
    onToggleConnect,
    onTestNode,
    onRestartCore,
    goTo,
    onChangeSettings,
    run,
    proxyPopoverOpen,
    setProxyPopoverOpen,
    connections,
    geoStatus,
    coreReadiness,
  } = app;
  const test = node ? nodeTests[node.id] : undefined;
  const ms =
    test?.status === "success" ? test.latencyMs : (node?.latencyMs ?? null);
  const tunUnavailable =
    platform?.tunAvailability === "unavailableInUnsignedBuild";

  return (
    <div className="page-pad">
      <Card className="hero-card">
        <div className="hero-main">
          <div>
            <div className="hero-status">
              <StatusDot
                size={9}
                tone={connected ? "ok" : "muted"}
                pulse={connected}
              />
              <div>
                <h1>{connected ? t("已连接") : t("未连接")}</h1>
                <span>{connected ? "Connected" : "Disconnected"}</span>
              </div>
            </div>
            {node ? (
              <div className="hero-node">
                <strong>{node.name}</strong>
                <Badge tone="acc">{node.protocol}</Badge>
                <span className="mono muted">
                  {node.server}:{node.port}
                </span>
              </div>
            ) : (
              <p className="hint">{t("尚未导入节点")}</p>
            )}
            <dl className="hero-fields">
              <div>
                <dt>{t("延迟")} LATENCY</dt>
                <dd className={ms === null ? undefined : `tone-${latencyQuality(ms)}`}>
                  {ms === null ? "—" : `${ms} ms`}
                </dd>
              </div>
              <div>
                <dt>{t("传输协议")} TRANSPORT</dt>
                <dd>{node?.transport ?? "—"}</dd>
              </div>
              <div>
                <dt>{t("Core")} CORE</dt>
                <dd>{status?.core ?? "—"}</dd>
              </div>
              <div>
                <dt>{t("分组")} GROUP</dt>
                <dd>
                  {node?.groupId
                    ? (app.nodeGroupNames.get(node.groupId) ?? t("未知分组"))
                    : t("未分组")}
                </dd>
              </div>
            </dl>
          </div>
          <div className="hero-actions">
            <button
              type="button"
              className={connected ? "btn-danger-ghost" : "btn-primary"}
              disabled={busy || (!connected && node === null)}
              onClick={onToggleConnect}
            >
              {connected ? t("断开") : t("连接")}
            </button>
            <button
              type="button"
              className="btn-secondary"
              onClick={() => goTo("nodes")}
            >
              {t("切换节点")}
            </button>
            <div className="hero-ghosts">
              <button
                type="button"
                className="btn-ghost"
                disabled={busy || node === null}
                onClick={() => node && void onTestNode(node.id)}
              >
                {t("测试延迟")}
              </button>
              <button
                type="button"
                className="btn-ghost"
                disabled={busy}
                onClick={onRestartCore}
              >
                {t("重启内核")}
              </button>
            </div>
          </div>
        </div>
        <div className="hero-foot">
          <div>
            <span className="field-label">{t("路由模式")}</span>
            <SegmentedControl
              ariaLabel={t("路由模式")}
              value={status?.mode ?? "global"}
              disabled={busy || status === null}
              onChange={(mode) => void run(() => setRoutingMode(mode))}
              options={[
                { value: "global" as RoutingMode, label: t("全局") },
                { value: "rule" as RoutingMode, label: t("规则") },
                { value: "direct" as RoutingMode, label: t("直连") },
              ]}
            />
          </div>
          <div className="hero-proxy">
            <StatusDot
              tone={settings?.systemProxyMode === "cleared" ? "muted" : "ok"}
            />
            <div>
              <strong>{t("系统代理")}</strong>
              <span>
                {t(PROXY_VALUE[settings?.systemProxyMode ?? "managed"])}
              </span>
            </div>
            <button
              type="button"
              className="btn-ghost"
              onClick={() => setProxyPopoverOpen(!proxyPopoverOpen)}
            >
              ▾
            </button>
            {proxyPopoverOpen ? (
              <div className="popover">
                {(Object.keys(PROXY_LABEL) as SystemProxyMode[]).map((mode) => (
                  <button
                    key={mode}
                    type="button"
                    onClick={() => {
                      void onChangeSettings({ systemProxyMode: mode });
                      setProxyPopoverOpen(false);
                    }}
                  >
                    {t(PROXY_LABEL[mode])}
                  </button>
                ))}
              </div>
            ) : null}
          </div>
          <div className="hero-tun">
            <div>
              <strong>TUN</strong>
              <span>
                {tunUnavailable
                  ? t("不可用")
                  : settings?.tunEnabled
                    ? t("已启用")
                    : t("未启用")}
              </span>
            </div>
            <span
              className="tun-tip"
              title={platform ? TUN_NOTICE[platform.tunAvailability] : undefined}
            >
              <IconInfo />
            </span>
            <Toggle
              label={t("启用 TUN")}
              checked={settings?.tunEnabled ?? false}
              disabled={
                busy || connected || settings === null || tunUnavailable
              }
              onChange={(checked) => void onChangeSettings({ tunEnabled: checked })}
            />
          </div>
        </div>
      </Card>

      <div className="stat-grid">
        <StatCard
          label={t("上传速率")}
          value={formatRate(traffic.uploadBytesPerSecond).replace(/ .*/, "")}
          unit={formatRate(traffic.uploadBytesPerSecond).split(" ").slice(1).join(" ")}
        />
        <StatCard
          label={t("下载速率")}
          value={formatRate(traffic.downloadBytesPerSecond).replace(/ .*/, "")}
          unit={formatRate(traffic.downloadBytesPerSecond).split(" ").slice(1).join(" ")}
        />
        <StatCard label={t("今日流量")} value={formatBytes(traffic.todayBytes)} />
        <StatCard label={t("累计流量")} value={formatBytes(traffic.totalBytes)} />
      </div>

      <div className="overview-split">
        <Card>
          <header className="card-head">
            <strong>{t("实时流量")}</strong>
            <span className="muted">{t("最近 60 秒")}</span>
            <span className="spark-legend">
              <i className="lg-down" /> {t("下载")}
              <i className="lg-up" /> {t("上传")}
            </span>
          </header>
          <Sparkline
            samples={trafficHistory}
            ticks={["-60s", "-30s", "now"]}
          />
        </Card>
        <Card>
          <header className="card-head">
            <strong>{t("运行状态")}</strong>
          </header>
          <dl className="kv-list">
            <div>
              <dt>{t("本地")} SOCKS</dt>
              <dd className="mono">{status?.socksPort ?? "—"}</dd>
            </div>
            <div>
              <dt>{t("本地")} HTTP</dt>
              <dd className="mono">{status?.httpPort ?? "—"}</dd>
            </div>
            <div>
              <dt>Clash API</dt>
              <dd className="mono">{status?.clashApiPort ?? "—"}</dd>
            </div>
            <div>
              <dt>{t("局域网")}</dt>
              <dd>
                <Badge tone={settings?.allowLan ? "ok" : "neutral"}>
                  {settings?.allowLan ? t("启用") : t("关闭")}
                </Badge>
              </dd>
            </div>
            <div>
              <dt>Geo</dt>
              <dd className="mono">
                {geoStatus?.geoip.present || geoStatus?.geosite.present
                  ? t("已准备就绪（下次启动 Xray 时生效）")
                  : t("未下载")}
              </dd>
            </div>
            <div>
              <dt>{t("活动连接")}</dt>
              <dd className="mono">{connections?.connections.length ?? 0}</dd>
            </div>
          </dl>
          {coreReadiness && !coreReadiness.singBox && !coreReadiness.xray ? (
            <p className="hint">
              <IconWarn /> {t("缺少 Core")}
            </p>
          ) : null}
        </Card>
      </div>
    </div>
  );
}