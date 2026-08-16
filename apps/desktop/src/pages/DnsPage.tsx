import { type KeyboardEvent, type ReactNode, useState } from "react";

import type { AppModel } from "../hooks/useAppController";
import type { DnsMode, DnsStrategy, DnsTemplate } from "../session";
import { Card, SegmentedControl, Toggle } from "../components/ui/Ui";

function DnsField({
  label,
  hint,
  children,
}: {
  children: ReactNode;
  hint: string;
  label: string;
}) {
  return (
    <div className="dns-field">
      <div className="dns-field-copy">
        <strong>{label}</strong>
        <span>{hint}</span>
      </div>
      <div className="dns-field-control">{children}</div>
    </div>
  );
}

export function DnsPage({ app }: { app: AppModel }) {
  const {
    t,
    busy,
    dnsDraft,
    dnsDirty,
    onSaveDns,
    onRestartCore,
    setDnsDirty,
    setDnsDraft,
  } = app;
  const [domainDraft, setDomainDraft] = useState("");

  const addDomain = () => {
    if (dnsDraft === null) {
      return;
    }
    const value = domainDraft.trim();
    if (value === "") {
      return;
    }
    const current = dnsDraft.systemDomains
      .map((item) => item.trim())
      .filter((item) => item !== "");
    if (current.includes(value)) {
      setDomainDraft("");
      return;
    }
    setDnsDraft({ ...dnsDraft, systemDomains: [...current, value] });
    setDnsDirty(true);
    setDomainDraft("");
  };

  const hostCount = (dnsDraft?.hosts ?? "")
    .split("\n")
    .filter((line) => line.trim() !== "").length;

  return (
    <div className="page-pad dns-page" aria-label="DNS">
      <div className="dns-layout">
        <div className="dns-main">
          <Card className="dns-card dns-basic">
            <header className="card-head">
              <strong>{t("基础 Basic")}</strong>
              {dnsDraft ? (
                <SegmentedControl
                  ariaLabel={t("DNS 模板")}
                  value={dnsDraft.template}
                  disabled={busy}
                  onChange={(template) => {
                    const patch =
                      template === "advanced"
                        ? {
                            mode: "doh" as const,
                            server: "cloudflare-dns.com",
                            port: 443,
                            dohPath: "/dns-query",
                            bootstrap: "223.5.5.5",
                            fakeIpEnabled: true,
                          }
                        : {
                            mode: "system" as const,
                            bootstrap: "",
                            fakeIpEnabled: false,
                          };
                    setDnsDraft({ ...dnsDraft, ...patch, template });
                    setDnsDirty(true);
                  }}
                  options={[
                    { value: "simple" as DnsTemplate, label: t("简单模板") },
                    { value: "advanced" as DnsTemplate, label: t("高级模板") },
                  ]}
                />
              ) : null}
            </header>
            {dnsDraft === null ? (
              <p className="hint">{t("正在读取 DNS 设置")}</p>
            ) : (
              <div className="dns-card-body">
                <DnsField
                  label={`${t("解析方式")} DNS mode`}
                  hint={t("DoH 会加密查询，system 跟随系统设置")}
                >
                  <SegmentedControl
                    ariaLabel={t("DNS 模式")}
                    value={dnsDraft.mode}
                    disabled={busy}
                    onChange={(mode) => {
                      const upstream =
                        mode === "doh"
                          ? { port: 443, server: "cloudflare-dns.com" }
                          : mode === "dot"
                            ? {
                                port: 853,
                                server: "1dot1dot1dot1.cloudflare-dns.com",
                              }
                            : mode === "plainUdp" || mode === "plainTcp"
                              ? { port: 53, server: "1.1.1.1" }
                              : {};
                      setDnsDraft({ ...dnsDraft, ...upstream, mode });
                      setDnsDirty(true);
                    }}
                    options={[
                      { value: "system" as DnsMode, label: "system" },
                      { value: "plainUdp" as DnsMode, label: "UDP" },
                      { value: "plainTcp" as DnsMode, label: "TCP" },
                      { value: "doh" as DnsMode, label: "DoH" },
                      { value: "dot" as DnsMode, label: "DoT" },
                    ]}
                  />
                </DnsField>
                <DnsField
                  label={`${t("DNS 服务器")} Server`}
                  hint={t("上游解析地址")}
                >
                  <input
                    className="mono"
                    aria-label={t("DNS 服务器")}
                    disabled={busy || dnsDraft.mode === "system"}
                    value={dnsDraft.server}
                    onChange={(event) => {
                      setDnsDraft({ ...dnsDraft, server: event.target.value });
                      setDnsDirty(true);
                    }}
                  />
                </DnsField>
                <DnsField
                  label={`${t("解析策略")} Strategy`}
                  hint={t("IPv4 / IPv6 优先级")}
                >
                  <select
                    aria-label={t("DNS 地址策略")}
                    disabled={busy}
                    value={dnsDraft.strategy}
                    onChange={(event) => {
                      setDnsDraft({
                        ...dnsDraft,
                        strategy: event.target.value as DnsStrategy,
                      });
                      setDnsDirty(true);
                    }}
                  >
                    <option value="preferIpv4">{t("优先 IPv4")}</option>
                    <option value="preferIpv6">{t("优先 IPv6")}</option>
                    <option value="ipv4Only">{t("仅 IPv4")}</option>
                    <option value="ipv6Only">{t("仅 IPv6")}</option>
                  </select>
                </DnsField>
                <DnsField
                  label={t("启用 IPv6")}
                  hint={t("关闭后不会返回 AAAA 记录")}
                >
                  <Toggle
                    label={t("启用 IPv6 DNS")}
                    checked={dnsDraft.ipv6Enabled}
                    disabled={busy}
                    onChange={(ipv6Enabled) => {
                      setDnsDraft({ ...dnsDraft, ipv6Enabled });
                      setDnsDirty(true);
                    }}
                  />
                </DnsField>
              </div>
            )}
          </Card>

          <Card className="dns-card dns-advanced">
            <header className="card-head">
              <strong>{t("高级 Advanced")}</strong>
              <em className="warn-chip">{t("修改前请确认影响")}</em>
            </header>
            {dnsDraft === null ? (
              <p className="hint">{t("正在读取 DNS 设置")}</p>
            ) : (
              <div className="dns-card-body">
                <DnsField
                  label="Fake-IP"
                  hint={t("按域名路由更准确，但部分应用不兼容")}
                >
                  <Toggle
                    label={t("启用 FakeIP")}
                    checked={dnsDraft.fakeIpEnabled}
                    disabled={busy}
                    onChange={(fakeIpEnabled) => {
                      setDnsDraft({ ...dnsDraft, fakeIpEnabled });
                      setDnsDirty(true);
                    }}
                  />
                </DnsField>
                <DnsField
                  label={t("Bootstrap 服务器")}
                  hint={t("用于解析 DoH / DoT 域名本身")}
                >
                  <input
                    className="mono"
                    aria-label={t("Bootstrap DNS")}
                    disabled={busy || dnsDraft.mode === "system"}
                    placeholder="223.5.5.5"
                    value={dnsDraft.bootstrap}
                    onChange={(event) => {
                      setDnsDraft({ ...dnsDraft, bootstrap: event.target.value });
                      setDnsDirty(true);
                    }}
                  />
                </DnsField>
                <DnsField
                  label={t("走系统解析的域名")}
                  hint={t("这些域名不经过上游 DNS")}
                >
                  <div className="chip-list">
                    {dnsDraft.systemDomains
                      .map((item) => item.trim())
                      .filter((item) => item !== "")
                      .map((domain) => (
                        <span key={domain} className="dns-chip">
                          {domain}
                          <button
                            type="button"
                            aria-label={`${t("删除")} ${domain}`}
                            disabled={busy}
                            onClick={() => {
                              setDnsDraft({
                                ...dnsDraft,
                                systemDomains: dnsDraft.systemDomains.filter(
                                  (item) => item.trim() !== domain,
                                ),
                              });
                              setDnsDirty(true);
                            }}
                          >
                            ×
                          </button>
                        </span>
                      ))}
                    <input
                      className="mono chip-input"
                      aria-label={t("系统 DNS 域名后缀")}
                      disabled={busy}
                      placeholder={t("添加域名")}
                      value={domainDraft}
                      onChange={(event) => setDomainDraft(event.target.value)}
                      onKeyDown={(event: KeyboardEvent<HTMLInputElement>) => {
                        if (event.key === "Enter") {
                          event.preventDefault();
                          addDomain();
                        }
                      }}
                    />
                    <button
                      type="button"
                      className="chip-add"
                      disabled={busy}
                      onClick={addDomain}
                    >
                      {t("+ 添加")}
                    </button>
                  </div>
                </DnsField>
                <DnsField
                  label={t("DoH 路径")}
                  hint={t("DoH 请求的路径，通常是 /dns-query")}
                >
                  <input
                    className="mono"
                    aria-label={t("DoH 路径")}
                    disabled={busy || dnsDraft.mode !== "doh"}
                    value={dnsDraft.dohPath}
                    onChange={(event) => {
                      setDnsDraft({ ...dnsDraft, dohPath: event.target.value });
                      setDnsDirty(true);
                    }}
                  />
                </DnsField>
                <DnsField
                  label="Hosts"
                  hint={t("Hosts 覆盖")}
                >
                  <textarea
                    aria-label={t("DNS Hosts")}
                    disabled={busy}
                    rows={3}
                    placeholder={t("每行一条：example.com 1.2.3.4")}
                    value={dnsDraft.hosts}
                    onChange={(event) => {
                      setDnsDraft({ ...dnsDraft, hosts: event.target.value });
                      setDnsDirty(true);
                    }}
                  />
                </DnsField>
                <DnsField
                  label={t("DNS 端口")}
                  hint={t("监听端口")}
                >
                  <input
                    className="mono"
                    aria-label={t("DNS 端口")}
                    disabled={busy || dnsDraft.mode === "system"}
                    min="1"
                    max="65535"
                    type="number"
                    value={dnsDraft.port}
                    onChange={(event) => {
                      setDnsDraft({ ...dnsDraft, port: Number(event.target.value) });
                      setDnsDirty(true);
                    }}
                  />
                </DnsField>
              </div>
            )}
          </Card>
        </div>

        <Card className="dns-effect">
          <header className="card-head">
            <strong>{t("当前生效")} In effect</strong>
          </header>
          <dl className="kv-list">
            <div>
              <dt>{t("监听端口")}</dt>
              <dd className="mono">{dnsDraft?.port ?? "—"}</dd>
            </div>
            <div>
              <dt>{t("模板")}</dt>
              <dd>{dnsDraft?.template === "advanced" ? t("高级") : t("简易")}</dd>
            </div>
            <div>
              <dt>{t("Hosts 覆盖")}</dt>
              <dd className="mono">
                {hostCount} {t("条")}
              </dd>
            </div>
          </dl>
          <p className="hint">
            {t("修改 DNS 后需要重启内核才会生效。保存时会自动提示。")}
          </p>
          <div className="actions">
            <button
              type="button"
              disabled={busy || dnsDraft === null || !dnsDirty}
              onClick={() => void onSaveDns()}
            >
              {t("保存 DNS")}
            </button>
            <button
              type="button"
              className="btn-primary"
              disabled={busy || dnsDraft === null || !dnsDirty}
              onClick={() => {
                void onSaveDns().then((saved) => {
                  if (saved) {
                    onRestartCore();
                  }
                });
              }}
            >
              {t("保存并重启内核")}
            </button>
          </div>
        </Card>
      </div>
    </div>
  );
}
