import { formatBytes, ruleDraftFromConnection } from "../appHelpers";
import {
  connectionAge,
  ConnectionsView,
} from "../components/ConnectionsView";
import type { AppModel } from "../hooks/useAppController";
import {
  closeConnection,
  closeConnections,
  loadConnections,
} from "../session";
import { IconClose } from "../components/Icons";

export function ConnectionsPage({ app }: { app: AppModel }) {
  const {
    t,
    busy,
    connected,
    status,
    connections,
    connectionQuery,
    setConnectionQuery,
    setConnections,
    run,
    selectedConnectionId,
    setSelectedConnectionId,
    setRouteRuleKind,
    setRouteRuleValue,
    goTo,
    connectionProcess,
    connectionNetwork,
    connectionRule,
    connectionChain,
    setConnectionProcess,
    setConnectionNetwork,
    setConnectionRule,
    setConnectionChain,
  } = app;
  const selected =
    connections?.connections.find((item) => item.id === selectedConnectionId) ??
    null;

  return (
    <div className="page-connections">
      <ConnectionsView
        busy={busy}
        connected={connected}
        supported={status?.core !== "xray"}
        snapshot={connections}
        query={connectionQuery}
        t={t}
        onQuery={setConnectionQuery}
        onRefresh={() =>
          void loadConnections().then(setConnections, (failure: unknown) => {
            console.warn("connection refresh failed", failure);
          })
        }
        onClose={(id) =>
          void run(async () => {
            await closeConnection(id);
            setConnections(await loadConnections());
          })
        }
        onCloseAll={() =>
          void run(async () => {
            await closeConnections();
            setConnections(await loadConnections());
          })
        }
        selectedId={selectedConnectionId}
        onSelect={setSelectedConnectionId}
        processFilter={connectionProcess}
        networkFilter={connectionNetwork}
        ruleFilter={connectionRule}
        chainFilter={connectionChain}
        onProcessFilter={setConnectionProcess}
        onNetworkFilter={setConnectionNetwork}
        onRuleFilter={setConnectionRule}
        onChainFilter={setConnectionChain}
        onAddRule={(host) => {
          const draft = ruleDraftFromConnection(host);
          if (draft === null) {
            return;
          }
          setRouteRuleKind(draft.kind);
          setRouteRuleValue(draft.value);
          goTo("routing");
        }}
      />
      {selected ? (
        <aside className="drawer conn-drawer" aria-label={t("连接详情")}>
          <header className="drawer-head">
            <strong>{t("连接详情")}</strong>
            <button
              type="button"
              className="icon-btn"
              aria-label={t("关闭")}
              onClick={() => setSelectedConnectionId(null)}
            >
              <IconClose />
            </button>
          </header>
          <p className="mono">{selected.host}</p>
          <div className="inspect-row">
            <span>{t("目标")}</span>
            <strong className="mono">{selected.destination}</strong>
          </div>
          <div className="inspect-row">
            <span>{t("进程")}</span>
            <strong>{selected.process || "—"}</strong>
          </div>
          <div className="inspect-row">
            <span>{t("网络")}</span>
            <strong>{selected.network}</strong>
          </div>
          <div className="inspect-row">
            <span>{t("规则")}</span>
            <strong>{selected.rule || "—"}</strong>
          </div>
          <div className="inspect-row">
            <span>{t("链路")}</span>
            <strong>{selected.chain || "—"}</strong>
          </div>
          <div className="inspect-row">
            <span>{t("上传")}</span>
            <strong className="mono">{formatBytes(selected.uploadBytes)}</strong>
          </div>
          <div className="inspect-row">
            <span>{t("下载")}</span>
            <strong className="mono">{formatBytes(selected.downloadBytes)}</strong>
          </div>
          <div className="inspect-row">
            <span>{t("时长")}</span>
            <strong>{connectionAge(selected.start, Date.now())}</strong>
          </div>
          <p className="nav-group">{t("快速操作")} QUICK ACTIONS</p>
          <div className="drawer-actions">
            <button
              type="button"
              className="btn-primary"
              onClick={() => {
                const draft = ruleDraftFromConnection(selected.host);
                if (draft === null) {
                  return;
                }
                setRouteRuleKind(draft.kind);
                setRouteRuleValue(draft.value);
                goTo("routing");
              }}
            >
              {t("加规则")}
            </button>
            <button
              type="button"
              className="btn-secondary"
              onClick={() => {
                void navigator.clipboard.writeText(selected.host);
              }}
            >
              {t("复制主机名")}
            </button>
            <button
              type="button"
              className="btn-danger-ghost"
              onClick={() =>
                void run(async () => {
                  await closeConnection(selected.id);
                  setConnections(await loadConnections());
                  setSelectedConnectionId(null);
                })
              }
            >
              {t("关闭连接")}
            </button>
          </div>
        </aside>
      ) : null}
    </div>
  );
}