import { useState } from "react";

import { formatRate, TUN_LABEL } from "../../appHelpers";
import type { AppModel } from "../../hooks/useAppController";
import {
  setRoutingMode,
  type RoutingMode,
  type SystemProxyMode,
} from "../../session";
import { StatusDot } from "../ui/Ui";

const PROXY_LABEL: Record<SystemProxyMode, string> = {
  managed: "自动配置系统代理",
  cleared: "清除系统代理",
  unchanged: "不改变系统代理",
  pac: "Pac 模式",
};

const MODE_LABEL: Record<RoutingMode, string> = {
  global: "全局",
  rule: "规则",
  direct: "直连",
};

export function AppStatusBar({ app }: { app: AppModel }) {
  const {
    t,
    busy,
    connected,
    platform,
    settings,
    status,
    traffic,
    onChangeSettings,
    run,
    portsOpen,
    setPortsOpen,
    node,
  } = app;
  const [routeOpen, setRouteOpen] = useState(false);
  const [proxyOpen, setProxyOpen] = useState(false);
  const proxyMode = settings?.systemProxyMode ?? "managed";
  const tunUnavailable =
    platform?.tunAvailability === "unavailableInUnsignedBuild";
  const mode = status?.mode ?? "global";

  return (
    <footer className="app-statusbar">
      <StatusDot tone={connected ? "ok" : "muted"} />
      <strong className={connected ? "tone-ok" : "tone-muted"}>
        {connected ? t("已连接") : t("未连接")}
      </strong>
      <i className="sb-sep" />
      <span className="sb-node">{node?.name ?? "—"}</span>
      <i className="sb-sep" />
      <div className="sb-pop">
        <button
          type="button"
          className="sb-chip"
          aria-label={t("状态栏路由模式")}
          disabled={busy || status === null}
          onClick={() => {
            setRouteOpen((open) => !open);
            setProxyOpen(false);
            setPortsOpen(false);
          }}
        >
          {t(MODE_LABEL[mode])}
        </button>
        {routeOpen ? (
          <div className="popover" role="listbox">
            {(Object.keys(MODE_LABEL) as RoutingMode[]).map((value) => (
              <button
                key={value}
                type="button"
                data-value={value}
                onClick={() => {
                  void run(() => setRoutingMode(value));
                  setRouteOpen(false);
                }}
              >
                {t(MODE_LABEL[value])}
              </button>
            ))}
          </div>
        ) : null}
      </div>
      <i className="sb-sep" />
      <div className="sb-pop">
        <button
          type="button"
          className="sb-chip"
          aria-label={t("状态栏系统代理")}
          disabled={busy || settings === null}
          title={
            proxyMode === "pac"
              ? t("PAC 模式会启动本地 PAC 服务并写入系统代理（全局脚本，不等价于规则模式）。")
              : t("系统代理")
          }
          onClick={() => {
            setProxyOpen((open) => !open);
            setRouteOpen(false);
            setPortsOpen(false);
          }}
        >
          {proxyMode === "cleared" ? "OFF" : "ON"}
        </button>
        {proxyOpen ? (
          <div className="popover">
            {(Object.keys(PROXY_LABEL) as SystemProxyMode[]).map((value) => (
              <button
                key={value}
                type="button"
                data-value={value}
                onClick={() => {
                  void onChangeSettings({ systemProxyMode: value });
                  setProxyOpen(false);
                }}
              >
                {t(PROXY_LABEL[value])}
              </button>
            ))}
          </div>
        ) : null}
      </div>
      <i className="sb-sep" />
      <label className="sb-inline tun-control">
        {t("TUN")}{" "}
        {tunUnavailable
          ? t("不可用")
          : settings?.tunEnabled
            ? "ON"
            : "OFF"}
        <input
          aria-label={t("状态栏 TUN")}
          type="checkbox"
          className="toggle"
          checked={settings?.tunEnabled ?? false}
          disabled={busy || settings === null || tunUnavailable}
          title={
            platform ? TUN_LABEL[platform.tunAvailability] : undefined
          }
          onChange={(event) =>
            onChangeSettings({ tunEnabled: event.target.checked })
          }
        />
      </label>
      <i className="sb-sep" />
      <div className="sb-pop">
        <button
          type="button"
          className="sb-ports"
          onClick={() => {
            setPortsOpen(!portsOpen);
            setRouteOpen(false);
            setProxyOpen(false);
          }}
        >
          SOCKS {status?.socksPort ?? "—"} · HTTP {status?.httpPort ?? "—"}
        </button>
        {portsOpen ? (
          <div className="popover">
            <p>
              {t("本地")} SOCKS {status?.socksPort ?? "—"}
            </p>
            <p>
              {t("本地")} HTTP {status?.httpPort ?? "—"}
            </p>
            <p>Clash API {status?.clashApiPort ?? "—"}</p>
            <p>
              {t("局域网")}{" "}
              {settings?.allowLan === true
                ? `socks ${status?.socksPort ?? "—"} · http ${status?.httpPort ?? "—"}`
                : t("关闭")}
            </p>
          </div>
        ) : null}
      </div>
      <span className="sb-speeds">
        <span aria-label={t("上传速率")}>
          ↑ {formatRate(traffic.uploadBytesPerSecond)}
        </span>
        <span aria-label={t("下载速率")}>
          ↓ {formatRate(traffic.downloadBytesPerSecond)}
        </span>
      </span>
      <span className="sb-core">
        {status?.core ?? "—"} · {platform?.artifactIdentifier ?? "—"}
      </span>
    </footer>
  );
}
