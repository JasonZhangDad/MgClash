import type { AppModel } from "../hooks/useAppController";
import type { DnsMode, DnsStrategy, DnsTemplate } from "../session";
import { Card, SegmentedControl } from "../components/ui/Ui";

export function DnsPage({ app }: { app: AppModel }) {
  const {
    t,
    busy,
    dnsDraft,
    dnsDirty,
    dnsTab,
    onSaveDns,
    onRestartCore,
    setDnsDirty,
    setDnsDraft,
    setDnsTab,
  } = app;

  return (
    <div className="page-pad dns-page" aria-label="DNS">
      <div className="dns-layout">
        <div className="dns-main">
        <Card className="dns-card">
          <header className="card-head">
            {/* Follows the open tab; a fixed "基础" title contradicted the
                tab bar right under it. */}
            <strong>
              {dnsTab === "basic" ? t("DNS 基础设置") : t("DNS 进阶设置")}
            </strong>
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
                  { value: "simple" as DnsTemplate, label: t("简易") },
                  { value: "advanced" as DnsTemplate, label: t("高级") },
                ]}
              />
            ) : null}
          </header>
        <div className="option-tabs" role="tablist">
          <button
            type="button"
            role="tab"
            className={dnsTab === "basic" ? "active" : undefined}
            onClick={() => setDnsTab("basic")}
          >
            {t("DNS 基础设置")}
          </button>
          <button
            type="button"
            role="tab"
            className={dnsTab === "advanced" ? "active" : undefined}
            onClick={() => setDnsTab("advanced")}
          >
            {t("DNS 进阶设置")}
          </button>
        </div>
        {dnsDraft === null ? (
          <p className="hint">{t("正在读取 DNS 设置")}</p>
        ) : (
          <div className="settings-form form-grid">
            <label>
              {t("模式")}
              <select
                aria-label={t("DNS 模式")}
                disabled={busy}
                value={dnsDraft.mode}
                onChange={(event) => {
                  const mode = event.target.value as DnsMode;
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
                  setDnsDraft((current) =>
                    current === null ? null : { ...current, ...upstream, mode },
                  );
                  setDnsDirty(true);
                }}
              >
                <option value="system">{t("系统 DNS")}</option>
                <option value="plainUdp">UDP</option>
                <option value="plainTcp">TCP</option>
                <option value="doh">DoH</option>
                <option value="dot">DoT</option>
              </select>
            </label>
            <label>
              {t("Bootstrap")}
              <input
                aria-label={t("Bootstrap DNS")}
                disabled={busy || dnsDraft.mode === "system"}
                placeholder="223.5.5.5"
                value={dnsDraft.bootstrap}
                onChange={(event) => {
                  setDnsDraft({ ...dnsDraft, bootstrap: event.target.value });
                  setDnsDirty(true);
                }}
              />
            </label>
            <label>
              {t("服务器")}
              <input
                aria-label={t("DNS 服务器")}
                disabled={busy || dnsDraft.mode === "system"}
                value={dnsDraft.server}
                onChange={(event) => {
                  setDnsDraft({ ...dnsDraft, server: event.target.value });
                  setDnsDirty(true);
                }}
              />
            </label>
            <label>
              {t("端口")}
              <input
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
            </label>
            <label>
              {t("DoH 路径")}
              <input
                aria-label={t("DoH 路径")}
                disabled={busy || dnsDraft.mode !== "doh"}
                value={dnsDraft.dohPath}
                onChange={(event) => {
                  setDnsDraft({ ...dnsDraft, dohPath: event.target.value });
                  setDnsDirty(true);
                }}
              />
            </label>
            <label>
              {t("地址策略")}
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
            </label>
            <label>
              {t("Hosts")}
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
            </label>
            <label>
              {t("系统 DNS 域名后缀")}
              <textarea
                aria-label={t("系统 DNS 域名后缀")}
                disabled={busy}
                rows={2}
                placeholder={t("每行一个，例如 lan")}
                value={dnsDraft.systemDomains.join("\n")}
                onChange={(event) => {
                  setDnsDraft({
                    ...dnsDraft,
                    systemDomains: event.target.value.split("\n"),
                  });
                  setDnsDirty(true);
                }}
              />
            </label>
            <label className="checkbox-label">
              <input
                aria-label={t("启用 IPv6 DNS")}
                checked={dnsDraft.ipv6Enabled}
                disabled={busy}
                type="checkbox"
                onChange={(event) => {
                  setDnsDraft({ ...dnsDraft, ipv6Enabled: event.target.checked });
                  setDnsDirty(true);
                }}
              />
              IPv6
            </label>
            <label className="checkbox-label">
              <input
                aria-label={t("启用 FakeIP")}
                checked={dnsDraft.fakeIpEnabled}
                disabled={busy}
                type="checkbox"
                onChange={(event) => {
                  setDnsDraft({ ...dnsDraft, fakeIpEnabled: event.target.checked });
                  setDnsDirty(true);
                }}
              />
              FakeIP
            </label>
          </div>
        )}

        <div className="actions">
          <button
            type="button"
            disabled={busy || dnsDraft === null || !dnsDirty}
            onClick={() => void onSaveDns()}
          >
            {t("保存 DNS")}
          </button>
        </div>
        </Card>

        </div>
        <Card className="dns-effect">
          <header className="card-head">
            <strong>{t("当前生效")} In effect</strong>
          </header>
          <dl className="kv-list">
            <div>
              <dt>{t("DNS 端口")}</dt>
              <dd className="mono">{dnsDraft?.port ?? "—"}</dd>
            </div>
            <div>
              <dt>{t("模板")}</dt>
              <dd>{dnsDraft?.template === "advanced" ? t("高级") : t("简易")}</dd>
            </div>
            <div>
              <dt>Hosts</dt>
              <dd className="mono">
                {(dnsDraft?.hosts ?? "").split("\n").filter((line) => line.trim() !== "").length}
              </dd>
            </div>
          </dl>
          <p className="hint">{t("修改 DNS 后需要重启内核才会生效")}</p>
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
        </Card>
      </div>
    </div>
  );
}
