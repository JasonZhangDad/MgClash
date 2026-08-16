import {
  describeFailure,
  describeRuleSetCache,
  isGeoRule,
  runtimeOrderedRoute,
  ROUTE_KIND_LABEL,
  ROUTE_OUTBOUND_LABEL,
} from "../appHelpers";
import type { AppModel } from "../hooks/useAppController";
import {
  createRouteScheme,
  deleteRouteScheme,
  setRouteScheme,
  setRoutingMode,
  type RouteOutbound,
  type RouteRuleKind,
  type RuleProviderFormat,
  type RoutingMode,
} from "../session";
import { Card, SegmentedControl } from "../components/ui/Ui";

export function RoutingPage({ app }: { app: AppModel }) {
  const {
    t,
    busy,
    connected,
    status,
    routeDraft,
    routeDirty,
    routeRuleKind,
    routeRuleValue,
    routeRuleOutbound,
    providerName,
    providerUrl,
    providerFormat,
    providerOutbound,
    ruleSets,
    onAddRouteRule,
    onAddRuleProvider,
    onMoveRouteRule,
    onOpenRouting,
    onSaveRoute,
    onUpdateRuleSet,
    onUpdateRuleSets,
    run,
    setBusy,
    setError,
    setProviderFormat,
    setProviderName,
    setProviderOutbound,
    setProviderUrl,
    setRouteDirty,
    setRouteDraft,
    setRouteRuleKind,
    setRouteRuleOutbound,
    setRouteRuleValue,
    setStatus,
  } = app;

  return (
    <div className="page-pad routing-page" aria-label={app.t("路由规则")}>
      <Card className="mode-card">
        <div className="mode-card-row">
          <div>
            <strong>{t("路由模式")} Routing mode</strong>
          </div>
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
          {routeDraft ? (
            <label className="final-out">
              <span className="field-label">{t("默认出口")} FINAL</span>
              <select
                aria-label={t("默认出口")}
                disabled={busy}
                value={routeDraft.finalOutbound}
                onChange={(event) => {
                  setRouteDraft({
                    ...routeDraft,
                    finalOutbound: event.target.value as RouteOutbound,
                  });
                  setRouteDirty(true);
                }}
              >
                <option value="proxy">{t("代理")}</option>
                <option value="direct">{t("直连")}</option>
              </select>
            </label>
          ) : null}
        </div>
      </Card>
      <Card className="rules-card">
        <header className="card-head">
          <div>
            <strong>{t("规则")}</strong>
            <p className="hint">
              {t("运行顺序固定为：本地安全规则 → 用户规则 → Geo 规则 → 默认出口。仅规则模式应用列表。")}
            </p>
          </div>
          <button
            type="button"
            className="btn-primary"
            disabled={busy}
            onClick={() => void onOpenRouting()}
          >
            {t("添加")}
          </button>
        </header>
        {routeDraft === null || status === null ? (
          <p className="hint">{t("正在读取路由设置")}</p>
        ) : (
          <>
            <div className="settings-form form-grid">
              <label>
                {t("路由方案")}
                <select
                  aria-label={t("路由方案")}
                  disabled={busy}
                  value={status.routeSchemeId}
                  onChange={(event) => {
                    void (async () => {
                      setBusy(true);
                      setError(null);
                      try {
                        const next = await setRouteScheme(event.target.value);
                        setStatus(next);
                        setRouteDraft(runtimeOrderedRoute(next.route));
                        setRouteDirty(false);
                      } catch (failure: unknown) {
                        setError(describeFailure(failure));
                      } finally {
                        setBusy(false);
                      }
                    })();
                  }}
                >
                  {status.routeSchemes.map((scheme) => (
                    <option key={scheme.id} value={scheme.id}>
                      {scheme.name}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <div className="actions">
              <button
                type="button"
                disabled={busy}
                onClick={() => {
                  void (async () => {
                    const name = window.prompt(t("新路由方案名称"), t("新方案"));
                    if (name === null || name.trim() === "") {
                      return;
                    }
                    setBusy(true);
                    setError(null);
                    try {
                      const next = await createRouteScheme(name.trim());
                      setStatus(next);
                      setRouteDraft(runtimeOrderedRoute(next.route));
                      setRouteDirty(false);
                    } catch (failure: unknown) {
                      setError(describeFailure(failure));
                    } finally {
                      setBusy(false);
                    }
                  })();
                }}
              >
                {t("新建方案")}
              </button>
              <button
                type="button"
                disabled={
                  busy ||
                  connected ||
                  status.routeSchemes.length <= 1
                }
                onClick={() => {
                  void (async () => {
                    if (
                      !window.confirm(
                        t("确定删除当前路由方案？"),
                      )
                    ) {
                      return;
                    }
                    setBusy(true);
                    setError(null);
                    try {
                      const next = await deleteRouteScheme(status.routeSchemeId);
                      setStatus(next);
                      setRouteDraft(runtimeOrderedRoute(next.route));
                      setRouteDirty(false);
                    } catch (failure: unknown) {
                      setError(describeFailure(failure));
                    } finally {
                      setBusy(false);
                    }
                  })();
                }}
              >
                {t("删除方案")}
              </button>
            </div>
            <div className="settings-form form-grid">
              <label>
                {t("规则类型")}
                <select
                  aria-label={t("规则类型")}
                  disabled={busy}
                  value={routeRuleKind}
                  onChange={(event) =>
                    setRouteRuleKind(event.target.value as RouteRuleKind)
                  }
                >
                  {Object.entries(ROUTE_KIND_LABEL).map(([value, label]) => (
                    <option key={value} value={value}>
                      {t(label)}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                {t("规则值")}
                <input
                  aria-label={t("规则值")}
                  disabled={busy}
                  placeholder={
                    routeRuleKind === "network"
                      ? "tcp 或 udp"
                      : routeRuleKind === "inbound"
                        ? "socks、http 或 tun"
                        : routeRuleKind === "protocol"
                          ? "http、tls 或 bittorrent"
                          : routeRuleKind === "processName"
                            ? "chrome"
                            : routeRuleKind === "processPath"
                              ? "/Applications/Safari.app"
                              : undefined
                  }
                  value={routeRuleValue}
                  onChange={(event) => setRouteRuleValue(event.target.value)}
                />
              </label>
              <label>
                {t("出口")}
                <select
                  aria-label={t("规则出口")}
                  disabled={busy}
                  value={routeRuleOutbound}
                  onChange={(event) =>
                    setRouteRuleOutbound(event.target.value as RouteOutbound)
                  }
                >
                  <option value="proxy">{t("代理")}</option>
                  <option value="direct">{t("直连")}</option>
                  <option value="block">{t("拦截")}</option>
                </select>
              </label>
            </div>
            <div className="actions">
              <button
                type="button"
                disabled={busy}
                onClick={onAddRouteRule}
              >
                {t("添加规则")}
              </button>
            </div>

            {routeDraft.rules.length === 0 ? (
              <p className="hint">{t("尚未添加规则")}</p>
            ) : (
              <table className="node-list" aria-label={t("路由规则列表")}>
                <thead>
                  <tr>
                    <th>{t("顺序")}</th>
                    <th>{t("类型")}</th>
                    <th>{t("值")}</th>
                    <th>{t("出口")}</th>
                    <th>{t("启用")}</th>
                    <th>{t("操作")}</th>
                  </tr>
                </thead>
                <tbody>
                  {routeDraft.rules.map((rule, index) => (
                    <tr key={`${rule.kind}-${rule.value}-${index}`}>
                      <td>{index + 1}</td>
                      <td>{t(ROUTE_KIND_LABEL[rule.kind])}</td>
                      <td>{rule.value}</td>
                      <td>
                        <em className={`ui-badge tone-${rule.outbound === "proxy" ? "acc" : rule.outbound === "block" ? "dang" : "neutral"}`}>
                          {t(ROUTE_OUTBOUND_LABEL[rule.outbound])}
                        </em>
                      </td>
                      <td>
                        <input
                          aria-label={`启用规则 ${index + 1}`}
                          checked={rule.enabled}
                          disabled={busy}
                          type="checkbox"
                          onChange={(event) => {
                            const rules = [...routeDraft.rules];
                            rules[index] = {
                              ...rule,
                              enabled: event.target.checked,
                            };
                            setRouteDraft({ ...routeDraft, rules });
                            setRouteDirty(true);
                          }}
                        />
                      </td>
                      <td className="node-actions">
                        <button
                          type="button"
                          disabled={
                            busy ||
                            connected ||
                            index === 0 ||
                            isGeoRule(rule.kind) !==
                              isGeoRule(routeDraft.rules[index - 1].kind)
                          }
                          onClick={() => onMoveRouteRule(index, -1)}
                        >
                          {t("上移")}
                        </button>
                        <button
                          type="button"
                          disabled={
                            busy ||
                            connected ||
                            index === routeDraft.rules.length - 1 ||
                            isGeoRule(rule.kind) !==
                              isGeoRule(routeDraft.rules[index + 1].kind)
                          }
                          onClick={() => onMoveRouteRule(index, 1)}
                        >
                          {t("下移")}
                        </button>
                        <button
                          type="button"
                          disabled={busy}
                          onClick={() => {
                            setRouteDraft({
                              ...routeDraft,
                              rules: routeDraft.rules.filter(
                                (_, ruleIndex) => ruleIndex !== index,
                              ),
                            });
                            setRouteDirty(true);
                          }}
                        >
                          {t("删除")}
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
            {status.core === "xray" ? (
              <p className="hint">{t("Xray 会忽略远程规则集，改用 sing-box 才会生效。")}</p>
            ) : null}
            <Card className="ruleset-card">
            <header className="card-head">
              <strong>{t("规则集")}</strong>
              <button
                type="button"
                disabled={busy || routeDraft.providers.length === 0}
                onClick={() => void onUpdateRuleSets()}
              >
                {t("全部更新")}
              </button>
            </header>
            <div className="settings-form form-grid" aria-label={t("规则集")}>
              <label>
                {t("规则集名称")}
                <input
                  aria-label={t("规则集名称")}
                  disabled={busy}
                  placeholder="ads"
                  value={providerName}
                  onChange={(event) => setProviderName(event.target.value)}
                />
              </label>
              <label>
                {t("规则集地址")}
                <input
                  aria-label={t("规则集地址")}
                  disabled={busy}
                  placeholder="https://example.com/ads.srs"
                  value={providerUrl}
                  onChange={(event) => setProviderUrl(event.target.value)}
                />
              </label>
              <label>
                {t("格式")}
                <select
                  aria-label={t("规则集格式")}
                  disabled={busy}
                  value={providerFormat}
                  onChange={(event) =>
                    setProviderFormat(event.target.value as RuleProviderFormat)
                  }
                >
                  <option value="binary">binary (.srs)</option>
                  <option value="source">source (.json)</option>
                </select>
              </label>
              <label>
                {t("出口")}
                <select
                  aria-label={t("规则集出口")}
                  disabled={busy}
                  value={providerOutbound}
                  onChange={(event) =>
                    setProviderOutbound(event.target.value as RouteOutbound)
                  }
                >
                  <option value="proxy">{t("代理")}</option>
                  <option value="direct">{t("直连")}</option>
                  <option value="block">{t("拦截")}</option>
                </select>
              </label>
            </div>
            <div className="actions">
              <button type="button" disabled={busy} onClick={onAddRuleProvider}>
                {t("添加规则集")}
              </button>
              <button
                type="button"
                disabled={busy || routeDraft.providers.length === 0}
                onClick={() => void onUpdateRuleSets()}
              >
                {t("全部更新规则集")}
              </button>
            </div>

            {routeDraft.providers.length === 0 ? (
              <p className="hint">{t("尚未添加规则集")}</p>
            ) : (
              <table className="node-list" aria-label={t("规则集列表")}>
                <thead>
                  <tr>
                    <th>{t("名称")}</th>
                    <th>{t("地址")}</th>
                    <th>{t("格式")}</th>
                    <th>{t("出口")}</th>
                    <th>{t("启用")}</th>
                    <th>{t("缓存")}</th>
                    <th>{t("操作")}</th>
                  </tr>
                </thead>
                <tbody>
                  {routeDraft.providers.map((provider, index) => (
                    <tr key={`${provider.name}-${index}`}>
                      <td>{provider.name}</td>
                      <td>{provider.url}</td>
                      <td>{provider.format}</td>
                      <td>
                        <em className={`ui-badge tone-${provider.outbound === "proxy" ? "acc" : provider.outbound === "block" ? "dang" : "neutral"}`}>
                          {t(ROUTE_OUTBOUND_LABEL[provider.outbound])}
                        </em>
                      </td>
                      <td>
                        <input
                          aria-label={`${t("启用规则集")} ${provider.name}`}
                          checked={provider.enabled}
                          disabled={busy}
                          type="checkbox"
                          onChange={(event) => {
                            const providers = [...routeDraft.providers];
                            providers[index] = {
                              ...provider,
                              enabled: event.target.checked,
                            };
                            setRouteDraft({ ...routeDraft, providers });
                            setRouteDirty(true);
                          }}
                        />
                      </td>
                      <td>
                        {describeRuleSetCache(ruleSets, provider.name, t)}
                        <em className={`ui-badge ${
                          ruleSets.some((entry) => entry.name === provider.name && entry.cached)
                            ? "tone-ok"
                            : "tone-warn"
                        }`}>
                          {ruleSets.some((entry) => entry.name === provider.name && entry.cached)
                            ? t("已缓存")
                            : t("待下载")}
                        </em>
                      </td>
                      <td className="node-actions">
                        <button
                          type="button"
                          aria-label={`更新规则集 ${provider.name}`}
                          disabled={busy}
                          onClick={() => void onUpdateRuleSet(provider.name)}
                        >
                          {t("更新")}
                        </button>
                        <button
                          type="button"
                          aria-label={`${t("删除规则集")} ${provider.name}`}
                          disabled={busy}
                          onClick={() => {
                            setRouteDraft({
                              ...routeDraft,
                              providers: routeDraft.providers.filter(
                                (_, providerIndex) => providerIndex !== index,
                              ),
                            });
                            setRouteDirty(true);
                          }}
                        >
                          {t("删除")}
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}

            <div className="settings-form form-grid">
              <label>
                {t("默认出口")}
                <select
                  aria-label={t("默认出口")}
                  disabled={busy}
                  value={routeDraft.finalOutbound}
                  onChange={(event) => {
                    setRouteDraft({
                      ...routeDraft,
                      finalOutbound: event.target.value as RouteOutbound,
                    });
                    setRouteDirty(true);
                  }}
                >
                  <option value="proxy">{t("代理")}</option>
                  <option value="direct">{t("直连")}</option>
                </select>
              </label>
            </div>
            </Card>
          </>
        )}
      </Card>

        <div className="actions">
          <button
            type="button"
            disabled={busy || routeDraft === null || !routeDirty}
            onClick={() => void onSaveRoute()}
          >
            {t("保存路由")}
          </button>
        </div>


    </div>
  );
}
