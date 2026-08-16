import type { AppPage } from "../../appHelpers";
import type { AppModel } from "../../hooks/useAppController";
import {
  IconChart,
  IconGauge,
  IconGear,
  IconGlobe,
  IconLink,
  IconNodes,
  IconRoute,
  IconServer,
  IconShield,
  IconTerminal,
} from "../Icons";
import { Badge } from "../ui/Ui";

const NAV: {
  icon: typeof IconGauge;
  id: AppPage;
  en: string;
  zh: string;
}[] = [
  { id: "overview", zh: "概览", en: "Overview", icon: IconGauge },
  { id: "nodes", zh: "节点", en: "Nodes", icon: IconServer },
  { id: "proxies", zh: "当前代理", en: "Proxies", icon: IconNodes },
  { id: "connections", zh: "当前连接", en: "Connections", icon: IconLink },
  { id: "traffic", zh: "流量", en: "Traffic", icon: IconChart },
  { id: "logs", zh: "信息", en: "Logs", icon: IconTerminal },
];

const CONFIG: typeof NAV = [
  { id: "routing", zh: "路由", en: "Routing", icon: IconRoute },
  { id: "dns", zh: "DNS", en: "Resolver", icon: IconGlobe },
];

export function Sidebar({ app }: { app: AppModel }) {
  const { t, page, goTo, nodes, nodeGroups, connections, openSettings } = app;
  const isMac = app.platform?.artifactIdentifier.startsWith("macos") ?? true;
  const counts: Partial<Record<AppPage, number>> = {
    nodes: nodes.length,
    proxies: nodeGroups.length,
    connections: connections?.connections.length ?? 0,
  };

  return (
    <aside className="app-sidebar" aria-label={t("主导航")}>
      {isMac ? (
        <div className="traffic-lights" aria-hidden>
          <i className="tl red" />
          <i className="tl yellow" />
          <i className="tl green" />
        </div>
      ) : (
        <div className="win-chrome" aria-hidden />
      )}
      <div className="brand-row">
        <span className="brand-mark">
          <IconShield />
        </span>
        <strong>MgClash</strong>
      </div>
      <nav className="side-nav">
        {NAV.map((item) => {
          const Icon = item.icon;
          const count = counts[item.id];
          return (
            <button
              key={item.id}
              type="button"
              data-page={item.id}
              className={page === item.id ? "nav-item is-on" : "nav-item"}
              onClick={() => goTo(item.id)}
            >
              <Icon />
              <span className="nav-copy">
                <span className="nav-zh">{t(item.zh)}</span>
                <span className="nav-en" data-en={item.en} />
              </span>
              {count !== undefined && count > 0 ? (
                <Badge className="nav-count">{count}</Badge>
              ) : null}
            </button>
          );
        })}
        <p className="nav-group">{t("配置")} CONFIGURATION</p>
        {CONFIG.map((item) => {
          const Icon = item.icon;
          return (
            <button
              key={item.id}
              type="button"
              data-page={item.id}
              className={page === item.id ? "nav-item is-on" : "nav-item"}
              onClick={() => goTo(item.id)}
            >
              <Icon />
              <span className="nav-copy">
                <span className="nav-zh">{t(item.zh)}</span>
                <span className="nav-en" data-en={item.en} />
              </span>
            </button>
          );
        })}
      </nav>
      <div className="side-foot">
        <button
          type="button"
          data-page="settings"
          className={page === "settings" ? "nav-item is-on" : "nav-item"}
          onClick={() => openSettings("general")}
        >
          <IconGear />
          <span className="nav-copy">
            <span className="nav-zh">{t("设置")}</span>
            <span className="nav-en" data-en="Settings" />
          </span>
        </button>
      </div>
    </aside>
  );
}