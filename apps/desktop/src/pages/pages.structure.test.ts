import { describe, expect, it } from "vitest";

import routingSource from "./RoutingPage.tsx?raw";
import dnsSource from "./DnsPage.tsx?raw";
import settingsSource from "./SettingsPage.tsx?raw";

describe("handoff pages are real surfaces", () => {
  it("routing is not a placeholder and binds to existing route commands", () => {
    expect(routingSource).not.toMatch(/PlaceholderPages/);
    expect(routingSource).toContain("onSaveRoute");
    expect(routingSource).toContain("onAddRouteRule");
    expect(routingSource).toContain("onUpdateRuleSets");
    expect(routingSource).toContain('aria-label={t("路由规则列表")}');
    expect(routingSource).toContain("setRoutingMode");
    expect(routingSource).not.toMatch(/demoNodes|fakeServers|sampleLatency/);
  });

  it("routing is a rule table plus a rule-set table, not a stacked add form", () => {
    expect(routingSource).toContain("rule-grid");
    expect(routingSource).toContain("provider-grid");
    expect(routingSource).toContain("drag-handle");
    expect(routingSource).toContain("row-menu");
    expect(routingSource).toContain("添加规则");
    expect(routingSource).toContain("添加规则集");
    expect(routingSource).not.toMatch(
      /className="settings-form form-grid"[\s\S]*规则类型/,
    );
  });

  it("dns is not a placeholder and binds to existing dns commands", () => {
    expect(dnsSource).toContain("onSaveDns");
    expect(dnsSource).toContain('ariaLabel={t("DNS 模式")}');
    expect(dnsSource).toContain('label={t("启用 FakeIP")}');
    expect(dnsSource).toContain("setDnsDraft");
  });

  it("dns is two cards with a hint on every row, not a tabbed form", () => {
    expect(dnsSource).toContain("dns-basic");
    expect(dnsSource).toContain("dns-advanced");
    expect(dnsSource).toContain("dns-field-copy");
    expect(dnsSource).not.toContain("option-tabs");
    expect(dnsSource).not.toContain("dnsTab");
    expect(dnsSource).toContain("DoH 会加密查询，system 跟随系统设置");
    expect(dnsSource).toContain("按域名路由更准确，但部分应用不兼容");
  });

  it("settings is not a placeholder and covers AppSettings fields", () => {
    expect(settingsSource).toContain("onChangeSettings");
    expect(settingsSource).toContain('aria-label={t("启用 TUN")}');
    expect(settingsSource).toContain('aria-label={t("启动时自动连接")}');
    expect(settingsSource).toContain('aria-label={t("Core 选择")}');
    expect(settingsSource).toContain('aria-label={t("SOCKS 端口")}');
    expect(settingsSource).toContain("systemProxyMode");
    expect(settingsSource).toContain("hotkeyConnect");
    expect(settingsSource).not.toMatch(/PlaceholderPages/);
  });
});
