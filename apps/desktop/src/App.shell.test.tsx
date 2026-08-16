// @vitest-environment jsdom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const loadPlatformSummaryMock = vi.hoisted(() => vi.fn());
const loadSessionStatusMock = vi.hoisted(() => vi.fn());
const loadTrafficMock = vi.hoisted(() => vi.fn());
const loadNodeTrafficMock = vi.hoisted(() => vi.fn());
const loadNodesMock = vi.hoisted(() => vi.fn());
const loadNodeGroupsMock = vi.hoisted(() => vi.fn());
const loadLogsMock = vi.hoisted(() => vi.fn());
const loadAppSettingsMock = vi.hoisted(() => vi.fn());
const loadSubscriptionsMock = vi.hoisted(() => vi.fn());
const loadSystemProxyStartupStatusMock = vi.hoisted(() => vi.fn());
const loadCoreReadinessMock = vi.hoisted(() => vi.fn());
const loadNodeDraftMock = vi.hoisted(() => vi.fn());
const connectSessionMock = vi.hoisted(() => vi.fn());
const disconnectSessionMock = vi.hoisted(() => vi.fn());
const testNodeMock = vi.hoisted(() => vi.fn());
const loadConnectionsMock = vi.hoisted(() => vi.fn());
const loadRuleSetsMock = vi.hoisted(() => vi.fn());
const setDnsSettingsMock = vi.hoisted(() => vi.fn());
const setNodeGroupMock = vi.hoisted(() => vi.fn());
const syncGlobalHotkeysMock = vi.hoisted(() => vi.fn());
const clearGlobalHotkeysMock = vi.hoisted(() => vi.fn());
const refreshAllSubscriptionsMock = vi.hoisted(() => vi.fn());
const updateSubscriptionMock = vi.hoisted(() => vi.fn());

vi.mock("./globalHotkeys", () => ({
  clearGlobalHotkeys: clearGlobalHotkeysMock,
  syncGlobalHotkeys: syncGlobalHotkeysMock,
}));

vi.mock("./platform", () => ({
  loadPlatformSummary: loadPlatformSummaryMock,
}));

vi.mock("./session", async () => {
  const actual = await vi.importActual<typeof import("./session")>("./session");
  return {
    ...actual,
    loadSessionStatus: loadSessionStatusMock,
    loadTraffic: loadTrafficMock,
    loadNodeTraffic: loadNodeTrafficMock,
    loadNodes: loadNodesMock,
    loadNodeGroups: loadNodeGroupsMock,
    loadLogs: loadLogsMock,
    loadAppSettings: loadAppSettingsMock,
    loadSystemProxyStartupStatus: loadSystemProxyStartupStatusMock,
    loadCoreReadiness: loadCoreReadinessMock,
    loadNodeDraft: loadNodeDraftMock,
    connectSession: connectSessionMock,
    disconnectSession: disconnectSessionMock,
    testNode: testNodeMock,
    loadConnections: loadConnectionsMock,
    loadRuleSets: loadRuleSetsMock,
    setDnsSettings: setDnsSettingsMock,
    setNodeGroup: setNodeGroupMock,
  };
});

vi.mock("./subscriptions", () => ({
  loadSubscriptions: loadSubscriptionsMock,
  createSubscription: vi.fn(),
  deleteSubscription: vi.fn(),
  refreshSubscription: vi.fn(),
  refreshAllSubscriptions: refreshAllSubscriptionsMock,
  updateSubscription: updateSubscriptionMock,
}));

import { formatClock } from "./appHelpers";
import App from "./App";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

const NODE = {
  deletable: true,
  enabled: true,
  frontNodeId: null,
  groupId: null,
  id: "00000000-0000-4000-8000-000000000001",
  lastTestedAt: null,
  latencyMs: 42,
  name: "Tokyo Edge",
  port: 8388,
  protocol: "shadowsocks" as const,
  server: "edge.example.com",
  transport: "tcp",
  tls: null,
};

const SETTINGS = {
  closeToTray: true,
  connectOnLaunch: false,
  corePreference: "auto" as const,
  launchAtLogin: false,
  tunEnabled: false,
  logLevel: "info" as const,
  systemProxyMode: "managed" as const,
  locale: "zh-Hans" as const,
  socksPort: 10808,
  httpPort: 10809,
  clashApiPort: 9090,
  muxEnabled: false,
  fragmentEnabled: false,
  finalFragmentEnabled: false,
  udpNoiseEnabled: false,
  autoSelectLowestLatency: false,
  urlTestAddress: "https://www.gstatic.com/generate_204",
  urlTestIntervalSeconds: 180,
  urlTestToleranceMs: 50,
  allowLan: false,
  speedTestUrl: "https://speed.cloudflare.com/__down?bytes=10000000",
  inboundUdpEnabled: true,
  defAllowInsecure: false,
  defFingerprint: "",
  hotkeyConnect: "Ctrl+Enter",
  hotkeyPrevious: "Ctrl+[",
  hotkeyNext: "Ctrl+]",
  configTemplate: "",
  configOverride: "",
};

describe("redesigned shell", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    loadPlatformSummaryMock.mockResolvedValue({
      artifactIdentifier: "macos-x86_64",
      tunAvailability: "unavailableInUnsignedBuild",
    });
    loadSessionStatusMock.mockResolvedValue({
      connected: false,
      core: "sing-box",
      dns: {
        bootstrap: "",
        dohPath: "/dns-query",
        fakeIpEnabled: false,
        hosts: "",
        ipv6Enabled: false,
        mode: "system",
        port: 53,
        server: "1.1.1.1",
        strategy: "preferIpv4",
        systemDomains: [],
        template: "simple",
      },
      httpPort: 10809,
      mode: "global",
      route: { finalOutbound: "proxy", providers: [], rules: [] },
      routeSchemeId: "default",
      routeSchemes: [{ id: "default", name: "默认" }],
      node: NODE,
      socksPort: 10808,
      clashApiPort: 9090,
      systemProxy: true,
      systemProxyMode: "managed",
    });
    loadTrafficMock.mockResolvedValue({
      downloadBytesPerSecond: 100,
      monthBytes: 0,
      todayBytes: 10,
      totalBytes: 20,
      uploadBytesPerSecond: 50,
    });
    loadNodeTrafficMock.mockResolvedValue({});
    loadNodesMock.mockResolvedValue([NODE]);
    loadNodeGroupsMock.mockResolvedValue([]);
    loadLogsMock.mockResolvedValue([]);
    loadAppSettingsMock.mockResolvedValue(SETTINGS);
    loadSubscriptionsMock.mockResolvedValue([]);
    refreshAllSubscriptionsMock.mockReset();
    refreshAllSubscriptionsMock.mockResolvedValue([]);
    updateSubscriptionMock.mockReset();
    setNodeGroupMock.mockReset();
    setNodeGroupMock.mockResolvedValue([NODE]);
    loadSystemProxyStartupStatusMock.mockResolvedValue("clean");
    loadCoreReadinessMock.mockResolvedValue({ singBox: true, xray: true });
    loadNodeDraftMock.mockResolvedValue({
      credential: { method: "aes-256-gcm", password: "x", protocol: "shadowsocks" },
      name: NODE.name,
      port: NODE.port,
      server: NODE.server,
      tls: null,
      transport: { type: "tcp" },
      udpEnabled: true,
    });
    loadConnectionsMock.mockResolvedValue({
      connections: [],
      downloadTotalBytes: 0,
      uploadTotalBytes: 0,
    });
    loadRuleSetsMock.mockResolvedValue([]);
    setDnsSettingsMock.mockReset();
    setDnsSettingsMock.mockImplementation(async (settings) => ({
      connected: false,
      core: "sing-box",
      dns: settings,
      httpPort: 10809,
      mode: "global",
      route: { finalOutbound: "proxy", providers: [], rules: [] },
      routeSchemeId: "default",
      routeSchemes: [{ id: "default", name: "默认" }],
      node: NODE,
      socksPort: 10808,
      clashApiPort: 9090,
      systemProxy: true,
      systemProxyMode: "managed",
    }));
    syncGlobalHotkeysMock.mockResolvedValue(false);
    clearGlobalHotkeysMock.mockResolvedValue(undefined);
    connectSessionMock.mockReset();
    disconnectSessionMock.mockReset();
    disconnectSessionMock.mockResolvedValue({
      connected: false,
      core: "sing-box",
      dns: {
        bootstrap: "",
        dohPath: "/dns-query",
        fakeIpEnabled: false,
        hosts: "",
        ipv6Enabled: false,
        mode: "system",
        port: 53,
        server: "1.1.1.1",
        strategy: "preferIpv4",
        systemDomains: [],
        template: "simple",
      },
      httpPort: 10809,
      mode: "global",
      route: { finalOutbound: "proxy", providers: [], rules: [] },
      routeSchemeId: "default",
      routeSchemes: [{ id: "default", name: "默认" }],
      node: NODE,
      socksPort: 10808,
      clashApiPort: 9090,
      systemProxy: true,
      systemProxyMode: "managed",
    });
    connectSessionMock.mockResolvedValue({
      connected: true,
      core: "sing-box",
      dns: {
        bootstrap: "",
        dohPath: "/dns-query",
        fakeIpEnabled: false,
        hosts: "",
        ipv6Enabled: false,
        mode: "system",
        port: 53,
        server: "1.1.1.1",
        strategy: "preferIpv4",
        systemDomains: [],
        template: "simple",
      },
      httpPort: 10809,
      mode: "global",
      route: { finalOutbound: "proxy", providers: [], rules: [] },
      routeSchemeId: "default",
      routeSchemes: [{ id: "default", name: "默认" }],
      node: NODE,
      socksPort: 10808,
      clashApiPort: 9090,
      systemProxy: true,
      systemProxyMode: "managed",
    });
    testNodeMock.mockResolvedValue({
      id: NODE.id,
      latencyMs: 12,
      status: "success",
    });
    localStorage.clear();
    container = document.createElement("div");
    document.body.append(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  async function render(): Promise<void> {
    await act(async () => root.render(<App />));
  }

  /// Polls until `ready` holds. The initial loads resolve over several promise
  /// chains, so a single `act` flush is not enough to guarantee the data has
  /// landed — on a slow runner it observably has not.
  async function settle(ready: () => boolean): Promise<void> {
    for (let attempt = 0; attempt < 50 && !ready(); attempt += 1) {
      await act(async () => {
        await Promise.resolve();
      });
    }
  }

  it("opens overview connect and the node inspector", async () => {
    await render();
    expect(
      container.querySelector(".nav-item[data-page='overview']")?.className,
    ).toContain("is-on");
    await act(async () => {
      [...container.querySelectorAll("button")]
        .find((button) => button.textContent === "连接")
        ?.click();
    });
    expect(connectSessionMock).toHaveBeenCalled();

    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='nodes']")
        ?.click();
    });
    const row = container.querySelector("[aria-label='节点列表'] tbody tr");
    await act(async () => {
      row?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    const inspector = container.querySelector("[aria-label='节点详情']");
    expect(inspector).not.toBeNull();
    for (const label of ["认证", "传输协议", "TLS", "高级", "链路", "诊断", "概览"]) {
      await act(async () => {
        [...(inspector?.querySelectorAll("button") ?? [])]
          .find((button) => button.textContent === label)
          ?.click();
      });
    }
    await act(async () => {
      [...container.querySelectorAll("button")]
        .find((button) => button.textContent === "切换节点")
        ?.click();
      [...container.querySelectorAll("button")]
        .find((button) => button.textContent === "测试延迟")
        ?.click();
      [...container.querySelectorAll("button")]
        .find((button) => button.textContent === "重启内核")
        ?.click();
    });
    expect(testNodeMock).toHaveBeenCalled();
    await act(async () => {
      [...container.querySelectorAll<HTMLButtonElement>(".page-tabs button")]
        .find((button) => button.textContent?.includes("分组"))
        ?.click();
      [...container.querySelectorAll<HTMLButtonElement>(".page-tabs button")]
        .find((button) => button.textContent?.includes("订阅"))
        ?.click();
    });
  });

  it("opens the command palette and the traffic page", async () => {
    await render();
    await act(async () => {
      document.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "k",
          metaKey: true,
          bubbles: true,
        }),
      );
    });
    const palette = container.querySelector("[aria-label='命令面板']");
    expect(palette).not.toBeNull();
    await act(async () => {
      palette?.dispatchEvent(
        new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }),
      );
      palette?.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", bubbles: true }),
      );
    });
    await act(async () => {
      document.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "k",
          metaKey: true,
          bubbles: true,
        }),
      );
    });
    await act(async () => {
      [...container.querySelectorAll<HTMLButtonElement>(".palette-body button")]
        .find((button) => button.textContent?.includes("全部测速"))
        ?.click();
    });
    await act(async () => {
      document.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
      );
    });
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='traffic']")
        ?.click();
    });
    expect(container.textContent).toContain("历史流量统计");
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='connections']")
        ?.click();
    });
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='routing']")
        ?.click();
    });
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='dns']")
        ?.click();
    });
    await act(async () => {
      container.querySelector<HTMLButtonElement>(".sb-ports")?.click();
      container
        .querySelector<HTMLButtonElement>("[aria-label='浅色主题'], [aria-label='深色主题']")
        ?.click();
      container.querySelector<HTMLButtonElement>(".palette-trigger")?.click();
    });
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='settings']")
        ?.click();
    });
  });

  it("inspects a live connection and copies a rule draft", async () => {
    loadSessionStatusMock.mockResolvedValue({
      connected: true,
      core: "sing-box",
      dns: {
        bootstrap: "",
        dohPath: "/dns-query",
        fakeIpEnabled: false,
        hosts: "",
        ipv6Enabled: false,
        mode: "system",
        port: 53,
        server: "1.1.1.1",
        strategy: "preferIpv4",
        systemDomains: [],
        template: "simple",
      },
      httpPort: 10809,
      mode: "global",
      route: { finalOutbound: "proxy", providers: [], rules: [] },
      routeSchemeId: "default",
      routeSchemes: [{ id: "default", name: "默认" }],
      node: NODE,
      socksPort: 10808,
      clashApiPort: 9090,
      systemProxy: true,
      systemProxyMode: "managed",
    });
    loadConnectionsMock.mockResolvedValue({
      downloadTotalBytes: 10,
      uploadTotalBytes: 4,
      connections: [
        {
          id: "c1",
          host: "cdn.example.com",
          destination: "1.1.1.1:443",
          network: "tcp",
          process: "Safari",
          rule: "geosite",
          chain: "proxy",
          uploadBytes: 4,
          downloadBytes: 10,
          start: "2026-08-13T00:00:00Z",
        },
      ],
    });
    await render();
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='connections']")
        ?.click();
    });
    await act(async () => {
      container
        .querySelector<HTMLTableRowElement>("[aria-label='连接列表'] tbody tr")
        ?.click();
    });
    expect(container.querySelector("[aria-label='连接详情']")).not.toBeNull();
    await act(async () => {
      [...container.querySelectorAll("button")]
        .find((button) => button.textContent === "加规则")
        ?.click();
    });
  });

  it("shows only the selected settings section", async () => {
    await render();
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='settings']")
        ?.click();
    });
    expect(
      container.querySelector("[data-settings-section='general']")?.hasAttribute(
        "hidden",
      ),
    ).toBe(false);
    expect(
      container.querySelector("[data-settings-section='core']")?.hasAttribute(
        "hidden",
      ),
    ).toBe(true);
    expect(
      container.querySelector("[data-settings-section='network']")?.hasAttribute(
        "hidden",
      ),
    ).toBe(true);
    expect(
      container.querySelector("[data-settings-section='tun']")?.hasAttribute(
        "hidden",
      ),
    ).toBe(true);
    await act(async () => {
      [...container.querySelectorAll<HTMLButtonElement>(".settings-nav-item")]
        .find((button) => button.textContent === "内核")
        ?.click();
    });
    expect(
      container.querySelector("[data-settings-section='general']")?.hasAttribute(
        "hidden",
      ),
    ).toBe(true);
    expect(
      container.querySelector("[data-settings-section='core']")?.hasAttribute(
        "hidden",
      ),
    ).toBe(false);
    expect(
      container.querySelector("[data-settings-section='network']")?.hasAttribute(
        "hidden",
      ),
    ).toBe(true);
  });

  it("filters the connection table from snapshot values", async () => {
    loadSessionStatusMock.mockResolvedValue({
      connected: true,
      core: "sing-box",
      dns: {
        bootstrap: "",
        dohPath: "/dns-query",
        fakeIpEnabled: false,
        hosts: "",
        ipv6Enabled: false,
        mode: "system",
        port: 53,
        server: "1.1.1.1",
        strategy: "preferIpv4",
        systemDomains: [],
        template: "simple",
      },
      httpPort: 10809,
      mode: "global",
      route: { finalOutbound: "proxy", providers: [], rules: [] },
      routeSchemeId: "default",
      routeSchemes: [{ id: "default", name: "默认" }],
      node: NODE,
      socksPort: 10808,
      clashApiPort: 9090,
      systemProxy: true,
      systemProxyMode: "managed",
    });
    loadConnectionsMock.mockResolvedValue({
      downloadTotalBytes: 20,
      uploadTotalBytes: 8,
      connections: [
        {
          id: "c1",
          host: "cdn.example.com",
          destination: "1.1.1.1:443",
          network: "tcp",
          process: "Safari",
          rule: "geosite",
          chain: "proxy",
          uploadBytes: 4,
          downloadBytes: 10,
          start: "2026-08-13T00:00:00Z",
        },
        {
          id: "c2",
          host: "api.example.com",
          destination: "1.1.1.1:443",
          network: "udp",
          process: "Chrome",
          rule: "direct",
          chain: "direct",
          uploadBytes: 4,
          downloadBytes: 10,
          start: "2026-08-13T00:00:00Z",
        },
      ],
    });
    await render();
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='connections']")
        ?.click();
    });
    await settle(
      () =>
        container.querySelectorAll("[aria-label='连接列表'] tbody tr").length ===
        2,
    );
    const processFilter = container.querySelector<HTMLSelectElement>(
      "select[aria-label='进程']",
    );
    expect(processFilter).not.toBeNull();
    await act(async () => {
      const setter = Object.getOwnPropertyDescriptor(
        HTMLSelectElement.prototype,
        "value",
      )?.set;
      setter?.call(processFilter, "Safari");
      processFilter?.dispatchEvent(new Event("change", { bubbles: true }));
    });
    const rows = [
      ...container.querySelectorAll("[aria-label='连接列表'] tbody tr"),
    ];
    expect(rows).toHaveLength(1);
    expect(rows[0]?.textContent).toContain("Safari");
    expect(rows[0]?.textContent).not.toContain("Chrome");
  });

  it("restarts the core only after DNS settings have been saved", async () => {
    let finishSave: ((value: unknown) => void) | undefined;
    setDnsSettingsMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          finishSave = resolve;
        }),
    );
    loadSessionStatusMock.mockResolvedValue({
      connected: true,
      core: "sing-box",
      dns: {
        bootstrap: "",
        dohPath: "/dns-query",
        fakeIpEnabled: false,
        hosts: "",
        ipv6Enabled: false,
        mode: "system",
        port: 53,
        server: "1.1.1.1",
        strategy: "preferIpv4",
        systemDomains: [],
        template: "simple",
      },
      httpPort: 10809,
      mode: "global",
      route: { finalOutbound: "proxy", providers: [], rules: [] },
      routeSchemeId: "default",
      routeSchemes: [{ id: "default", name: "默认" }],
      node: NODE,
      socksPort: 10808,
      clashApiPort: 9090,
      systemProxy: true,
      systemProxyMode: "managed",
    });
    disconnectSessionMock.mockResolvedValue({
      connected: false,
      core: "sing-box",
      dns: {
        bootstrap: "",
        dohPath: "/dns-query",
        fakeIpEnabled: true,
        hosts: "",
        ipv6Enabled: true,
        mode: "system",
        port: 53,
        server: "1.1.1.1",
        strategy: "preferIpv4",
        systemDomains: [],
        template: "simple",
      },
      httpPort: 10809,
      mode: "global",
      route: { finalOutbound: "proxy", providers: [], rules: [] },
      routeSchemeId: "default",
      routeSchemes: [{ id: "default", name: "默认" }],
      node: NODE,
      socksPort: 10808,
      clashApiPort: 9090,
      systemProxy: true,
      systemProxyMode: "managed",
    });
    await render();
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='dns']")
        ?.click();
    });
    await act(async () => {
      container
        .querySelector<HTMLInputElement>("[aria-label='启用 IPv6 DNS']")
        ?.click();
    });
    await act(async () => {
      [...container.querySelectorAll("button")]
        .find((button) => button.textContent === "保存并重启内核")
        ?.click();
    });
    expect(setDnsSettingsMock).toHaveBeenCalled();
    expect(disconnectSessionMock).not.toHaveBeenCalled();
    await act(async () => {
      finishSave?.({
        connected: true,
        core: "sing-box",
        dns: {
          bootstrap: "",
          dohPath: "/dns-query",
          fakeIpEnabled: false,
          hosts: "",
          ipv6Enabled: true,
          mode: "system",
          port: 53,
          server: "1.1.1.1",
          strategy: "preferIpv4",
          systemDomains: [],
          template: "simple",
        },
        httpPort: 10809,
        mode: "global",
        route: { finalOutbound: "proxy", providers: [], rules: [] },
        routeSchemeId: "default",
        routeSchemes: [{ id: "default", name: "默认" }],
        node: NODE,
        socksPort: 10808,
        clashApiPort: 9090,
        systemProxy: true,
        systemProxyMode: "managed",
      });
    });
    expect(disconnectSessionMock).toHaveBeenCalled();
    expect(connectSessionMock).toHaveBeenCalled();
  });

  it("shows the designed nodes toolbar, row fields, and selection actions", async () => {
    loadNodeGroupsMock.mockResolvedValue([
      { id: "g1", name: "Work", strategy: "select" },
    ]);
    loadNodeTrafficMock.mockResolvedValue({
      [NODE.id]: {
        todayDownloadBytes: 100,
        todayUploadBytes: 20,
        totalDownloadBytes: 400,
        totalUploadBytes: 80,
      },
    });
    await render();
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='nodes']")
        ?.click();
    });
    expect(
      container.querySelector("[aria-label='节点分组筛选']"),
    ).not.toBeNull();
    expect(container.querySelector("[aria-label='排序']")).not.toBeNull();
    expect(container.querySelector("[aria-label='列表视图']")).not.toBeNull();
    expect(container.querySelector("[aria-label='卡片视图']")).not.toBeNull();
    expect(
      [...container.querySelectorAll("button")].some((button) =>
        button.textContent?.includes("添加节点"),
      ),
    ).toBe(true);
    expect(
      [...container.querySelectorAll("button")].some((button) =>
        button.textContent?.includes("测试延迟"),
      ),
    ).toBe(true);
    const headers = [
      ...container.querySelectorAll("[aria-label='节点列表'] thead th"),
    ].map((cell) => cell.textContent);
    expect(headers).toEqual(
      expect.arrayContaining(["别名", "协议", "地址", "分组", "延迟", "今日流量", "累计流量"]),
    );
    const row = container.querySelector("[aria-label='节点列表'] tbody tr");
    expect(row?.textContent).toContain("Tokyo Edge");
    expect(row?.textContent).toContain("shadowsocks");
    expect(row?.textContent).toContain("edge.example.com:8388");
    expect(row?.querySelector(".status-dot")).not.toBeNull();
    const check = row?.querySelector<HTMLInputElement>("input[type='checkbox']");
    await act(async () => check?.click());
    const bar = container.querySelector("[aria-label='已选']");
    expect(bar).not.toBeNull();
    expect(bar?.textContent).toContain("测试延迟");
    expect(bar?.textContent).toContain("移动到分组");
    expect(bar?.textContent).toContain("导出分享链接");
    expect(bar?.textContent).toContain("删除");
  });

  it("offers import, add-node, and clipboard actions when there are no nodes", async () => {
    loadNodesMock.mockResolvedValue([]);
    await render();
    const empty = container.querySelector("[aria-label='还没有代理节点']");
    expect(empty).not.toBeNull();
    const labels = [...(empty?.querySelectorAll("button") ?? [])].map(
      (button) => button.textContent,
    );
    expect(labels).toEqual(
      expect.arrayContaining(["导入订阅", "添加节点", "从剪贴板导入分享链接"]),
    );
  });

  it("renders subscription cards with update, edit, delete, and retry", async () => {
    loadSubscriptionsMock.mockResolvedValue([
      {
        autoUpdate: true,
        enabled: true,
        excludeKeywords: "",
        id: "00000000-0000-4000-8000-000000000010",
        includeKeywords: "",
        lastError: "subscription request timed out",
        lastUpdatedAt: 1_723_456_789,
        name: "Airport",
        nodeCount: 3,
        url: "https://example.com/list?token=url-secret",
        subconverterUrl: "https://sub.example.com/link",
        updateIntervalMinutes: 60,
        userAgent: null,
      },
    ]);
    await render();
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='nodes']")
        ?.click();
      [...container.querySelectorAll<HTMLButtonElement>(".page-tabs button")]
        .find((button) => button.textContent?.includes("订阅"))
        ?.click();
    });
    const card = container.querySelector("[aria-label='订阅 Airport']");
    expect(card).not.toBeNull();
    expect(card?.querySelector("[aria-label='订阅地址']")?.textContent).toContain(
      "https://example.com/list?token=url-secret",
    );
    expect(card?.textContent).toContain("自动更新");
    expect(container.querySelector("[aria-label='刷新 Airport']")).not.toBeNull();
    expect(
      container.querySelector("[aria-label='更新当前订阅 (通过代理) Airport']"),
    ).not.toBeNull();
    expect(container.querySelector("[aria-label='编辑 Airport']")).not.toBeNull();
    expect(container.querySelector("[aria-label='删除订阅 Airport']")).not.toBeNull();
    expect(container.querySelector("[aria-label='启用 Airport']")).not.toBeNull();
    expect(card?.textContent).toContain("subscription request timed out");
    expect(
      [...(card?.querySelectorAll("button") ?? [])].some(
        (button) => button.textContent === "重试",
      ),
    ).toBe(true);
  });

  it("prints a 2026-era last-update clock from millisecond timestamps", async () => {
    const updatedAt = 1_786_291_200_000;
    loadSubscriptionsMock.mockResolvedValue([
      {
        autoUpdate: true,
        enabled: true,
        excludeKeywords: "",
        id: "00000000-0000-4000-8000-000000000010",
        includeKeywords: "",
        lastError: null,
        lastUpdatedAt: updatedAt,
        name: "Airport",
        nodeCount: 3,
        url: "https://airport.example/list",
        subconverterUrl: null,
        updateIntervalMinutes: 60,
        userAgent: null,
      },
    ]);
    await render();
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='nodes']")
        ?.click();
      [...container.querySelectorAll<HTMLButtonElement>(".page-tabs button")]
        .find((button) => button.textContent?.includes("订阅"))
        ?.click();
    });
    const card = container.querySelector("[aria-label='订阅 Airport']");
    const clock = formatClock(updatedAt);
    expect(card?.textContent).toContain(clock);
    expect(clock.startsWith("2026-")).toBe(true);
    expect(card?.textContent).not.toMatch(/58\d{3}-/u);
  });

  it("translates untested latency labels in English", async () => {
    loadAppSettingsMock.mockResolvedValue({ ...SETTINGS, locale: "en" });
    loadNodesMock.mockResolvedValue([
      { ...NODE, lastTestedAt: 1_786_291_200_000, latencyMs: null },
    ]);
    await render();
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='nodes']")
        ?.click();
    });
    const row = container.querySelector(".node-list tbody tr");
    expect(row?.textContent).toContain("Failed");
    expect(row?.textContent).not.toContain("失败");
  });

  it("moves every checked node into the saved group", async () => {
    const osaka = {
      ...NODE,
      id: "00000000-0000-4000-8000-000000000002",
      name: "Osaka",
    };
    const work = { id: "g-work", name: "Work", strategy: "select" as const };
    loadNodesMock.mockResolvedValue([NODE, osaka]);
    loadNodeGroupsMock.mockResolvedValue([work]);
    setNodeGroupMock.mockImplementation(async (id: string, groupName: string | null) => {
      const current = (await loadNodesMock()) as typeof NODE[];
      return current.map((node) =>
        node.id === id
          ? { ...node, groupId: groupName === "Work" ? work.id : null }
          : node,
      );
    });
    await render();
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='nodes']")
        ?.click();
    });
    await act(async () => {
      container
        .querySelector<HTMLInputElement>("input[aria-label='选择 Tokyo Edge']")
        ?.click();
      container
        .querySelector<HTMLInputElement>("input[aria-label='选择 Osaka']")
        ?.click();
    });
    await act(async () => {
      [...container.querySelectorAll("button")]
        .find((button) => button.textContent === "移动到分组")
        ?.click();
    });
    const groupName = container.querySelector<HTMLInputElement>(
      "[aria-label='节点分组']",
    );
    await act(async () => {
      const setter = Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        "value",
      )?.set;
      setter?.call(groupName, "Work");
      groupName?.dispatchEvent(new Event("input", { bubbles: true }));
    });
    await act(async () => {
      [...container.querySelectorAll("button")]
        .find((button) => button.textContent === "保存分组")
        ?.click();
    });
    expect(setNodeGroupMock).toHaveBeenCalledWith(NODE.id, "Work");
    expect(setNodeGroupMock).toHaveBeenCalledWith(osaka.id, "Work");
  });

  it("picks add-node protocol from twelve chips and keeps paste/scan", async () => {
    await render();
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nodes-toolbar .btn-primary")
        ?.click();
    });
    const dialog = container.querySelector("[aria-label='手动创建节点']");
    expect(dialog?.hasAttribute("hidden")).toBe(false);
    const chips = [
      ...container.querySelectorAll<HTMLButtonElement>(".protocol-chips button"),
    ].map((button) => button.textContent);
    expect(chips).toEqual([
      "VLESS",
      "VMess",
      "Shadowsocks",
      "Trojan",
      "Hysteria2",
      "TUIC",
      "WireGuard",
      "AnyTLS",
      "Naive",
      "SOCKS",
      "HTTP",
      "自定义",
    ]);
    await act(async () => {
      chips;
      [...container.querySelectorAll<HTMLButtonElement>(".protocol-chips button")]
        .find((button) => button.textContent === "Hysteria2")
        ?.click();
    });
    expect(
      container.querySelector("[aria-label='Hysteria2 认证密码']"),
    ).not.toBeNull();
    expect(container.querySelector("[aria-label='节点 UUID']")).toBeNull();
    expect(
      [...container.querySelectorAll("button")].some((button) =>
        button.textContent?.includes("从分享链接粘贴"),
      ),
    ).toBe(true);
    expect(
      [...container.querySelectorAll("button")].some((button) =>
        button.textContent?.includes("扫描二维码"),
      ),
    ).toBe(true);
  });

  /*
   * The handoff closes the topmost overlay on Escape. Dialogs were left out, so
   * the node form could only be dismissed by finding its close button.
   */
  it("closes the node dialog on Escape without closing the window", async () => {
    await render();
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='nodes']")
        ?.click();
    });
    await act(async () => {
      [...container.querySelectorAll<HTMLButtonElement>("button")]
        .find((button) => button.textContent?.includes("添加节点"))
        ?.click();
    });
    const dialog = container.querySelector("[aria-label='手动创建节点']");
    expect(dialog).not.toBeNull();
    expect(dialog?.closest(".dialog-backdrop")?.hasAttribute("hidden")).toBe(
      false,
    );

    await act(async () => {
      document.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
      );
    });
    expect(
      container
        .querySelector("[aria-label='手动创建节点']")
        ?.closest(".dialog-backdrop")
        ?.hasAttribute("hidden"),
    ).toBe(true);
  });

  it("uses page chrome for routing, DNS, and settings fields", async () => {
    loadSessionStatusMock.mockResolvedValue({
      connected: false,
      core: "sing-box",
      dns: {
        bootstrap: "223.5.5.5",
        dohPath: "/dns-query",
        fakeIpEnabled: false,
        hosts: "example.com 1.2.3.4",
        ipv6Enabled: false,
        mode: "system",
        port: 53,
        server: "1.1.1.1",
        strategy: "preferIpv4",
        systemDomains: ["lan"],
        template: "simple",
      },
      httpPort: 10809,
      mode: "rule",
      route: {
        finalOutbound: "proxy",
        providers: [
          {
            enabled: true,
            format: "binary",
            name: "ads",
            outbound: "block",
            url: "https://example.com/ads.srs",
          },
        ],
        rules: [
          {
            enabled: true,
            kind: "domainSuffix",
            outbound: "direct",
            value: "cn",
          },
        ],
      },
      routeSchemeId: "default",
      routeSchemes: [{ id: "default", name: "默认" }],
      node: NODE,
      socksPort: 10808,
      clashApiPort: 9090,
      systemProxy: true,
      systemProxyMode: "managed",
    });
    loadRuleSetsMock.mockResolvedValue([
      { bytes: 12, cached: true, name: "ads", updatedAt: 1_700_000_000 },
    ]);
    await render();
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='routing']")
        ?.click();
    });
    expect(container.querySelector(".routing-page .mode-card")).not.toBeNull();
    expect(container.querySelector("[aria-label='路由规则列表']")).not.toBeNull();
    expect(container.querySelector("[aria-label='规则集列表']")).not.toBeNull();
    expect(container.querySelector("[aria-label='默认出口']")).not.toBeNull();
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='dns']")
        ?.click();
    });
    expect(container.querySelector("[aria-label='DNS 模式']")).not.toBeNull();
    expect(container.querySelector("[aria-label='DNS 服务器']")).not.toBeNull();
    expect(container.querySelector("[aria-label='DNS 地址策略']")).not.toBeNull();
    expect(container.querySelector("[aria-label='启用 IPv6 DNS']")).not.toBeNull();
    expect(container.querySelector("[aria-label='启用 FakeIP']")).not.toBeNull();
    expect(container.querySelector("[aria-label='Bootstrap DNS']")).not.toBeNull();
    expect(
      container.querySelector("[aria-label='系统 DNS 域名后缀']"),
    ).not.toBeNull();
    expect(container.querySelector("[aria-label='DoH 路径']")).not.toBeNull();
    expect(container.querySelector("[aria-label='DNS Hosts']")).not.toBeNull();
    expect(container.querySelector("[aria-label='DNS 端口']")).not.toBeNull();
    expect(
      [...container.querySelectorAll("button")].some(
        (button) => button.textContent === "保存并重启内核",
      ),
    ).toBe(true);
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='settings']")
        ?.click();
    });
    expect(container.querySelector("[aria-label='启动时自动连接']")).not.toBeNull();
    expect(container.querySelector("[aria-label='启用 TUN']")).not.toBeNull();
    expect(container.querySelector("[aria-label='Core 选择']")).not.toBeNull();
    expect(container.querySelector("[aria-label='SOCKS 端口']")).not.toBeNull();
    expect(container.querySelector("[aria-label='热键：连接/断开']")).not.toBeNull();
    expect(container.querySelector(".settings-row")).not.toBeNull();
  });

  it("shows TUN alert, subscription skeleton, and a core-start failure banner", async () => {
    loadPlatformSummaryMock.mockResolvedValue({
      artifactIdentifier: "macos-x86_64",
      tunAvailability: "unavailableInUnsignedBuild",
    });
    connectSessionMock.mockRejectedValue({
      code: "core_not_configured",
      message: "the pinned sing-box binary is not configured",
    });
    let finishRefresh: ((value: unknown) => void) | undefined;
    refreshAllSubscriptionsMock.mockImplementation(
      () =>
        new Promise((resolve) => {
          finishRefresh = resolve;
        }),
    );
    await render();
    expect(container.querySelector("[aria-label='未签名提示']")).not.toBeNull();
    expect(container.textContent).toContain(
      "TUN 在未签名版本中不可用",
    );
    await act(async () => {
      [...container.querySelectorAll("button")]
        .find((button) => button.textContent === "连接")
        ?.click();
    });
    const banner = container.querySelector("[aria-label='内核启动失败']");
    expect(banner).not.toBeNull();
    expect(banner?.textContent).toContain("the pinned sing-box binary is not configured");
    expect(
      [...(banner?.querySelectorAll("button") ?? [])].map(
        (button) => button.textContent,
      ),
    ).toEqual(expect.arrayContaining(["重试", "查看日志", "修改端口"]));
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='nodes']")
        ?.click();
      [...container.querySelectorAll("button")]
        .find((button) => button.textContent === "更新全部订阅 (不通过代理)")
        ?.click();
    });
    expect(container.querySelector("[aria-label='正在更新订阅']")).not.toBeNull();
    expect(container.querySelector(".node-skeleton")).not.toBeNull();
    await act(async () => {
      finishRefresh?.([]);
    });
  });

  it("renders proxy members as cards and inspector transport/tls branches", async () => {
    const work = { id: "g1", name: "Work", strategy: "select" as const };
    loadNodeGroupsMock.mockResolvedValue([work]);
    loadNodesMock.mockResolvedValue([{ ...NODE, groupId: work.id }]);
    loadNodeDraftMock.mockResolvedValue({
      credential: {
        flow: "xtls-rprx-vision",
        protocol: "vless",
        userId: "11111111-1111-4111-8111-111111111111",
      },
      name: NODE.name,
      port: NODE.port,
      server: NODE.server,
      tls: {
        alpn: ["h2"],
        fingerprint: "chrome",
        publicKey: "pk",
        serverName: "www.example.com",
        shortId: "abcd",
        spiderX: "/",
        type: "reality",
      },
      transport: {
        host: "cdn.example.com",
        mode: "packet-up",
        path: "/api",
        type: "xhttp",
      },
      udpEnabled: true,
    });
    await render();
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='proxies']")
        ?.click();
    });
    const card = container.querySelector(".member-card");
    expect(card).not.toBeNull();
    expect(card?.textContent).toContain("Tokyo Edge");
    expect(card?.textContent).toContain("edge.example.com:8388");
    expect(container.querySelector("[aria-label='代理组节点'] tbody")).toBeNull();

    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='nodes']")
        ?.click();
    });
    await act(async () => {
      container
        .querySelector<HTMLTableRowElement>("[aria-label='节点列表'] tbody tr")
        ?.click();
    });
    const inspector = container.querySelector("[aria-label='节点详情']");
    await act(async () => {
      [...(inspector?.querySelectorAll("button") ?? [])]
        .find((button) => button.textContent === "传输协议")
        ?.click();
    });
    expect(inspector?.textContent).toContain("packet-up");
    expect(inspector?.textContent).toContain("/api");
    await act(async () => {
      [...(inspector?.querySelectorAll("button") ?? [])]
        .find((button) => button.textContent === "TLS")
        ?.click();
    });
    expect(inspector?.textContent).toContain("abcd");
    expect(inspector?.textContent).toContain("spiderX");
    expect(inspector?.textContent).toContain("chrome");
  });

  it("changes routing and system proxy from status-bar popovers", async () => {
    await render();
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>("[aria-label='状态栏路由模式']")
        ?.click();
    });
    expect(
      container.querySelector("[data-value='global']"),
    ).not.toBeNull();
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>("[aria-label='状态栏系统代理']")
        ?.click();
    });
    expect(
      [...container.querySelectorAll<HTMLButtonElement>("[data-value]")].map(
        (button) => button.dataset.value,
      ),
    ).toEqual(expect.arrayContaining(["managed", "cleared", "unchanged", "pac"]));
    expect(
      [...container.querySelectorAll("button")].some(
        (button) => button.textContent === "左右分栏",
      ),
    ).toBe(false);
  });

  it("shows a millisecond last-tested clock on the inspector diagnostics tab", async () => {
    const testedAt = 1_786_291_200_000;
    loadNodesMock.mockResolvedValue([{ ...NODE, lastTestedAt: testedAt }]);
    loadSessionStatusMock.mockResolvedValue({
      connected: false,
      core: "sing-box",
      dns: {
        bootstrap: "",
        dohPath: "/dns-query",
        fakeIpEnabled: false,
        hosts: "",
        ipv6Enabled: false,
        mode: "system",
        port: 53,
        server: "1.1.1.1",
        strategy: "preferIpv4",
        systemDomains: [],
        template: "simple",
      },
      httpPort: 10809,
      mode: "global",
      route: { finalOutbound: "proxy", providers: [], rules: [] },
      routeSchemeId: "default",
      routeSchemes: [{ id: "default", name: "默认" }],
      node: { ...NODE, lastTestedAt: testedAt },
      socksPort: 10808,
      clashApiPort: 9090,
      systemProxy: true,
      systemProxyMode: "managed",
    });
    await render();
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='nodes']")
        ?.click();
    });
    await act(async () => {
      container
        .querySelector<HTMLTableRowElement>("[aria-label='节点列表'] tbody tr")
        ?.click();
    });
    const inspector = container.querySelector("[aria-label='节点详情']");
    await act(async () => {
      [...(inspector?.querySelectorAll("button") ?? [])]
        .find((button) => button.textContent === "诊断")
        ?.click();
    });
    const clock = formatClock(testedAt);
    expect(inspector?.textContent).toContain(clock);
    expect(clock.startsWith("2026-")).toBe(true);
    expect(inspector?.textContent).not.toMatch(/58\d{3}-/u);
  });

  it("double-clicks the already-selected node to connect while disconnected", async () => {
    await render();
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(".nav-item[data-page='nodes']")
        ?.click();
    });
    const row = container.querySelector("[aria-label='节点列表'] tbody tr");
    expect(row?.className).toContain("active-node");
    connectSessionMock.mockClear();
    await act(async () => {
      row?.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    });
    expect(connectSessionMock).toHaveBeenCalled();
  });
});
