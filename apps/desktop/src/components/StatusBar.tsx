import type { PlatformSummary } from "../platform";
import type {
  AppSettings,
  RoutingMode,
  SessionStatus,
  SystemProxyMode,
  TrafficSnapshot,
} from "../session";
import { formatRate } from "../appHelpers";

interface StatusBarProps {
  busy: boolean;
  connected: boolean;
  platform: PlatformSummary | null;
  settings: AppSettings | null;
  status: SessionStatus | null;
  traffic: TrafficSnapshot;
  t: (text: string) => string;
  onChangeSettings: (patch: Partial<AppSettings>) => void;
  onRoutingMode: (mode: RoutingMode) => void;
}

export function StatusBar({
  busy,
  connected,
  platform,
  settings,
  status,
  traffic,
  t,
  onChangeSettings,
  onRoutingMode,
}: StatusBarProps) {
  const proxyMode = settings?.systemProxyMode ?? "managed";
  const nodeName = status?.node?.name ?? "—";

  return (
    <footer className="statusbar">
      <div className="status-stack">
        <span>
          {t("本地")} socks {status?.socksPort ?? "—"} · http{" "}
          {status?.httpPort ?? "—"}
        </span>
        <span>
          {t("局域网")}{" "}
          {settings?.allowLan === true
            ? `socks ${status?.socksPort ?? "—"} · http ${status?.httpPort ?? "—"}`
            : t("关闭")}
        </span>
      </div>
      <label className="status-control tun-control">
        {t("启用 TUN")}
        <input
          aria-label={t("状态栏 TUN")}
          type="checkbox"
          className="toggle"
          checked={settings?.tunEnabled ?? false}
          disabled={
            busy ||
            settings === null ||
            platform?.tunAvailability === "unavailableInUnsignedBuild"
          }
          onChange={(event) =>
            onChangeSettings({ tunEnabled: event.target.checked })
          }
        />
      </label>
      <label className="status-control">
        <select
          aria-label={t("状态栏系统代理")}
          className="proxy-mode-select"
          disabled={busy || settings === null}
          title={
            proxyMode === "pac"
              ? t("PAC 模式会启动本地 PAC 服务并写入系统代理（全局脚本，不等价于规则模式）。")
              : t("系统代理")
          }
          value={proxyMode}
          onChange={(event) =>
            onChangeSettings({
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
      <label className="status-control">
        <select
          aria-label={t("状态栏路由模式")}
          disabled={busy || status === null}
          value={status?.mode ?? "global"}
          onChange={(event) => onRoutingMode(event.target.value as RoutingMode)}
        >
          <option value="global">{t("全局")}</option>
          <option value="rule">{t("规则")}</option>
          <option value="direct">{t("直连")}</option>
        </select>
      </label>
      <div className="status-stack status-running">
        <span>
          {nodeName} · Core {status?.core ?? "—"}
        </span>
        <span>
          {connected ? t("已连接") : t("未连接")}
          {" · "}
          {platform?.artifactIdentifier ?? "—"}
        </span>
      </div>
      <div className="status-stack status-speeds">
        <span aria-label={t("下载速率")}>
          ↓ {formatRate(traffic.downloadBytesPerSecond)}
        </span>
        <span aria-label={t("上传速率")}>
          ↑ {formatRate(traffic.uploadBytesPerSecond)}
        </span>
      </div>
    </footer>
  );
}
