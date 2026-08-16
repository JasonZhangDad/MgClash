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

  it("dns is not a placeholder and binds to existing dns commands", () => {
    expect(dnsSource).toContain("onSaveDns");
    expect(dnsSource).toContain('aria-label={t("DNS 模式")}');
    expect(dnsSource).toContain('aria-label={t("启用 FakeIP")}');
    expect(dnsSource).toContain("setDnsDraft");
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
