import { LOCALES, type Locale } from "../i18n";
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
  locale: Locale;
  platform: PlatformSummary | null;
  settings: AppSettings | null;
  status: SessionStatus | null;
  traffic: TrafficSnapshot;
  t: (text: string) => string;
  onChangeSettings: (patch: Partial<AppSettings>) => void;
  onRoutingMode: (mode: RoutingMode) => void;
}

const PROXY_MODES: { id: SystemProxyMode; label: string }[] = [
  { id: "managed", label: "自动配置" },
  { id: "pac", label: "PAC" },
  { id: "cleared", label: "清除" },
  { id: "unchanged", label: "不改变" },
];

export function StatusBar({
  busy,
  connected,
  locale,
  platform,
  settings,
  status,
  traffic,
  t,
  onChangeSettings,
  onRoutingMode,
}: StatusBarProps) {
  const proxyMode = settings?.systemProxyMode ?? "managed";

  return (
    <footer className="statusbar">
      <span>
        SOCKS {status?.socksPort ?? "—"} · HTTP {status?.httpPort ?? "—"}
        {" · "}
        API {status?.clashApiPort ?? settings?.clashApiPort ?? "—"}
      </span>
      <span>Core {status?.core ?? "—"}</span>
      <span className="statusbar-artifact">
        {platform?.artifactIdentifier ?? "—"}
      </span>
      <label className="status-control">
        {t("路由")}
        <select
          aria-label={t("状态栏路由模式")}
          disabled={busy || connected || status === null}
          value={status?.mode ?? "global"}
          onChange={(event) => onRoutingMode(event.target.value as RoutingMode)}
        >
          <option value="global">{t("全局")}</option>
          <option value="rule">{t("规则")}</option>
          <option value="direct">{t("直连")}</option>
        </select>
      </label>
      <label className="status-control">
        <input
          aria-label={t("状态栏 TUN")}
          type="checkbox"
          checked={settings?.tunEnabled ?? false}
          disabled={
            busy ||
            connected ||
            settings === null ||
            platform?.tunAvailability === "unavailableInUnsignedBuild"
          }
          onChange={(event) =>
            onChangeSettings({ tunEnabled: event.target.checked })
          }
        />
        TUN
      </label>
      <label className="status-control">
        {t("语言")}
        <select
          aria-label={t("界面语言")}
          disabled={busy || settings === null}
          value={locale}
          onChange={(event) =>
            onChangeSettings({ locale: event.target.value as Locale })
          }
        >
          {LOCALES.map((entry) => (
            <option key={entry.id} value={entry.id}>
              {entry.label}
            </option>
          ))}
        </select>
      </label>
      <div className="proxy-mode-group" role="group">
        <span>{t("系统代理")}</span>
        {PROXY_MODES.map((mode) => (
          <button
            key={mode.id}
            type="button"
            className={proxyMode === mode.id ? "active" : undefined}
            disabled={busy || connected || settings === null}
            aria-pressed={proxyMode === mode.id}
            title={
              mode.id === "pac"
                ? t("PAC 模式会启动本地 PAC 服务并写入系统代理（全局脚本，不等价于规则模式）。")
                : undefined
            }
            onClick={() => onChangeSettings({ systemProxyMode: mode.id })}
          >
            {mode.id === "pac" ? "PAC" : t(mode.label)}
          </button>
        ))}
        <select
          aria-label={t("状态栏系统代理")}
          className="proxy-mode-select"
          disabled={busy || connected || settings === null}
          value={proxyMode}
          onChange={(event) =>
            onChangeSettings({
              systemProxyMode: event.target.value as SystemProxyMode,
            })
          }
        >
          <option value="managed">{t("自动配置")}</option>
          <option value="pac">PAC</option>
          <option value="cleared">{t("清除")}</option>
          <option value="unchanged">{t("不改变")}</option>
        </select>
      </div>
      <span className="statusbar-rates" aria-label={t("下载速率")}>
        ↓ {formatRate(traffic.downloadBytesPerSecond)}
      </span>
      <span aria-label={t("上传速率")}>
        ↑ {formatRate(traffic.uploadBytesPerSecond)}
      </span>
      <span className={connected ? "badge on" : "badge off"}>
        {connected ? t("已连接") : t("未连接")}
      </span>
    </footer>
  );
}
