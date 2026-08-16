import { type DragEvent, type ReactNode, useState } from "react";

import {
  describeFailure,
  describeRuleSetCache,
  isGeoRule,
  moveWithinGroup,
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
import { IconGrip, IconMore, IconPlus } from "../components/Icons";
import {
  Badge,
  Card,
  IconButton,
  SegmentedControl,
  Toggle,
} from "../components/ui/Ui";

function outboundTone(outbound: RouteOutbound): "acc" | "dang" | "neutral" {
  if (outbound === "proxy") {
    return "acc";
  }
  if (outbound === "block") {
    return "dang";
  }
  return "neutral";
}

function RowMenu({
  label,
  open,
  onToggle,
  children,
}: {
  children: ReactNode;
  label: string;
  onToggle: () => void;
  open: boolean;
}) {
  return (
    <div className="row-menu">
      <IconButton label={label} onClick={onToggle}>
        <IconMore />
      </IconButton>
      <div className="row-menu-pop" hidden={!open} role="menu">
        {children}
      </div>
    </div>
  );
}

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
    onSaveRoute,
    onUpdateRuleSet,
    onUpdateRuleSets,
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
  const [openMenu, setOpenMenu] = useState<string | null>(null);
  const [addingRule, setAddingRule] = useState(false);
  const [addingProvider, setAddingProvider] = useState(false);

  const dropRule = (event: DragEvent<HTMLElement>, target: number) => {
    event.preventDefault();
    if (routeDraft === null || connected || busy) {
      return;
    }
    const from = Number(event.dataTransfer.getData("text/plain"));
    const rules = moveWithinGroup(
      routeDraft.rules,
      from,
      target,
      (left, right) => isGeoRule(left.kind) === isGeoRule(right.kind),
    );
    if (rules === routeDraft.rules) {
      return;
    }
    setRouteDraft({ ...routeDraft, rules });
    setRouteDirty(true);
  };

  return (
    <div className="page-pad routing-page" aria-label={app.t("路由规则")}>
      <Card className="mode-card">
        <div className="mode-card-row">
          <div>
            <strong>{t("路由模式")} Routing mode</strong>
            <p className="hint">
              {t("决定流量是走规则、全部走代理还是全部直连")}
            </p>
          </div>
          <SegmentedControl
            ariaLabel={t("路由模式")}
            value={status?.mode ?? "global"}
            disabled={busy || status === null}
            onChange={(mode) => void app.run(() => setRoutingMode(mode))}
            options={[
              { value: "global" as RoutingMode, label: t("全局") },
              { value: "rule" as RoutingMode, label: t("规则") },
              { value: "direct" as RoutingMode, label: t("直连") },
            ]}
          />
          <span className="card-vdiv" />
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
        {status ? (
          <div className="scheme-row">
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
              disabled={busy || connected || status.routeSchemes.length <= 1}
              onClick={() => {
                void (async () => {
                  if (!window.confirm(t("确定删除当前路由方案？"))) {
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
            <button
              type="button"
              className="btn-primary"
              disabled={busy || routeDraft === null || !routeDirty}
              onClick={() => void onSaveRoute()}
            >
              {t("保存路由")}
            </button>
          </div>
        ) : null}
      </Card>

      <Card className="rules-card">
        <header className="card-head">
          <strong>{t("规则")} Rules</strong>
          <p className="hint">{t("按顺序匹配，Geo 规则始终最后执行")}</p>
          <button
            type="button"
            className="btn-primary"
            disabled={busy}
            onClick={() => {
              if (routeRuleValue.trim() === "") {
                setAddingRule(true);
                return;
              }
              onAddRouteRule();
            }}
          >
            <IconPlus />{t("添加规则")}
          </button>
        </header>
        {routeDraft === null || status === null ? (
          <p className="hint">{t("正在读取路由设置")}</p>
        ) : (
          <>
            <div
              className="rule-composer"
              hidden={!addingRule && routeRuleValue.trim() === ""}
            >
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
            </div>

            {routeDraft.rules.length === 0 ? (
              <p className="hint">{t("尚未添加规则")}</p>
            ) : (
              <div
                className="rule-grid"
                role="table"
                aria-label={t("路由规则列表")}
              >
                <div role="row" className="grid-head">
                  <div role="columnheader" />
                  <div role="columnheader">{t("类型")} Type</div>
                  <div role="columnheader">{t("值")} Value</div>
                  <div role="columnheader">{t("出口")} Outbound</div>
                  <div role="columnheader">{t("状态")} State</div>
                  <div role="columnheader" />
                </div>
                {routeDraft.rules.map((rule, index) => (
                  <div
                    key={`${rule.kind}-${rule.value}-${index}`}
                    role="row"
                    data-row
                    className="grid-row"
                    onDragOver={(event) => event.preventDefault()}
                    onDrop={(event) => dropRule(event, index)}
                  >
                    <div role="cell">
                      <button
                        type="button"
                        className="drag-handle"
                        aria-label={t("拖拽调整顺序")}
                        draggable={!busy && !connected}
                        disabled={busy || connected}
                        onDragStart={(event) => {
                          event.dataTransfer.setData("text/plain", String(index));
                          event.dataTransfer.effectAllowed = "move";
                        }}
                      >
                        <IconGrip />
                      </button>
                    </div>
                    <div role="cell">
                      <Badge>{t(ROUTE_KIND_LABEL[rule.kind])}</Badge>
                    </div>
                    <div role="cell" className="mono">
                      {rule.value}
                    </div>
                    <div role="cell">
                      <Badge tone={outboundTone(rule.outbound)}>
                        {t(ROUTE_OUTBOUND_LABEL[rule.outbound])}
                      </Badge>
                    </div>
                    <div role="cell">
                      <Toggle
                        label={`启用规则 ${index + 1}`}
                        checked={rule.enabled}
                        disabled={busy}
                        onChange={(enabled) => {
                          const rules = [...routeDraft.rules];
                          rules[index] = { ...rule, enabled };
                          setRouteDraft({ ...routeDraft, rules });
                          setRouteDirty(true);
                        }}
                      />
                    </div>
                    <div role="cell">
                      <RowMenu
                        label={`${t("规则操作")} ${index + 1}`}
                        open={openMenu === `rule-${index}`}
                        onToggle={() =>
                          setOpenMenu((current) =>
                            current === `rule-${index}` ? null : `rule-${index}`,
                          )
                        }
                      >
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
                            setOpenMenu(null);
                          }}
                        >
                          {t("删除")}
                        </button>
                      </RowMenu>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </>
        )}
      </Card>

      <Card className="ruleset-card">
        <header className="card-head">
          <strong>{t("规则集")} Rule providers</strong>
          <button
            type="button"
            className="btn-secondary"
            aria-label={t("全部更新规则集")}
            disabled={busy || routeDraft === null || routeDraft.providers.length === 0}
            onClick={() => void onUpdateRuleSets()}
          >
            {t("全部更新")}
          </button>
          <button
            type="button"
            className="btn-secondary"
            disabled={busy}
            onClick={() => {
              if (providerName.trim() === "" || providerUrl.trim() === "") {
                setAddingProvider(true);
                return;
              }
              onAddRuleProvider();
            }}
          >
            {t("添加规则集")}
          </button>
        </header>
        {status?.core === "xray" ? (
          <p className="hint">{t("Xray 会忽略远程规则集，改用 sing-box 才会生效。")}</p>
        ) : null}
        {routeDraft === null || status === null ? (
          <p className="hint">{t("正在读取路由设置")}</p>
        ) : (
          <>
            <div
              className="rule-composer"
              aria-label={t("规则集")}
              hidden={
                !addingProvider &&
                providerName.trim() === "" &&
                providerUrl.trim() === ""
              }
            >
              <input
                aria-label={t("规则集名称")}
                disabled={busy}
                placeholder="ads"
                value={providerName}
                onChange={(event) => setProviderName(event.target.value)}
              />
              <input
                aria-label={t("规则集地址")}
                disabled={busy}
                placeholder="https://example.com/ads.srs"
                value={providerUrl}
                onChange={(event) => setProviderUrl(event.target.value)}
              />
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
            </div>

            {routeDraft.providers.length === 0 ? (
              <p className="hint">{t("尚未添加规则集")}</p>
            ) : (
              <div
                className="provider-grid"
                role="table"
                aria-label={t("规则集列表")}
              >
                <div role="row" className="grid-head">
                  <div role="columnheader">{t("名称")} Name</div>
                  <div role="columnheader">URL</div>
                  <div role="columnheader">{t("格式")} Format</div>
                  <div role="columnheader">{t("出口")} Outbound</div>
                  <div role="columnheader">{t("缓存")} Cache</div>
                  <div role="columnheader">{t("状态")}</div>
                </div>
                {routeDraft.providers.map((provider, index) => {
                  const cached = ruleSets.some(
                    (entry) => entry.name === provider.name && entry.cached,
                  );
                  return (
                    <div
                      key={`${provider.name}-${index}`}
                      role="row"
                      data-row
                      className="grid-row"
                    >
                      <div role="cell">{provider.name}</div>
                      <div role="cell" className="mono">
                        {provider.url}
                      </div>
                      <div role="cell">{provider.format}</div>
                      <div role="cell">
                        {t(ROUTE_OUTBOUND_LABEL[provider.outbound])}
                      </div>
                      <div role="cell">
                        {describeRuleSetCache(ruleSets, provider.name, t)}
                      </div>
                      <div role="cell">
                        <button
                          type="button"
                          className="status-action"
                          aria-label={`更新规则集 ${provider.name}`}
                          disabled={busy}
                          onClick={() => void onUpdateRuleSet(provider.name)}
                        >
                          <Badge tone={cached ? "ok" : "warn"}>
                            {cached ? t("已缓存") : t("待下载")}
                          </Badge>
                        </button>
                        <Toggle
                          label={`${t("启用规则集")} ${provider.name}`}
                          checked={provider.enabled}
                          disabled={busy}
                          onChange={(enabled) => {
                            const providers = [...routeDraft.providers];
                            providers[index] = { ...provider, enabled };
                            setRouteDraft({ ...routeDraft, providers });
                            setRouteDirty(true);
                          }}
                        />
                        <RowMenu
                          label={`${t("规则集操作")} ${provider.name}`}
                          open={openMenu === `set-${index}`}
                          onToggle={() =>
                            setOpenMenu((current) =>
                              current === `set-${index}` ? null : `set-${index}`,
                            )
                          }
                        >
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
                              setOpenMenu(null);
                            }}
                          >
                            {t("删除")}
                          </button>
                        </RowMenu>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </>
        )}
      </Card>
    </div>
  );
}
