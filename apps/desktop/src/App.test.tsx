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
const importNodeMock = vi.hoisted(() => vi.fn());
const exportNodeLinkMock = vi.hoisted(() => vi.fn());
const cloneNodeMock = vi.hoisted(() => vi.fn());
const nodeQrCodeMock = vi.hoisted(() => vi.fn());
const readQrCodeMock = vi.hoisted(() => vi.fn());
const checkUpdateMock = vi.hoisted(() => vi.fn());
const checkCoreUpdateMock = vi.hoisted(() => vi.fn());
const loadCoreReadinessMock = vi.hoisted(() => vi.fn());
const removeDuplicateNodesMock = vi.hoisted(() => vi.fn());
const createNodeMock = vi.hoisted(() => vi.fn());
const importNodesMock = vi.hoisted(() => vi.fn());
const loadLogsMock = vi.hoisted(() => vi.fn());
const loadAppSettingsMock = vi.hoisted(() => vi.fn());
const previewCoreConfigMock = vi.hoisted(() => vi.fn());
const saveAppSettingsMock = vi.hoisted(() => vi.fn());
const clearLogsMock = vi.hoisted(() => vi.fn());
const clearTrafficMock = vi.hoisted(() => vi.fn());
const selectNodeMock = vi.hoisted(() => vi.fn());
const switchNodeMock = vi.hoisted(() => vi.fn());
const deleteNodeMock = vi.hoisted(() => vi.fn());
const editNodeMock = vi.hoisted(() => vi.fn());
const loadNodeDraftMock = vi.hoisted(() => vi.fn());
const updateNodeMock = vi.hoisted(() => vi.fn());
const moveNodeMock = vi.hoisted(() => vi.fn());
const reorderNodesMock = vi.hoisted(() => vi.fn());
const setNodeGroupMock = vi.hoisted(() => vi.fn());
const setNodeFrontMock = vi.hoisted(() => vi.fn());
const setNodeGroupStrategyMock = vi.hoisted(() => vi.fn());
const testNodeMock = vi.hoisted(() => vi.fn());
const testAllNodesMock = vi.hoisted(() => vi.fn());
const testDownloadSpeedMock = vi.hoisted(() => vi.fn());
const testUrlMock = vi.hoisted(() => vi.fn());
const connectSessionMock = vi.hoisted(() => vi.fn());
const disconnectSessionMock = vi.hoisted(() => vi.fn());
const setRoutingModeMock = vi.hoisted(() => vi.fn());
const setDnsSettingsMock = vi.hoisted(() => vi.fn());
const setRouteSettingsMock = vi.hoisted(() => vi.fn());
const exportDiagnosticsMock = vi.hoisted(() => vi.fn());
const exportPreferencesMock = vi.hoisted(() => vi.fn());
const importPreferencesMock = vi.hoisted(() => vi.fn());
const loadSystemProxyStartupStatusMock = vi.hoisted(() => vi.fn());
const recoverSystemProxyMock = vi.hoisted(() => vi.fn());
const dismissSystemProxyRecoveryMock = vi.hoisted(() => vi.fn());
const loadSubscriptionsMock = vi.hoisted(() => vi.fn());
const createSubscriptionMock = vi.hoisted(() => vi.fn());
const updateSubscriptionMock = vi.hoisted(() => vi.fn());
const refreshSubscriptionMock = vi.hoisted(() => vi.fn());
const refreshAllSubscriptionsMock = vi.hoisted(() => vi.fn());
const setNodeEnabledMock = vi.hoisted(() => vi.fn());
const deleteSubscriptionMock = vi.hoisted(() => vi.fn());
const syncGlobalHotkeysMock = vi.hoisted(() => vi.fn());
const loadConnectionsMock = vi.hoisted(() => vi.fn());
const loadRuleSetsMock = vi.hoisted(() => vi.fn());
const updateRuleSetMock = vi.hoisted(() => vi.fn());
const updateRuleSetsMock = vi.hoisted(() => vi.fn());
const downloadCoreUpdateMock = vi.hoisted(() => vi.fn());
const setRouteSchemeMock = vi.hoisted(() => vi.fn());
const createRouteSchemeMock = vi.hoisted(() => vi.fn());
const deleteRouteSchemeMock = vi.hoisted(() => vi.fn());
const loadGeoAssetsStatusMock = vi.hoisted(() => vi.fn());
const updateGeoAssetsMock = vi.hoisted(() => vi.fn());
const closeConnectionMock = vi.hoisted(() => vi.fn());
const closeConnectionsMock = vi.hoisted(() => vi.fn());
const clearGlobalHotkeysMock = vi.hoisted(() => vi.fn());

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
    clearLogs: clearLogsMock,
    loadConnections: loadConnectionsMock,
    loadRuleSets: loadRuleSetsMock,
    updateRuleSet: updateRuleSetMock,
    updateRuleSets: updateRuleSetsMock,
    downloadCoreUpdate: downloadCoreUpdateMock,
    setRouteScheme: setRouteSchemeMock,
    createRouteScheme: createRouteSchemeMock,
    deleteRouteScheme: deleteRouteSchemeMock,
    loadGeoAssetsStatus: loadGeoAssetsStatusMock,
    updateGeoAssets: updateGeoAssetsMock,
    closeConnection: closeConnectionMock,
    closeConnections: closeConnectionsMock,
    clearTraffic: clearTrafficMock,
    connectSession: connectSessionMock,
    createNode: createNodeMock,
    loadAppSettings: loadAppSettingsMock,
    previewCoreConfig: previewCoreConfigMock,
    loadLogs: loadLogsMock,
    saveAppSettings: saveAppSettingsMock,
    disconnectSession: disconnectSessionMock,
    dismissSystemProxyRecovery: dismissSystemProxyRecoveryMock,
    exportDiagnostics: exportDiagnosticsMock,
    exportPreferences: exportPreferencesMock,
    importPreferences: importPreferencesMock,
    editNode: editNodeMock,
    loadNodeDraft: loadNodeDraftMock,
    updateNode: updateNodeMock,
    importNode: importNodeMock,
    importNodes: importNodesMock,
    isCommandError: actual.isCommandError,
    deleteNode: deleteNodeMock,
  exportNodeLink: exportNodeLinkMock,
  cloneNode: cloneNodeMock,
  nodeQrCode: nodeQrCodeMock,
  readQrCode: readQrCodeMock,
  checkUpdate: checkUpdateMock,
  checkCoreUpdate: checkCoreUpdateMock,
  loadCoreReadiness: loadCoreReadinessMock,
  removeDuplicateNodes: removeDuplicateNodesMock,
    loadNodeGroups: loadNodeGroupsMock,
    loadNodes: loadNodesMock,
    loadSessionStatus: loadSessionStatusMock,
    loadTraffic: loadTrafficMock,
  loadNodeTraffic: loadNodeTrafficMock,
    moveNode: moveNodeMock,
    reorderNodes: reorderNodesMock,
    loadSystemProxyStartupStatus: loadSystemProxyStartupStatusMock,
    recoverSystemProxy: recoverSystemProxyMock,
    selectNode: selectNodeMock,
    switchNode: switchNodeMock,
    setNodeEnabled: setNodeEnabledMock,
    setRoutingMode: setRoutingModeMock,
    setNodeGroup: setNodeGroupMock,
    setNodeFront: setNodeFrontMock,
    setNodeGroupStrategy: setNodeGroupStrategyMock,
    setDnsSettings: setDnsSettingsMock,
    setRouteSettings: setRouteSettingsMock,
    testAllNodes: testAllNodesMock,
    testDownloadSpeed: testDownloadSpeedMock,
    testNode: testNodeMock,
    testUrl: testUrlMock,
  };
});

vi.mock("./subscriptions", () => ({
  createSubscription: createSubscriptionMock,
  deleteSubscription: deleteSubscriptionMock,
  loadSubscriptions: loadSubscriptionsMock,
  refreshSubscription: refreshSubscriptionMock,
  refreshAllSubscriptions: refreshAllSubscriptionsMock,
  updateSubscription: updateSubscriptionMock,
}));

import App from "./App";
import type { SessionStatus } from "./session";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

const IDLE: SessionStatus = {
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
  route: { finalOutbound: "proxy", providers: [], rules: [],
  },
  routeSchemeId: "default",
  routeSchemes: [{ id: "default", name: "默认" }],
  node: null,
  socksPort: 10808,
  clashApiPort: 9090,
  systemProxy: true,
  systemProxyMode: "managed" as const,
};

const SELECTED: SessionStatus = {
  ...IDLE,
  node: {
    deletable: true,
    enabled: true,
    frontNodeId: null,
    groupId: null,
    id: "00000000-0000-0000-0000-000000000001",
    lastTestedAt: null,
    latencyMs: null,
    name: "Tokyo Edge",
    port: 8388,
    protocol: "shadowsocks",
    server: "edge.example.com",
    transport: "tcp",
    tls: null,
  },
};

const CONNECTED: SessionStatus = { ...SELECTED, connected: true };

const DEFAULT_SETTINGS = {
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

const SUBSCRIPTION = {
  autoUpdate: true,
      enabled: true,
    excludeKeywords: "",
  id: "00000000-0000-0000-0000-000000000010",
  includeKeywords: "",
  lastUpdatedAt: null,
  lastError: null,
  name: "Airport",
  nodeCount: 3,
  updateIntervalMinutes: 60,
  userAgent: null,
  subconverterUrl: null,
};

describe("App", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    loadPlatformSummaryMock.mockReset();
    loadSessionStatusMock.mockReset();
    loadTrafficMock.mockReset();
    loadNodeTrafficMock.mockReset();
    loadNodeTrafficMock.mockResolvedValue({});
    syncGlobalHotkeysMock.mockReset();
    clearGlobalHotkeysMock.mockReset();
    // Tests run outside Tauri; keep the in-window keydown path.
    syncGlobalHotkeysMock.mockResolvedValue(false);
    clearGlobalHotkeysMock.mockResolvedValue(undefined);
    loadNodesMock.mockReset();
    loadNodeGroupsMock.mockReset();
    importNodeMock.mockReset();
    createNodeMock.mockReset();
    importNodesMock.mockReset();
    selectNodeMock.mockReset();
    switchNodeMock.mockReset();
    setNodeEnabledMock.mockReset();
    deleteNodeMock.mockReset();
    editNodeMock.mockReset();
    loadNodeDraftMock.mockReset();
    updateNodeMock.mockReset();
    moveNodeMock.mockReset();
    reorderNodesMock.mockReset();
    setNodeGroupMock.mockReset();
    setNodeFrontMock.mockReset();
    setNodeGroupStrategyMock.mockReset();
    testNodeMock.mockReset();
    testAllNodesMock.mockReset();
    testDownloadSpeedMock.mockReset();
    testUrlMock.mockReset();
    connectSessionMock.mockReset();
    disconnectSessionMock.mockReset();
    setRoutingModeMock.mockReset();
    setDnsSettingsMock.mockReset();
    setRouteSettingsMock.mockReset();
    exportDiagnosticsMock.mockReset();
    exportPreferencesMock.mockReset();
    importPreferencesMock.mockReset();
    loadSystemProxyStartupStatusMock.mockReset();
    recoverSystemProxyMock.mockReset();
    dismissSystemProxyRecoveryMock.mockReset();
    loadLogsMock.mockReset();
    loadAppSettingsMock.mockReset();
    saveAppSettingsMock.mockReset();
    clearLogsMock.mockReset();
    clearTrafficMock.mockReset();
    loadSubscriptionsMock.mockReset();
    createSubscriptionMock.mockReset();
    updateSubscriptionMock.mockReset();
    refreshSubscriptionMock.mockReset();
    refreshAllSubscriptionsMock.mockReset();
    deleteSubscriptionMock.mockReset();
    loadConnectionsMock.mockReset();
    loadRuleSetsMock.mockReset();
    updateRuleSetMock.mockReset();
    updateRuleSetsMock.mockReset();
    loadRuleSetsMock.mockResolvedValue([]);
    updateRuleSetMock.mockResolvedValue([]);
    updateRuleSetsMock.mockResolvedValue([]);
    downloadCoreUpdateMock.mockReset();
    setRouteSchemeMock.mockReset();
    createRouteSchemeMock.mockReset();
    deleteRouteSchemeMock.mockReset();
    loadGeoAssetsStatusMock.mockReset();
    updateGeoAssetsMock.mockReset();
    closeConnectionMock.mockReset();
    closeConnectionsMock.mockReset();

    loadPlatformSummaryMock.mockResolvedValue({
      artifactIdentifier: "macos-x86_64",
      tunAvailability: "unavailableInUnsignedBuild",
    });
    loadSessionStatusMock.mockResolvedValue(IDLE);
    loadTrafficMock.mockResolvedValue({
      downloadBytesPerSecond: 0,
      monthBytes: 0,
      todayBytes: 0,
      totalBytes: 0,
      uploadBytesPerSecond: 0,
    });
    loadNodesMock.mockResolvedValue([]);
    loadNodeGroupsMock.mockResolvedValue([]);
    setNodeGroupStrategyMock.mockResolvedValue([]);
    setNodeGroupStrategyMock.mockResolvedValue([]);
    loadLogsMock.mockResolvedValue([]);
    loadAppSettingsMock.mockResolvedValue(DEFAULT_SETTINGS);
    saveAppSettingsMock.mockImplementation((value) => Promise.resolve(value));
    clearLogsMock.mockResolvedValue(undefined);
    clearTrafficMock.mockResolvedValue({
      downloadBytesPerSecond: 0,
      monthBytes: 0,
      todayBytes: 0,
      totalBytes: 0,
      uploadBytesPerSecond: 0,
    });
    loadSystemProxyStartupStatusMock.mockResolvedValue("clean");
    recoverSystemProxyMock.mockResolvedValue("clean");
    dismissSystemProxyRecoveryMock.mockResolvedValue("clean");
    loadSubscriptionsMock.mockResolvedValue([]);
    loadCoreReadinessMock.mockReset();
    loadCoreReadinessMock.mockResolvedValue({ singBox: true, xray: false });
    testAllNodesMock.mockResolvedValue(undefined);
    loadConnectionsMock.mockResolvedValue({
      uploadTotalBytes: 0,
      downloadTotalBytes: 0,
      connections: [],
    });
    closeConnectionMock.mockResolvedValue(undefined);
    closeConnectionsMock.mockResolvedValue(undefined);

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

  /// Whether a menu item exists but cannot be used.
  async function nodeMenuItemDisabled(
    node: string,
    item: string,
  ): Promise<boolean> {
    const opener = container.querySelector<HTMLButtonElement>(
      `[aria-label='操作 ${node}']`,
    );
    await act(async () => opener?.click());
    const target = Array.from(
      container
        .querySelector(`[aria-label='节点操作 ${node}']`)!
        .querySelectorAll<HTMLButtonElement>("button"),
    ).find((button) => button.textContent === item);
    return target?.disabled ?? false;
  }

  /// Ticks a node's selection checkbox.
  function checkNode(name: string): void {
    const box = container.querySelector<HTMLInputElement>(
      `[aria-label='选择 ${name}']`,
    );
    if (!box) {
      throw new Error(`no selection checkbox for ${name}`);
    }
    box.click();
  }

  /// The names of every checked node, in table order.
  function checkedNodeNames(): string[] {
    return [
      ...container.querySelectorAll<HTMLInputElement>(
        "[aria-label^='选择 ']",
      ),
    ]
      .filter((box) => box.checked)
      .map((box) => box.getAttribute("aria-label")!.replace("选择 ", ""));
  }

  /// Opens a node's context menu and clicks one of its items, the way the
  /// v2rayN-style table exposes every per-node action.
  async function nodeMenuAction(node: string, item: string): Promise<void> {
    const opener = container.querySelector<HTMLButtonElement>(
      `[aria-label='操作 ${node}']`,
    );
    if (!opener) {
      throw new Error(`no context-menu opener for ${node}`);
    }
    await act(async () => opener.click());
    const menu = container.querySelector(`[aria-label='节点操作 ${node}']`);
    if (!menu) {
      throw new Error(`the context menu for ${node} did not open`);
    }
    const target = Array.from(
      menu.querySelectorAll<HTMLButtonElement>("button"),
    ).find((button) => button.textContent === item);
    if (!target) {
      throw new Error(`no ${item} item in the menu for ${node}`);
    }
    if (target.disabled) {
      throw new Error(`${item} is disabled for ${node}`);
    }
    await act(async () => target.click());
  }

  function selectValue(value: string, field: HTMLSelectElement): void {
    const setter = Object.getOwnPropertyDescriptor(
      HTMLSelectElement.prototype,
      "value",
    )?.set;
    if (!setter) {
      throw new Error("no select value setter to drive React with");
    }
    setter.call(field, value);
    field.dispatchEvent(new Event("change", { bubbles: true }));
  }

  /** React tracks the DOM value itself, so a plain assignment is ignored. */
  function type(value: string, field: HTMLTextAreaElement): void {
    const setter = Object.getOwnPropertyDescriptor(
      HTMLTextAreaElement.prototype,
      "value",
    )?.set;
    if (!setter) {
      throw new Error("no textarea value setter to drive React with");
    }
    setter.call(field, value);
    field.dispatchEvent(new Event("input", { bubbles: true }));
  }

  function typeInput(value: string, field: HTMLInputElement): void {
    const setter = Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      "value",
    )?.set;
    if (!setter) {
      throw new Error("no input value setter to drive React with");
    }
    setter.call(field, value);
    field.dispatchEvent(new Event("input", { bubbles: true }));
  }

  function button(label: string): HTMLButtonElement {
    const found = [...container.querySelectorAll("button")].find(
      (candidate) => candidate.textContent === label,
    );
    if (!found) {
      throw new Error(`no "${label}" button in ${container.innerHTML}`);
    }
    return found;
  }

  async function toggleConnect(): Promise<void> {
    await act(async () => {
      document.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Enter",
          code: "Enter",
          ctrlKey: true,
          bubbles: true,
        }),
      );
    });
  }

  it("warns on first run that this build is unsigned", async () => {
    await render();

    const notice = container.querySelector("[role='note']");
    expect(notice?.textContent).toContain("未签名");
    expect(notice?.textContent).toContain("Gatekeeper");
  });

  it("names the capability an unsigned macOS build gives up", async () => {
    await render();

    expect(container.querySelector("[role='note']")?.textContent).toContain(
      "TUN",
    );
  });

  it("stops warning once the notice is dismissed", async () => {
    await render();

    await act(async () => button("我知道了").click());

    expect(container.querySelector("[role='note']")).toBeNull();
  });

  it("keeps the notice dismissed on the next launch", async () => {
    await render();
    await act(async () => button("我知道了").click());

    act(() => root.unmount());
    container = document.createElement("div");
    document.body.append(container);
    root = createRoot(container);
    await render();

    expect(container.querySelector("[role='note']")).toBeNull();
  });

  it("still starts, and still warns, when storage is unavailable", async () => {
    const getItem = vi
      .spyOn(Storage.prototype, "getItem")
      .mockImplementation(() => {
        throw new Error("storage disabled");
      });
    const setItem = vi
      .spyOn(Storage.prototype, "setItem")
      .mockImplementation(() => {
        throw new Error("storage disabled");
      });
    try {
      await render();
      expect(container.querySelector("[role='note']")).not.toBeNull();

      await act(async () => button("我知道了").click());

      expect(container.querySelector("[role='note']")).toBeNull();
    } finally {
      getItem.mockRestore();
      setItem.mockRestore();
    }
  });

  it("shows an idle dashboard before a node is imported", async () => {
    await render();

    expect(container.textContent).toContain("未连接");
    expect(container.textContent).toContain("sing-box");
    expect(container.textContent).toContain("macos-x86_64");
    expect(connectSessionMock).not.toHaveBeenCalled();
    expect(loadTrafficMock).toHaveBeenCalledOnce();
  });

  it("shows zeroed traffic rates in the status bar while disconnected", async () => {
    loadTrafficMock.mockResolvedValue({
      downloadBytesPerSecond: 0,
      monthBytes: 1_048_576,
      todayBytes: 2_048,
      totalBytes: 1_073_741_824,
      uploadBytesPerSecond: 0,
    });

    await render();

    expect(
      container.querySelector("[aria-label='下载速率']")?.textContent,
    ).toBe("↓ 0 B/s");
    expect(
      container.querySelector("[aria-label='上传速率']")?.textContent,
    ).toBe("↑ 0 B/s");
  });

  it("shows live upload and download rates while connected", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    loadTrafficMock.mockResolvedValue({
      downloadBytesPerSecond: 2_048,
      monthBytes: 3_072,
      todayBytes: 3_072,
      totalBytes: 4_096,
      uploadBytesPerSecond: 1_048_576,
    });

    await render();

    expect(loadTrafficMock).toHaveBeenCalledOnce();
    expect(
      container.querySelector("[aria-label='下载速率']")?.textContent,
    ).toBe("↓ 2.0 KB/s");
    expect(
      container.querySelector("[aria-label='上传速率']")?.textContent,
    ).toBe("↑ 1.0 MB/s");
  });

  it("keeps background traffic failures out of the user alert", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    loadTrafficMock.mockRejectedValue({
      code: "traffic_sample_timeout",
      message: "traffic sample timed out",
    });
    const warning = vi.spyOn(console, "warn").mockImplementation(() => {});

    try {
      await render();

      expect(container.querySelector("[role='alert']")).toBeNull();
      expect(warning).toHaveBeenCalledWith(
        "traffic refresh failed",
        expect.anything(),
      );
    } finally {
      warning.mockRestore();
    }
  });

  it("reports TUN as unavailable in an unsigned macOS build", async () => {
    await render();

    expect(container.textContent).toContain("未签名版本不可用");
  });

  it("reports TUN as needing elevation on Windows and Linux", async () => {
    loadPlatformSummaryMock.mockResolvedValue({
      artifactIdentifier: "windows-x86_64",
      tunAvailability: "requiresElevation",
    });

    await render();

    expect(container.textContent).toContain("需要管理员权限");
  });

  it("offers to restore System Proxy settings left by an abnormal exit", async () => {
    loadSystemProxyStartupStatusMock.mockResolvedValue("restoreRequired");

    await render();

    const recovery = container.querySelector(
      "[aria-label='系统代理恢复']",
    );
    expect(recovery?.textContent).toContain("异常退出");
    expect(recovery?.textContent).toContain("恢复原设置");
    expect(recovery?.textContent).toContain("保留当前设置");
  });

  it("does not connect before startup System Proxy inspection finishes", async () => {
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadSystemProxyStartupStatusMock.mockReturnValue(new Promise(() => {}));

    await render();

    await toggleConnect();
    expect(connectSessionMock).not.toHaveBeenCalled();
  });

  it("restores the saved System Proxy settings", async () => {
    loadSystemProxyStartupStatusMock.mockResolvedValue("restoreRequired");
    await render();

    await act(async () => button("恢复原设置").click());

    expect(recoverSystemProxyMock).toHaveBeenCalledOnce();
    expect(
      container.querySelector("[aria-label='系统代理恢复']"),
    ).toBeNull();
  });

  it("can keep the current System Proxy settings", async () => {
    loadSystemProxyStartupStatusMock.mockResolvedValue("restoreRequired");
    await render();

    await act(async () => button("保留当前设置").click());

    expect(dismissSystemProxyRecoveryMock).toHaveBeenCalledOnce();
    expect(
      container.querySelector("[aria-label='系统代理恢复']"),
    ).toBeNull();
  });

  it("keeps the recovery choice visible when restoration fails", async () => {
    loadSystemProxyStartupStatusMock.mockResolvedValue("restoreRequired");
    recoverSystemProxyMock.mockRejectedValue({
      code: "system_proxy_failed",
      message: "failed to restore System Proxy",
    });
    await render();

    await act(async () => button("恢复原设置").click());

    expect(
      container.querySelector("[aria-label='系统代理恢复']"),
    ).not.toBeNull();
    expect(container.querySelector("[role='alert']")?.textContent).toContain(
      "failed to restore System Proxy",
    );
  });

  it("imports a share link and enables connecting", async () => {
    importNodeMock.mockResolvedValue(SELECTED);
    loadNodesMock
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([SELECTED.node]);
    await render();

    const field = container.querySelector<HTMLTextAreaElement>(
      "textarea[aria-label='分享链接']",
    );
    if (!field) {
      throw new Error("no sharing URI field");
    }
    await act(async () => {
      type("  ss://aes-128-gcm:secret@edge.example.com:8388  ", field);
    });
    await act(async () => button("导入").click());

    expect(importNodeMock).toHaveBeenCalledWith(
      "ss://aes-128-gcm:secret@edge.example.com:8388",
    );
    expect(container.textContent).toContain("Tokyo Edge");
    expect(container.textContent).toContain("shadowsocks");
    expect(container.textContent).toContain("edge.example.com");
    expect(
      container.querySelector("[aria-label='节点列表']")?.textContent,
    ).toContain("Tokyo Edge");
    expect(
      container.querySelector("[aria-label='节点列表']")?.textContent,
    ).toContain("Tokyo Edge");
  });

  function createField(label: string): HTMLInputElement {
    const field = container.querySelector<HTMLInputElement>(
      `input[aria-label='${label}']`,
    );
    if (!field) {
      throw new Error(`no "${label}" field in ${container.innerHTML}`);
    }
    return field;
  }

  function createSelect(label: string): HTMLSelectElement {
    const field = container.querySelector<HTMLSelectElement>(
      `select[aria-label='${label}']`,
    );
    if (!field) {
      throw new Error(`no "${label}" select in ${container.innerHTML}`);
    }
    return field;
  }

  function bulkField(): HTMLTextAreaElement {
    const field = container.querySelector<HTMLTextAreaElement>(
      "textarea[aria-label='批量节点列表']",
    );
    if (!field) {
      throw new Error("no bulk import field");
    }
    return field;
  }

  const TWO_LINKS =
    "ss://aes-128-gcm:secret@edge.example.com:8388#Tokyo\nss://aes-128-gcm:secret@osaka.example.com:9000#Osaka";

  it("bulk imports pasted links and reports the count", async () => {
    importNodesMock.mockResolvedValue({
      duplicates: 0,
      failures: [],
      imported: 2,
      status: SELECTED,
    });
    loadNodesMock
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([SELECTED.node]);
    await render();

    await act(async () => {
      type(TWO_LINKS, bulkField());
    });
    await act(async () => button("批量导入").click());

    expect(importNodesMock).toHaveBeenCalledWith(TWO_LINKS);
    expect(
      container.querySelector("[aria-label='批量导入结果']")?.textContent,
    ).toContain("成功导入 2 个");
    // A successful import clears the box so the list is not imported twice.
    expect(bulkField().value).toBe("");
  });

  it("reports duplicates and per-line failures", async () => {
    importNodesMock.mockResolvedValue({
      duplicates: 1,
      failures: [
        { line: 2, message: "not a supported sharing link" },
        { line: null, message: "keyring is locked" },
      ],
      imported: 1,
      status: SELECTED,
    });
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      type(TWO_LINKS, bulkField());
    });
    await act(async () => button("批量导入").click());

    const report = container.querySelector("[aria-label='批量导入结果']");
    expect(report?.textContent).toContain("成功导入 1 个");
    expect(report?.textContent).toContain("跳过 1 个重复");
    expect(report?.textContent).toContain("2 行失败");
    expect(report?.textContent).toContain(
      "第 2 行：not a supported sharing link",
    );
    // A failure with no line renders without a line prefix.
    expect(report?.textContent).toContain("keyring is locked");
    expect(report?.textContent).not.toContain("第 null 行");
  });

  it("keeps the pasted text when nothing was imported", async () => {
    importNodesMock.mockResolvedValue({
      duplicates: 0,
      failures: [{ line: 1, message: "not a supported sharing link" }],
      imported: 0,
      status: IDLE,
    });
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      type("not a link", bulkField());
    });
    await act(async () => button("批量导入").click());

    expect(bulkField().value).toBe("not a link");
  });

  it("refuses to bulk import an empty box", async () => {
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      type("   ", bulkField());
    });
    await act(async () => button("批量导入").click());

    expect(importNodesMock).not.toHaveBeenCalled();
    expect(container.textContent).toContain("请先粘贴节点链接或选择文件");
  });

  it("imports a chosen file through the same command", async () => {
    importNodesMock.mockResolvedValue({
      duplicates: 0,
      failures: [],
      imported: 2,
      status: SELECTED,
    });
    loadNodesMock.mockResolvedValue([]);
    await render();

    const picker = container.querySelector<HTMLInputElement>(
      "input[aria-label='从文件导入节点']",
    );
    if (!picker) {
      throw new Error("no file picker");
    }
    const file = new File([TWO_LINKS], "nodes.txt", { type: "text/plain" });
    Object.defineProperty(picker, "files", { value: [file] });

    await act(async () => {
      picker.dispatchEvent(new Event("change", { bubbles: true }));
    });

    expect(importNodesMock).toHaveBeenCalledWith(TWO_LINKS);
    expect(
      container.querySelector("[aria-label='批量导入结果']")?.textContent,
    ).toContain("成功导入 2 个");
  });

  it("surfaces a backend error from a bulk import", async () => {
    importNodesMock.mockRejectedValue({
      code: "invalid_node_list",
      message: "the text is neither sharing links nor a Base64 node list",
    });
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      type("!!!garbage!!!", bulkField());
    });
    await act(async () => button("批量导入").click());

    expect(container.textContent).toContain(
      "the text is neither sharing links nor a Base64 node list",
    );
    expect(container.querySelector("[aria-label='批量导入结果']")).toBeNull();
  });

  const LOG_ENTRIES = [
    {
      level: "info" as const,
      message: "session connected",
      source: "app" as const,
      timestampMs: 1_760_000_000_000,
    },
    {
      level: "error" as const,
      message: "outbound dial failed",
      source: "core" as const,
      timestampMs: 1_760_000_001_000,
    },
  ];

  it("saves a settings toggle and keeps the new value", async () => {
    await render();

    await act(async () => createField("启动时自动连接").click());

    expect(saveAppSettingsMock).toHaveBeenCalledWith({
      ...DEFAULT_SETTINGS,
      connectOnLaunch: true,
    });
    expect(createField("启动时自动连接").checked).toBe(true);
  });

  it("offers TUN and explains that it replaces the system proxy", async () => {
    loadPlatformSummaryMock.mockResolvedValue({
      artifactIdentifier: "linux-x86_64",
      tunAvailability: "requiresElevation",
    });
    await render();

    const toggle = createField("启用 TUN");
    expect(toggle.disabled).toBe(false);

    await act(async () => toggle.click());

    expect(saveAppSettingsMock).toHaveBeenCalledWith({
      ...DEFAULT_SETTINGS,
      tunEnabled: true,
    });
    expect(container.textContent).toContain("TUN 与系统代理互斥");
  });

  it("keeps TUN unavailable on an unsigned macOS build", async () => {
    loadPlatformSummaryMock.mockResolvedValue({
      artifactIdentifier: "macos-aarch64",
      tunAvailability: "unavailableInUnsignedBuild",
    });
    await render();

    // The entitlement is missing, so the switch must not look usable.
    expect(createField("启用 TUN").disabled).toBe(true);
  });

  it("saves a Core choice and shows the matrix caveat", async () => {
    await render();

    await act(async () => {
      selectValue("xray", createSelect("Core 选择"));
    });

    expect(saveAppSettingsMock).toHaveBeenCalledWith({
      ...DEFAULT_SETTINGS,
      corePreference: "xray",
    });
    // The UI must not decide what Xray can do; it repeats what the matrix says.
    expect(container.textContent).toContain("Xray 不支持 Hysteria2");
  });

  it("changes the Core while connected", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    loadNodesMock.mockResolvedValue([CONNECTED.node]);
    await render();

    const core = createSelect("Core 选择");
    expect(core.disabled).toBe(false);
    await act(async () => selectValue("sing-box", core));

    expect(saveAppSettingsMock).toHaveBeenCalledWith(
      expect.objectContaining({ corePreference: "sing-box" }),
    );
  });

  it("switches the system proxy mode while connected", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    loadNodesMock.mockResolvedValue([CONNECTED.node]);
    await render();

    const control = container.querySelector<HTMLSelectElement>(
      "[aria-label='状态栏系统代理']",
    );
    expect(control?.disabled).toBe(false);
    await act(async () => selectValue("pac", control!));

    expect(saveAppSettingsMock).toHaveBeenCalledWith(
      expect.objectContaining({ systemProxyMode: "pac" }),
    );
  });

  it("seeds the log filter from the saved level", async () => {
    loadAppSettingsMock.mockResolvedValue({
      ...DEFAULT_SETTINGS,
      logLevel: "debug",
      });
    await render();

    expect(createSelect("日志级别").value).toBe("debug");
    expect(loadLogsMock).toHaveBeenCalledWith("debug", null);
  });

  it("changing the default level also changes what the panel shows", async () => {
    await render();
    loadLogsMock.mockClear();

    await act(async () => {
      selectValue("warn", createSelect("默认日志级别"));
    });

    expect(saveAppSettingsMock).toHaveBeenCalledWith({
      ...DEFAULT_SETTINGS,
      logLevel: "warn",
    });
    expect(loadLogsMock).toHaveBeenLastCalledWith("warn", null);
  });

  it("restores the previous value when saving settings fails", async () => {
    saveAppSettingsMock.mockRejectedValue({
      code: "app_settings_store_failed",
      message: "database is locked",
    });
    await render();

    await act(async () => createField("关闭时最小化到托盘").click());

    // The switch must not claim a state the app failed to persist.
    expect(createField("关闭时最小化到托盘").checked).toBe(true);
    expect(container.textContent).toContain("database is locked");
  });

  it("shows log entries with their source and level", async () => {
    loadLogsMock.mockResolvedValue(LOG_ENTRIES);
    await render();

    const list = container.querySelector("[aria-label='日志列表']");
    expect(list?.textContent).toContain("session connected");
    expect(list?.textContent).toContain("outbound dial failed");
    expect(list?.textContent).toContain("Core");
    expect(list?.textContent).toContain("应用");
    expect(loadLogsMock).toHaveBeenCalledWith("info", null);
  });

  it("shows a placeholder when there are no logs", async () => {
    loadLogsMock.mockResolvedValue([]);
    await render();

    expect(container.querySelector("[aria-label='日志列表']")).toBeNull();
    expect(container.textContent).toContain("暂无日志");
  });

  it("reloads logs when the level or source filter changes", async () => {
    loadLogsMock.mockResolvedValue(LOG_ENTRIES);
    await render();
    loadLogsMock.mockClear();

    await act(async () => {
      selectValue("error", createSelect("日志级别"));
    });
    expect(loadLogsMock).toHaveBeenLastCalledWith("error", null);

    await act(async () => {
      selectValue("core", createSelect("日志来源"));
    });
    expect(loadLogsMock).toHaveBeenLastCalledWith("error", "core");
  });

  it("clears the log buffer", async () => {
    loadLogsMock.mockResolvedValue(LOG_ENTRIES);
    await render();
    expect(container.querySelector("[aria-label='日志列表']")).not.toBeNull();

    await act(async () => button("清空日志").click());

    expect(clearLogsMock).toHaveBeenCalled();
    expect(container.textContent).toContain("暂无日志");
  });

  it("clears traffic statistics from the settings menu", async () => {
    loadTrafficMock.mockResolvedValue({
      downloadBytesPerSecond: 2_048,
      monthBytes: 1_048_576,
      todayBytes: 2_048,
      totalBytes: 1_073_741_824,
      uploadBytesPerSecond: 1_024,
    });
    clearTrafficMock.mockResolvedValue({
      downloadBytesPerSecond: 0,
      monthBytes: 0,
      todayBytes: 0,
      totalBytes: 0,
      uploadBytesPerSecond: 0,
    });
    await render();
    expect(
      container.querySelector("[aria-label='下载速率']")?.textContent,
    ).toBe("↓ 2.0 KB/s");

    await act(async () => button("清除流量统计").click());

    expect(clearTrafficMock).toHaveBeenCalledOnce();
    expect(
      container.querySelector("[aria-label='下载速率']")?.textContent,
    ).toBe("↓ 0 B/s");
    expect(
      container.querySelector("[aria-label='上传速率']")?.textContent,
    ).toBe("↑ 0 B/s");
  });

  it("persists inbound UDP and TLS create defaults", async () => {
    await render();

    await act(async () => createField("启用入站 UDP").click());
    expect(saveAppSettingsMock).toHaveBeenCalledWith({
      ...DEFAULT_SETTINGS,
      inboundUdpEnabled: false,
    });

    await act(async () => createField("默认允许不安全证书").click());
    expect(saveAppSettingsMock).toHaveBeenCalledWith({
      ...DEFAULT_SETTINGS,
      inboundUdpEnabled: false,
      defAllowInsecure: true,
    });
  });

  it("creates a node from the manual form", async () => {
    createNodeMock.mockResolvedValue(SELECTED);
    loadNodesMock
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([SELECTED.node]);
    await render();

    await act(async () => {
      selectValue("trojan", createSelect("节点协议"));
    });
    await act(async () => {
      typeInput("Frankfurt", createField("新建节点名称"));
      typeInput("edge.example.com", createField("新建节点服务器"));
      typeInput("8443", createField("新建节点端口"));
      typeInput("hunter2", createField("节点密码"));
    });
    await act(async () => button("创建节点").click());

    expect(createNodeMock).toHaveBeenCalledWith({
      credential: { password: "hunter2", protocol: "trojan" },
      name: "Frankfurt",
      port: 8443,
      server: "edge.example.com",
      tls: null,
      transport: { type: "tcp" },
      udpEnabled: true,
      xrayFinalmaskJson: null,
    });
    expect(container.textContent).toContain("Tokyo Edge");
  });

  it("hides the transport picker for Hysteria2 and always sends TLS", async () => {
    createNodeMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      selectValue("hysteria2", createSelect("节点协议"));
    });

    expect(
      container.querySelector("select[aria-label='传输方式']"),
    ).toBeNull();

    await act(async () => {
      typeInput("Frankfurt", createField("新建节点名称"));
      typeInput("edge.example.com", createField("新建节点服务器"));
      typeInput("8443", createField("新建节点端口"));
      typeInput("token", createField("Hysteria2 认证密码"));
    });
    await act(async () => button("创建节点").click());

    expect(createNodeMock).toHaveBeenCalledWith(
      expect.objectContaining({
        credential: {
          authentication: "token",
          obfuscation: null,
          protocol: "hysteria2",
        },
        transport: null,
        tls: expect.objectContaining({ type: "tls" }),
      }),
    );
  });

  it("creates an AnyTLS node without a transport picker", async () => {
    createNodeMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      selectValue("anytls", createSelect("节点协议"));
    });

    expect(
      container.querySelector("select[aria-label='传输方式']"),
    ).toBeNull();

    await act(async () => {
      typeInput("AnyTLS Node", createField("新建节点名称"));
      typeInput("edge.example.com", createField("新建节点服务器"));
      typeInput("443", createField("新建节点端口"));
      typeInput("hunter2", createField("AnyTLS 密码"));
    });
    await act(async () => button("创建节点").click());

    expect(createNodeMock).toHaveBeenCalledWith(
      expect.objectContaining({
        credential: { password: "hunter2", protocol: "anytls" },
        transport: null,
        tls: expect.objectContaining({ type: "tls" }),
      }),
    );
  });

  it("creates a Naive node with QUIC and SNI only", async () => {
    createNodeMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      selectValue("naive", createSelect("节点协议"));
    });

    expect(
      container.querySelector("select[aria-label='传输方式']"),
    ).toBeNull();
    expect(
      container.querySelector("input[aria-label='启用 Reality']"),
    ).toBeNull();

    await act(async () => {
      typeInput("Naive Node", createField("新建节点名称"));
      typeInput("edge.example.com", createField("新建节点服务器"));
      typeInput("443", createField("新建节点端口"));
      typeInput("alice", createField("代理用户名"));
      typeInput("hunter2", createField("节点密码"));
      container.querySelector<HTMLInputElement>("input[aria-label='启用 QUIC']")!.click();
    });
    await act(async () => {
      selectValue("bbr", createSelect("Naive 拥塞控制"));
      typeInput("cdn.example.com", createField("TLS SNI"));
    });
    await act(async () => button("创建节点").click());

    expect(createNodeMock).toHaveBeenCalledWith(
      expect.objectContaining({
        credential: {
          password: "hunter2",
          protocol: "naive",
          quic: true,
          quicCongestionControl: "bbr",
          username: "alice",
        },
        transport: null,
        tls: expect.objectContaining({
          serverName: "cdn.example.com",
          type: "tls",
        }),
      }),
    );
  });

  it("creates a custom Core JSON node", async () => {
    createNodeMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      selectValue("custom", createSelect("节点协议"));
    });

    expect(container.querySelector("input[aria-label='新建节点服务器']")).toBeNull();
    expect(container.querySelector("select[aria-label='传输方式']")).toBeNull();

    await act(async () => {
      typeInput("Full JSON", createField("新建节点名称"));
      selectValue("xray", createSelect("自定义 Core 类型"));
      type(
        '{"inbounds":[],"outbounds":[{"type":"direct"}]}',
        container.querySelector<HTMLTextAreaElement>(
          "textarea[aria-label='Core JSON 配置']",
        )!,
      );
    });
    await act(async () => button("创建节点").click());

    expect(createNodeMock).toHaveBeenCalledWith(
      expect.objectContaining({
        credential: {
          core: "xray",
          document: '{"inbounds":[],"outbounds":[{"type":"direct"}]}',
          protocol: "custom",
        },
        name: "Full JSON",
        port: 443,
        server: "127.0.0.1",
        tls: null,
        transport: null,
      }),
    );
  });

  it("hides the transport picker for TUIC and always sends TLS", async () => {
    createNodeMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      selectValue("tuic", createSelect("节点协议"));
    });

    expect(
      container.querySelector("select[aria-label='传输方式']"),
    ).toBeNull();

    await act(async () => {
      typeInput("Frankfurt", createField("新建节点名称"));
      typeInput("edge.example.com", createField("新建节点服务器"));
      typeInput("8443", createField("新建节点端口"));
      typeInput(
        "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc",
        createField("节点 UUID"),
      );
      typeInput("secret", createField("TUIC 密码"));
    });
    await act(async () => {
      selectValue("bbr", createSelect("拥塞控制"));
    });
    await act(async () => {
      selectValue("native", createSelect("UDP 中继模式"));
    });
    await act(async () => createField("0-RTT 握手").click());
    await act(async () => button("创建节点").click());

    expect(createNodeMock).toHaveBeenCalledWith(
      expect.objectContaining({
        credential: {
          congestionControl: "bbr",
          password: "secret",
          protocol: "tuic",
          udpOverStream: false,
          udpRelayMode: "native",
          uuid: "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc",
          zeroRttHandshake: true,
        },
        transport: null,
        tls: expect.objectContaining({ type: "tls" }),
      }),
    );
  });

  it("creates a SOCKS5 node with optional username and password", async () => {
    createNodeMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      selectValue("socks", createSelect("节点协议"));
    });

    expect(container.querySelector("input[aria-label='启用 TLS']")).toBeNull();

    await act(async () => {
      typeInput("SocksNode", createField("新建节点名称"));
      typeInput("proxy.example.com", createField("新建节点服务器"));
      typeInput("1080", createField("新建节点端口"));
      typeInput("alice", createField("代理用户名"));
      typeInput("secret", createField("节点密码"));
    });
    await act(async () => button("创建节点").click());

    expect(createNodeMock).toHaveBeenCalledWith(
      expect.objectContaining({
        credential: {
          password: "secret",
          protocol: "socks",
          username: "alice",
        },
        transport: { type: "tcp" },
        tls: null,
      }),
    );
  });

  it("creates an HTTP proxy node with TLS and no Reality toggle", async () => {
    createNodeMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      selectValue("http", createSelect("节点协议"));
    });

    expect(
      container.querySelector("input[aria-label='启用 Reality']"),
    ).toBeNull();

    await act(async () => {
      typeInput("HttpNode", createField("新建节点名称"));
      typeInput("proxy.example.com", createField("新建节点服务器"));
      typeInput("443", createField("新建节点端口"));
      createField("启用 TLS").click();
    });
    await act(async () => button("创建节点").click());

    expect(createNodeMock).toHaveBeenCalledWith(
      expect.objectContaining({
        credential: {
          password: null,
          protocol: "http",
          username: null,
        },
        transport: { type: "tcp" },
        tls: expect.objectContaining({ type: "tls" }),
      }),
    );
  });

  it("creates a WireGuard node with no transport or TLS toggle", async () => {
    createNodeMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      selectValue("wireguard", createSelect("节点协议"));
    });

    expect(
      container.querySelector("select[aria-label='传输方式']"),
    ).toBeNull();
    expect(container.querySelector("input[aria-label='启用 TLS']")).toBeNull();
    expect(
      container.querySelector("input[aria-label='启用 Reality']"),
    ).toBeNull();

    await act(async () => {
      typeInput("WgNode", createField("新建节点名称"));
      typeInput("edge.example.com", createField("新建节点服务器"));
      typeInput("51820", createField("新建节点端口"));
      typeInput("priv", createField("WireGuard 私钥"));
      typeInput("peer-pub", createField("WireGuard 对端公钥"));
      typeInput("10.0.0.2/32", createField("WireGuard 本地地址"));
    });
    await act(async () => button("创建节点").click());

    expect(createNodeMock).toHaveBeenCalledWith(
      expect.objectContaining({
        credential: {
          localAddress: ["10.0.0.2/32"],
          mtu: null,
          peerPublicKey: "peer-pub",
          preSharedKey: null,
          privateKey: "priv",
          protocol: "wireguard",
          reserved: null,
        },
        transport: null,
        tls: null,
      }),
    );
  });

  it("disables the TUIC UDP relay mode picker once UDP over stream is checked", async () => {
    createNodeMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      selectValue("tuic", createSelect("节点协议"));
    });
    await act(async () => createField("UDP over stream").click());

    expect(
      (
        container.querySelector(
          "select[aria-label='UDP 中继模式']",
        ) as HTMLSelectElement
      ).disabled,
    ).toBe(true);
  });

  it("sends every VLESS field over a WebSocket transport with TLS", async () => {
    createNodeMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      typeInput("Frankfurt", createField("新建节点名称"));
      typeInput("edge.example.com", createField("新建节点服务器"));
      typeInput("8443", createField("新建节点端口"));
      typeInput("b0dd64e4-0fbd-4038-9139-d1f32a68a0dc", createField("节点 UUID"));
      typeInput("xtls-rprx-vision", createField("VLESS flow"));
    });
    await act(async () => {
      selectValue("websocket", createSelect("传输方式"));
    });
    await act(async () => {
      typeInput("/ray", createField("WebSocket 路径"));
      typeInput("cdn.example.com", createField("WebSocket Host"));
      createField("启用 TLS").click();
    });
    await act(async () => {
      typeInput("sni.example.com", createField("TLS SNI"));
      typeInput("h2, http/1.1", createField("TLS ALPN"));
      typeInput("chrome", createField("TLS 指纹"));
      createField("允许不安全证书").click();
      createField("启用 UDP").click();
    });
    await act(async () => button("创建节点").click());

    expect(createNodeMock).toHaveBeenCalledWith({
      credential: {
        flow: "xtls-rprx-vision",
        protocol: "vless",
        userId: "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc",
      },
      name: "Frankfurt",
      port: 8443,
      server: "edge.example.com",
      tls: {
        allowInsecure: true,
        alpn: ["h2", "http/1.1"],
        fingerprint: "chrome",
        pinnedSha256: null,
        serverName: "sni.example.com",
        type: "tls",
      },
      transport: {
        host: "cdn.example.com",
        path: "/ray",
        type: "websocket",
      },
      udpEnabled: false,
      xrayFinalmaskJson: null,
    });
  });

  it("sends every VMess field over a gRPC transport", async () => {
    createNodeMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      selectValue("vmess", createSelect("节点协议"));
    });
    await act(async () => {
      typeInput("Frankfurt", createField("新建节点名称"));
      typeInput("edge.example.com", createField("新建节点服务器"));
      typeInput("8443", createField("新建节点端口"));
      typeInput("b0dd64e4-0fbd-4038-9139-d1f32a68a0dc", createField("节点 UUID"));
      typeInput("4", createField("VMess alterId"));
    });
    await act(async () => {
      selectValue("Chacha20Poly1305", createSelect("VMess 加密方式"));
    });
    await act(async () => {
      selectValue("grpc", createSelect("传输方式"));
    });
    await act(async () => {
      typeInput("tunnel", createField("gRPC serviceName"));
      typeInput("authority.example.com", createField("gRPC authority"));
    });
    await act(async () => {
      selectValue("multi", createSelect("gRPC 模式"));
    });
    await act(async () => button("创建节点").click());

    expect(createNodeMock).toHaveBeenCalledWith(
      expect.objectContaining({
        credential: {
          alterId: 4,
          protocol: "vmess",
          security: "Chacha20Poly1305",
          userId: "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc",
        },
        transport: {
          authority: "authority.example.com",
          mode: "multi",
          serviceName: "tunnel",
          type: "grpc",
        },
      }),
    );
  });

  it("locks Shadowsocks to TCP and sends the chosen cipher", async () => {
    createNodeMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      selectValue("shadowsocks", createSelect("节点协议"));
    });
    await act(async () => {
      typeInput("Frankfurt", createField("新建节点名称"));
      typeInput("edge.example.com", createField("新建节点服务器"));
      typeInput("8388", createField("新建节点端口"));
      typeInput("hunter2", createField("节点密码"));
    });
    await act(async () => {
      selectValue("chacha20-ietf-poly1305", createSelect("Shadowsocks 加密方式"));
    });

    expect(createSelect("传输方式").disabled).toBe(true);

    await act(async () => button("创建节点").click());

    expect(createNodeMock).toHaveBeenCalledWith(
      expect.objectContaining({
        credential: {
          method: "chacha20-ietf-poly1305",
          password: "hunter2",
          protocol: "shadowsocks",
        },
        tls: null,
        transport: { type: "tcp" },
      }),
    );
  });

  it("sends Hysteria2 obfuscation when enabled", async () => {
    createNodeMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      selectValue("hysteria2", createSelect("节点协议"));
    });
    await act(async () => {
      typeInput("Frankfurt", createField("新建节点名称"));
      typeInput("edge.example.com", createField("新建节点服务器"));
      typeInput("8443", createField("新建节点端口"));
      createField("启用混淆").click();
    });
    await act(async () => {
      typeInput("obfs-secret", createField("混淆密码"));
    });
    await act(async () => {
      selectValue("Gecko", createSelect("混淆方式"));
    });
    await act(async () => button("创建节点").click());

    expect(createNodeMock).toHaveBeenCalledWith(
      expect.objectContaining({
        credential: {
          authentication: null,
          obfuscation: { method: "Gecko", password: "obfs-secret" },
          protocol: "hysteria2",
        },
      }),
    );
  });

  it("clears the manual form on reset", async () => {
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      typeInput("Frankfurt", createField("新建节点名称"));
      typeInput("edge.example.com", createField("新建节点服务器"));
    });
    expect(createField("新建节点名称").value).toBe("Frankfurt");

    await act(async () => button("重置").click());

    expect(createField("新建节点名称").value).toBe("");
    expect(createField("新建节点服务器").value).toBe("");
  });

  it("reports a form error without calling the backend", async () => {
    loadNodesMock.mockResolvedValue([]);
    await render();

    await act(async () => {
      typeInput("Frankfurt", createField("新建节点名称"));
      typeInput("edge.example.com", createField("新建节点服务器"));
      typeInput("70000", createField("新建节点端口"));
    });
    await act(async () => button("创建节点").click());

    expect(createNodeMock).not.toHaveBeenCalled();
    expect(container.textContent).toContain(
      "节点端口必须是 1 到 65535 的整数",
    );
  });

  it("lists persisted nodes and changes the selection", async () => {
    const osaka = {
      id: "00000000-0000-0000-0000-000000000002",
      deletable: true,
      enabled: true,
    lastTestedAt: null,
      latencyMs: null,
      name: "Osaka",
      port: 9000,
      protocol: "shadowsocks" as const,
      server: "osaka.example.com",
      transport: "tcp",
      tls: null,
    };
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    selectNodeMock.mockResolvedValue({ ...SELECTED, node: osaka });
    connectSessionMock.mockResolvedValue({ ...SELECTED, node: osaka, connected: true });
    await render();

    await nodeMenuAction("Osaka", "设为活动");

    expect(selectNodeMock).toHaveBeenCalledWith(osaka.id);
    expect(connectSessionMock).toHaveBeenCalled();
    expect(container.textContent).toContain("osaka.example.com");
    expect(container.textContent).toContain("9000");
  });

  it("switches node while connected from the context menu", async () => {
    const osaka = {
      id: "00000000-0000-0000-0000-000000000002",
      deletable: true,
      enabled: true,
      lastTestedAt: null,
      latencyMs: 42,
      name: "Osaka",
      port: 9000,
      protocol: "shadowsocks" as const,
      server: "osaka.example.com",
      transport: "tcp",
      tls: null,
    };
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    loadNodesMock.mockResolvedValue([CONNECTED.node, osaka]);
    switchNodeMock.mockResolvedValue({ ...CONNECTED, node: osaka });
    await render();

    await nodeMenuAction("Osaka", "设为活动");

    expect(switchNodeMock).toHaveBeenCalledWith(osaka.id);
    expect(selectNodeMock).not.toHaveBeenCalled();
  });

  it("edits a manual node through the full form dialog", async () => {
    const editedNode = {
      ...SELECTED.node!,
      name: "Tokyo 2",
      port: 443,
      server: "new.example.com",
    };
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock
      .mockResolvedValueOnce([SELECTED.node])
      .mockResolvedValueOnce([editedNode]);
    loadNodeDraftMock.mockResolvedValue({
      credential: {
        method: "aes-256-gcm",
        password: "secret",
        protocol: "shadowsocks",
      },
      name: "Tokyo Edge",
      port: 8388,
      server: "edge.example.com",
      tls: null,
      transport: { type: "tcp" },
      udpEnabled: true,
      xrayFinalmaskJson: null,
    });
    updateNodeMock.mockResolvedValue({ ...SELECTED, node: editedNode });
    await render();

    await nodeMenuAction("Tokyo Edge", "编辑");

    const name = container.querySelector<HTMLInputElement>(
      "[aria-label='新建节点名称']",
    );
    const server = container.querySelector<HTMLInputElement>(
      "[aria-label='新建节点服务器']",
    );
    const port = container.querySelector<HTMLInputElement>(
      "[aria-label='新建节点端口']",
    );
    if (!name || !server || !port) {
      throw new Error("node edit fields are missing");
    }
    await act(async () => {
      typeInput(" Tokyo 2 ", name);
      typeInput(" new.example.com ", server);
      typeInput("443", port);
    });
    await act(async () => button("保存节点").click());

    expect(loadNodeDraftMock).toHaveBeenCalledWith(SELECTED.node?.id);
    expect(updateNodeMock).toHaveBeenCalledWith(
      SELECTED.node?.id,
      expect.objectContaining({
        name: "Tokyo 2",
        port: 443,
        server: "new.example.com",
      }),
    );
    expect(
      container.querySelector("[aria-label='节点列表']")?.textContent,
    ).toContain("Tokyo 2");
    expect(container.textContent).toContain("new.example.com");
    expect(container.textContent).toContain("443");
  });

  it("moves nodes with compact ordering controls", async () => {
    const osaka = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
      port: 9000,
      server: "osaka.example.com",
    };
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    moveNodeMock.mockResolvedValue([osaka, SELECTED.node]);
    await render();

    expect(await nodeMenuItemDisabled("Tokyo Edge", "上移")).toBe(true);
    await nodeMenuAction("Tokyo Edge", "下移");

    expect(moveNodeMock).toHaveBeenCalledWith(SELECTED.node?.id, "down");
    const rows = [...container.querySelectorAll("[aria-label='节点列表'] tbody tr")];
    expect(rows.map((row) => row.textContent)).toEqual([
      expect.stringContaining("Osaka"),
      expect.stringContaining("Tokyo Edge"),
    ]);
  });

  it("lists the groups as wrap chips like v2rayN", async () => {
    const work = {
      id: "00000000-0000-0000-0000-000000000020",
      name: "Work",
      strategy: "select",
    };
    loadNodeGroupsMock.mockResolvedValue([work]);
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    await render();

    expect(
      container.querySelector(".profiles-toolbar [aria-label='节点分组筛选']"),
    ).toBeNull();
    const chips = container.querySelector(
      ".group-chips[aria-label='节点分组筛选']",
    );
    if (!chips) {
      throw new Error("the group chips are missing");
    }
    expect(
      [...chips.querySelectorAll("button")].map((entry) => entry.textContent),
    ).toEqual(["全部", "未分组", "Work"]);
  });

  it("orders the server table like v2rayN: type, remarks, address, port", async () => {
    const osaka = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
    };
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    await render();

    const headers = [
      ...container.querySelectorAll("[aria-label='节点列表'] thead th"),
    ].map((cell) => cell.textContent);
    expect(headers.slice(1, 5)).toEqual(["类型", "别名", "地址", "端口"]);
  });

  it("assigns and filters named node groups", async () => {
    const work = {
      id: "00000000-0000-0000-0000-000000000020",
      name: "Work",
      strategy: "select",
    };
    const osaka = {
      ...SELECTED.node!,
      groupId: work.id,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
    };
    const groupedTokyo = { ...SELECTED.node!, groupId: work.id };
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodeGroupsMock.mockResolvedValue([work]);
    setNodeGroupStrategyMock.mockResolvedValue([
      { ...work, strategy: "urlTest" },
    ]);
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    setNodeGroupMock.mockResolvedValue([groupedTokyo, osaka]);
    await render();

    const filter = container.querySelector("[aria-label='节点分组筛选']");
    if (!filter) {
      throw new Error("node group filter is missing");
    }
    await act(async () => button(work.name).click());
    expect(
      [...container.querySelectorAll("[aria-label='节点列表'] tbody tr")].map(
        (row) => row.textContent,
      ),
    ).toEqual([expect.stringContaining("Osaka")]);

    await act(async () => button("全部").click());
    await nodeMenuAction("Tokyo Edge", "设置分组");
    const groupName = container.querySelector<HTMLInputElement>(
      "[aria-label='节点分组']",
    );
    if (!groupName) {
      throw new Error("node group field is missing");
    }
    await act(async () => typeInput("Work", groupName));
    const strategy = container.querySelector<HTMLSelectElement>(
      "[aria-label='分组策略']",
    );
    if (!strategy) {
      throw new Error("group strategy select is missing");
    }
    await act(async () => {
      strategy.value = "urlTest";
      strategy.dispatchEvent(new Event("change", { bubbles: true }));
    });
    await act(async () => button("保存分组").click());

    expect(setNodeGroupMock).toHaveBeenCalledWith(SELECTED.node?.id, "Work");
    expect(setNodeGroupStrategyMock).toHaveBeenCalledWith(work.id, "urlTest");
    expect(
      container.querySelector("[aria-label='节点列表']")?.textContent,
    ).toContain("Work");
  });

  it("tests one node and shows its TCP latency", async () => {
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    testNodeMock.mockResolvedValue({
      id: SELECTED.node?.id,
      latencyMs: 42,
      status: "success",
    });
    await render();

    await nodeMenuAction("Tokyo Edge", "测试延迟");

    expect(testNodeMock).toHaveBeenCalledWith(SELECTED.node?.id);
    expect(
      container.querySelector("[aria-label='节点列表']")?.textContent,
    ).toContain("42 ms");
  });

  it("shows a node test command failure and restores its action", async () => {
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    testNodeMock.mockRejectedValue({
      code: "node_store_failed",
      message: "failed to save the node test",
    });
    await render();

    await nodeMenuAction("Tokyo Edge", "测试延迟");

    expect(container.querySelector("[role='alert']")?.textContent).toContain(
      "failed to save the node test",
    );
    // The action has to come back after a failure, not stay stuck in "测试中".
    expect(await nodeMenuItemDisabled("Tokyo Edge", "测试延迟")).toBe(false);
  });

  it("offers batch node tests and cancels queued work", async () => {
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    let cancelled = () => false;
    let release = () => {};
    testAllNodesMock.mockImplementation(
      async (
        _ids: string[],
        _onResult: (result: unknown) => void,
        isCancelled: () => boolean,
      ) =>
        new Promise<void>((resolve) => {
          cancelled = isCancelled;
          release = resolve;
        }),
    );
    await render();

    await act(async () => button("全部测速").click());
    expect(testAllNodesMock).toHaveBeenCalledWith(
      [SELECTED.node?.id],
      expect.any(Function),
      expect.any(Function),
    );

    await act(async () => button("取消测速").click());
    expect(cancelled()).toBe(true);
    await act(async () => release());
    expect(button("全部测速").disabled).toBe(false);
  });

  it("skips disabled nodes when testing all", async () => {
    const disabled = {
      ...SELECTED.node!,
      enabled: false,
      id: "00000000-0000-0000-0000-000000000099",
      name: "Disabled Edge",
    };
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node, disabled]);
    testAllNodesMock.mockResolvedValue(undefined);
    await render();

    await act(async () => button("全部测速").click());
    expect(testAllNodesMock).toHaveBeenCalledWith(
      [SELECTED.node?.id],
      expect.any(Function),
      expect.any(Function),
    );
  });

  it("sorts the node list by latency", async () => {
    const slow = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000002",
      latencyMs: 200,
      name: "Slow Edge",
    };
    const fast = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000003",
      latencyMs: 40,
      name: "Fast Edge",
    };
    const untested = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000004",
      latencyMs: null,
      name: "Untested Edge",
    };
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([slow, fast, untested]);
    reorderNodesMock.mockResolvedValue([fast, slow, untested]);
    await render();

    await nodeMenuAction("Slow Edge", "按延迟排序");
    expect(reorderNodesMock).toHaveBeenCalledWith([
      fast.id,
      slow.id,
      untested.id,
    ]);
  });

  it("tests a configurable URL through the connected node and remembers it", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    testUrlMock.mockResolvedValue({
      id: SELECTED.node?.id,
      latencyMs: 55,
      status: "success",
    });
    await render();

    await act(async () => button("参数设置").click());
    const field = container.querySelector<HTMLInputElement>(
      "[aria-label='设置中的 URL 测试地址']",
    );
    if (!field) {
      throw new Error("URL test address field is missing");
    }
    expect(field.value).toBe("https://www.gstatic.com/generate_204");
    await act(async () => typeInput(" https://probe.example/204 ", field));
    await nodeMenuAction("Tokyo Edge", "URL 测试");

    expect(testUrlMock).toHaveBeenCalledWith("https://probe.example/204");
    expect(saveAppSettingsMock).toHaveBeenCalledWith(
      expect.objectContaining({
        urlTestAddress: "https://probe.example/204",
      }),
    );
    expect(
      container.querySelector("[aria-label='节点列表']")?.textContent,
    ).toContain("55 ms");

    act(() => root.unmount());
    container = document.createElement("div");
    document.body.append(container);
    root = createRoot(container);
    loadAppSettingsMock.mockResolvedValue({
      ...DEFAULT_SETTINGS,
      urlTestAddress: "https://probe.example/204",
    });
    await render();
    await act(async () => button("参数设置").click());
    expect(
      container.querySelector<HTMLInputElement>(
        "[aria-label='设置中的 URL 测试地址']",
      )?.value,
    ).toBe("https://probe.example/204");
  });

  it("does not remember a URL rejected by the backend", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    testUrlMock.mockRejectedValue({
      code: "invalid_url_test",
      message: "URL 测试地址无效",
    });
    await render();

    await act(async () => button("参数设置").click());
    const field = container.querySelector<HTMLInputElement>(
      "[aria-label='设置中的 URL 测试地址']",
    );
    if (!field) {
      throw new Error("URL test address field is missing");
    }
    await act(async () => typeInput("file:///tmp/probe", field));
    await nodeMenuAction("Tokyo Edge", "URL 测试");

    expect(saveAppSettingsMock).not.toHaveBeenCalledWith(
      expect.objectContaining({ urlTestAddress: "file:///tmp/probe" }),
    );
    expect(container.querySelector("[role='alert']")?.textContent).toContain(
      "URL 测试地址无效",
    );
  });

  it("requires a connected node for URL testing", async () => {
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    await render();

    expect(await nodeMenuItemDisabled("Tokyo Edge", "URL 测试")).toBe(true);
  });

  it("runs a download speed test through the connected node", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    testDownloadSpeedMock.mockResolvedValue({
      id: SELECTED.node?.id,
      bytesPerSecond: 2_500_000,
      bytesRead: 10_000_000,
      elapsedMs: 4000,
      status: "success",
    });
    await render();

    await nodeMenuAction("Tokyo Edge", "下载测速");
    expect(testDownloadSpeedMock).toHaveBeenCalledWith(
      "https://speed.cloudflare.com/__down?bytes=10000000",
    );
    expect(
      container.querySelector("[aria-label='节点列表']")?.textContent,
    ).toContain("2.4 MB/s");
  });

  it("persists Allow LAN from settings", async () => {
    await render();
    await act(async () => createField("允许来自局域网的连接").click());
    expect(saveAppSettingsMock).toHaveBeenCalledWith(
      expect.objectContaining({ allowLan: true }),
    );
  });

  it("persists the Fragment anti-detection toggle from settings", async () => {
    await render();
    await act(async () => createField("启用 Fragment").click());
    expect(saveAppSettingsMock).toHaveBeenCalledWith(
      expect.objectContaining({ fragmentEnabled: true }),
    );
  });

  it("persists the Final Fragment toggle from settings", async () => {
    await render();
    await act(async () => createField("启用 Final Fragment").click());
    expect(saveAppSettingsMock).toHaveBeenCalledWith(
      expect.objectContaining({ finalFragmentEnabled: true }),
    );
  });

  it("persists the UDP Noise anti-detection toggle from settings", async () => {
    await render();
    await act(async () => createField("启用 UDP Noise").click());
    expect(saveAppSettingsMock).toHaveBeenCalledWith(
      expect.objectContaining({ udpNoiseEnabled: true }),
    );
  });

  it("deletes a persisted node", async () => {
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock
      .mockResolvedValueOnce([SELECTED.node])
      .mockResolvedValueOnce([]);
    deleteNodeMock.mockResolvedValue(IDLE);
    await render();

    await nodeMenuAction("Tokyo Edge", "移除所选");

    expect(deleteNodeMock).toHaveBeenCalledWith(SELECTED.node?.id);
    expect(container.querySelector("[aria-label='节点列表']")).toBeNull();
    expect(container.textContent).toContain("尚未导入节点");
  });

  it("opens the node menu on right-click and dismisses it on Escape", async () => {
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    await render();
    const row = container.querySelector("[aria-label='节点列表'] tbody tr");
    if (!row) {
      throw new Error("no node row");
    }

    await act(async () => {
      row.dispatchEvent(
        new MouseEvent("contextmenu", { bubbles: true, cancelable: true }),
      );
    });

    expect(
      container.querySelector("[aria-label='节点操作 Tokyo Edge']"),
    ).not.toBeNull();

    await act(async () => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    });

    expect(container.querySelector("[aria-label='节点操作 Tokyo Edge']")).toBeNull();
  });

  it("activates a node on double-click", async () => {
    const osaka = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
    };
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    selectNodeMock.mockResolvedValue({ ...SELECTED, node: osaka });
    connectSessionMock.mockResolvedValue({ ...SELECTED, node: osaka, connected: true });
    await render();
    const rows = container.querySelectorAll("[aria-label='节点列表'] tbody tr");

    await act(async () => {
      rows[1].dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    });

    expect(selectNodeMock).toHaveBeenCalledWith(osaka.id);
    expect(connectSessionMock).toHaveBeenCalled();
  });

  it("copies an exported share link without rendering it", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    exportNodeLinkMock.mockResolvedValue(
      "ss://aes-256-gcm:hunter2@edge.example.com:8388#Tokyo Edge",
    );
    await render();

    await nodeMenuAction("Tokyo Edge", "导出分享链接");

    expect(exportNodeLinkMock).toHaveBeenCalledWith(SELECTED.node?.id);
    expect(writeText).toHaveBeenCalledWith(
      "ss://aes-256-gcm:hunter2@edge.example.com:8388#Tokyo Edge",
    );
    // The link is the credential, so it must not appear on screen.
    expect(container.textContent).not.toContain("hunter2");
    expect(container.textContent).toContain("已复制到剪贴板");
  });

  it("clones a node without moving the selection", async () => {
    const clone = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-0000000000cc",
    };
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    cloneNodeMock.mockResolvedValue([SELECTED.node, clone]);
    await render();

    await nodeMenuAction("Tokyo Edge", "克隆所选");

    expect(cloneNodeMock).toHaveBeenCalledWith(SELECTED.node?.id);
    expect(
      container.querySelectorAll("[aria-label='节点列表'] tbody tr"),
    ).toHaveLength(2);
  });

  it("reports how many duplicates were removed", async () => {
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    removeDuplicateNodesMock.mockResolvedValue(3);
    await render();

    await nodeMenuAction("Tokyo Edge", "移除重复");

    expect(container.textContent).toContain("已移除 3 个重复节点");
  });

  it("says so when there is nothing to de-duplicate", async () => {
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    removeDuplicateNodesMock.mockResolvedValue(0);
    await render();

    await nodeMenuAction("Tokyo Edge", "移除重复");

    // Silence after a command that removed nothing reads as a failure.
    expect(container.textContent).toContain("没有重复节点");
  });

  it("switches the System Proxy mode from the status bar", async () => {
    await render();
    const control = container.querySelector<HTMLSelectElement>(
      "[aria-label='状态栏系统代理']",
    );
    if (!control) {
      throw new Error("no System Proxy control in the status bar");
    }

    await act(async () => selectValue("unchanged", control));

    // Leaving the host's proxy alone and clearing it are different requests,
    // which is why there are four values rather than a checkbox.
    expect(saveAppSettingsMock).toHaveBeenCalledWith(
      expect.objectContaining({ systemProxyMode: "unchanged" }),
    );
  });

  it("offers all four System Proxy modes v2rayN has", async () => {
    await render();
    const control = container.querySelector<HTMLSelectElement>(
      "[aria-label='状态栏系统代理']",
    );

    expect(
      [...(control?.options ?? [])].map((option) => option.value),
    ).toEqual(["cleared", "managed", "unchanged", "pac"]);
  });

  it("shows each node's own traffic in the table", async () => {
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    loadNodeTrafficMock.mockResolvedValue({
      [SELECTED.node!.id]: {
        todayUploadBytes: 1_024,
        todayDownloadBytes: 2_048,
        totalUploadBytes: 1_048_576,
        totalDownloadBytes: 2_097_152,
      },
    });
    await render();

    const row = container.querySelector("[aria-label='节点列表'] tbody tr");

    expect(row?.textContent).toContain("1.0 KB");
    expect(row?.textContent).toContain("2.0 KB");
    expect(row?.textContent).toContain("1.0 MB");
    expect(row?.textContent).toContain("2.0 MB");
  });

  it("shows zeroes for a node that has carried nothing", async () => {
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    loadNodeTrafficMock.mockResolvedValue({});
    await render();

    const row = container.querySelector("[aria-label='节点列表'] tbody tr");

    // A node with no history reads as zero, not as a missing column.
    expect(row?.textContent).toContain("0 B");
  });

  it("acts on every checked node when one of them is right-clicked", async () => {
    const osaka = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
    };
    loadSessionStatusMock.mockResolvedValue({ ...SELECTED, node: null });
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    deleteNodeMock.mockResolvedValue({ ...SELECTED, node: null });
    await render();
    checkNode("Tokyo Edge");
    checkNode("Osaka");

    await nodeMenuAction("Osaka", "移除所选 (2)");

    // v2rayN marks this action 多选; acting on one row would silently ignore
    // the rest of what the user picked.
    expect(deleteNodeMock).toHaveBeenCalledTimes(2);
    expect(deleteNodeMock).toHaveBeenCalledWith(SELECTED.node?.id);
    expect(deleteNodeMock).toHaveBeenCalledWith(osaka.id);
  });

  it("acts on the right-clicked node alone when it is not checked", async () => {
    const osaka = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
    };
    loadSessionStatusMock.mockResolvedValue({ ...SELECTED, node: null });
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    deleteNodeMock.mockResolvedValue({ ...SELECTED, node: null });
    await render();
    checkNode("Tokyo Edge");

    await nodeMenuAction("Osaka", "移除所选");

    // Right-clicking outside the selection is a fresh act, not an extension of it.
    expect(deleteNodeMock).toHaveBeenCalledTimes(1);
    expect(deleteNodeMock).toHaveBeenCalledWith(osaka.id);
  });

  it("selects and clears every visible node at once", async () => {
    const osaka = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
    };
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    await render();
    const all = container.querySelector<HTMLInputElement>(
      "[aria-label='全选节点']",
    );
    if (!all) {
      throw new Error("no select-all control");
    }

    await act(async () => all.click());
    expect(checkedNodeNames()).toEqual(["Tokyo Edge", "Osaka"]);

    await act(async () => all.click());
    expect(checkedNodeNames()).toEqual([]);
  });

  it("reports how many nodes a batch removal could not delete", async () => {
    const osaka = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
    };
    loadSessionStatusMock.mockResolvedValue({ ...SELECTED, node: null });
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    deleteNodeMock
      .mockResolvedValueOnce({ ...SELECTED, node: null })
      .mockRejectedValueOnce({ code: "node_store_failed", message: "boom" });
    await render();
    checkNode("Tokyo Edge");
    checkNode("Osaka");

    await nodeMenuAction("Osaka", "移除所选 (2)");

    // A batch that stops at the first failure leaves the user guessing which
    // half happened.
    expect(deleteNodeMock).toHaveBeenCalledTimes(2);
    expect(container.textContent).toContain("1 个节点未能移除");
  });

  it("copies several links as one line each", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const osaka = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
    };
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    exportNodeLinkMock
      .mockResolvedValueOnce("ss://one#Tokyo Edge")
      .mockResolvedValueOnce("ss://two#Osaka");
    await render();
    checkNode("Tokyo Edge");
    checkNode("Osaka");

    await nodeMenuAction("Osaka", "导出分享链接 (2)");

    // One link per line is what a bulk import reads back.
    expect(writeText).toHaveBeenCalledWith("ss://one#Tokyo Edge\nss://two#Osaka");
    expect(container.textContent).toContain("已复制 2 条分享链接");
  });

  it("says how many links could not be exported", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const osaka = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
    };
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    exportNodeLinkMock
      .mockResolvedValueOnce("ss://one#Tokyo Edge")
      .mockRejectedValueOnce({ code: "share_link_unavailable", message: "no" });
    await render();
    checkNode("Tokyo Edge");
    checkNode("Osaka");

    await nodeMenuAction("Osaka", "导出分享链接 (2)");

    // A node with no representable link must not silently vanish from the copy.
    expect(container.textContent).toContain("已复制 1 条，1 条无法导出");
  });

  it("latency-tests only the checked nodes", async () => {
    const osaka = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
    };
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    testAllNodesMock.mockResolvedValue(undefined);
    await render();
    checkNode("Tokyo Edge");
    checkNode("Osaka");

    await nodeMenuAction("Osaka", "测试延迟 (2)");

    expect(testAllNodesMock).toHaveBeenCalledWith(
      [SELECTED.node?.id, osaka.id],
      expect.any(Function),
      expect.any(Function),
    );
  });

  it("never removes a node the group filter is hiding", async () => {
    const group = {
      id: "00000000-0000-0000-0000-000000000021",
      name: "Asia",
      strategy: "select",
    };
    const osaka = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
      groupId: group.id,
    };
    loadSessionStatusMock.mockResolvedValue({ ...SELECTED, node: null });
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    loadNodeGroupsMock.mockResolvedValue([group]);
    deleteNodeMock.mockResolvedValue({ ...SELECTED, node: null });
    await render();
    checkNode("Tokyo Edge");
    checkNode("Osaka");
    const filter = container.querySelector("[aria-label='节点分组筛选']");
    if (!filter) {
      throw new Error("no group filter");
    }

    await act(async () => button(group.name).click());
    await nodeMenuAction("Osaka", "移除所选");

    // Tokyo Edge is checked but filtered out of view; acting on it would remove
    // a node the user cannot see.
    expect(deleteNodeMock).toHaveBeenCalledTimes(1);
    expect(deleteNodeMock).toHaveBeenCalledWith(osaka.id);
  });

  it("shows a node's QR code with a warning about what it carries", async () => {
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    nodeQrCodeMock.mockResolvedValue(
      '<svg xmlns="http://www.w3.org/2000/svg"><rect width="1" height="1"/></svg>',
    );
    await render();

    await nodeMenuAction("Tokyo Edge", "显示二维码");

    const dialog = container.querySelector("[aria-label='二维码 Tokyo Edge']");
    expect(dialog).not.toBeNull();
    expect(dialog?.querySelector("svg")).not.toBeNull();
    // The code is the credential; a user photographing it hands over the node.
    expect(dialog?.textContent).toContain("包含凭据");
  });

  it("reports a node that has no drawable code", async () => {
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    nodeQrCodeMock.mockRejectedValue({
      code: "share_link_unavailable",
      message: "this node has no sharing link",
    });
    await render();

    await nodeMenuAction("Tokyo Edge", "显示二维码");

    expect(container.querySelector("[role='alert']")?.textContent).toContain(
      "no sharing link",
    );
    expect(container.querySelector("[aria-label^='二维码']")).toBeNull();
  });

  it("imports the node a QR code image holds", async () => {
    readQrCodeMock.mockResolvedValue(
      "ss://aes-256-gcm:hunter2@edge.example.com:8388#Tokyo",
    );
    importNodesMock.mockResolvedValue({
      imported: 1,
      duplicates: 0,
      failures: [],
      status: SELECTED,
    });
    await render();
    const picker = container.querySelector<HTMLInputElement>(
      "[aria-label='从二维码图片导入节点']",
    );
    if (!picker) {
      throw new Error("no QR image picker");
    }

    await act(async () => {
      Object.defineProperty(picker, "files", {
        configurable: true,
        value: [new File([new Uint8Array([1, 2, 3])], "node.png")],
      });
      picker.dispatchEvent(new Event("change", { bubbles: true }));
    });

    // The decoded link goes through the same bulk import a paste uses, so it
    // gets the same validation and the same report.
    expect(readQrCodeMock).toHaveBeenCalled();
    expect(importNodesMock).toHaveBeenCalledWith(
      "ss://aes-256-gcm:hunter2@edge.example.com:8388#Tokyo",
    );
  });

  it("reports an image that holds no code", async () => {
    readQrCodeMock.mockRejectedValue({
      code: "qr_code_not_found",
      message: "no QR code was found in the image",
    });
    await render();
    const picker = container.querySelector<HTMLInputElement>(
      "[aria-label='从二维码图片导入节点']",
    );

    await act(async () => {
      Object.defineProperty(picker!, "files", {
        configurable: true,
        value: [new File([new Uint8Array([1])], "holiday.png")],
      });
      picker!.dispatchEvent(new Event("change", { bubbles: true }));
    });

    expect(container.querySelector("[role='alert']")?.textContent).toContain(
      "no QR code was found",
    );
    expect(importNodesMock).not.toHaveBeenCalled();
  });

  it("contacts nothing until the user asks for an update check", async () => {
    await render();

    // A proxy client that phones home on launch tells an observer it is
    // running; that is the user's decision, not a default.
    expect(checkUpdateMock).not.toHaveBeenCalled();

    checkUpdateMock.mockResolvedValue({
      current: "0.1.0",
      latest: "0.2.0",
      url: "https://example.invalid/releases/v0.2.0",
      updateAvailable: true,
    });
    await act(async () => button("检查更新").click());

    expect(checkUpdateMock).toHaveBeenCalledTimes(1);
    const dialog = container.querySelector("[aria-label='检查更新结果']");
    expect(dialog?.textContent).toContain("有新版本 0.2.0");
    expect(dialog?.textContent).toContain("https://example.invalid/releases/v0.2.0");
  });

  it("says so when the build is already current", async () => {
    checkUpdateMock.mockResolvedValue({
      current: "0.1.0",
      latest: "0.1.0",
      url: "https://example.invalid/releases",
      updateAvailable: false,
    });
    await render();

    await act(async () => button("检查更新").click());

    expect(
      container.querySelector("[aria-label='检查更新结果']")?.textContent,
    ).toContain("已是最新版本 0.1.0");
  });

  it("reports a failed update check without a dialog", async () => {
    checkUpdateMock.mockRejectedValue({
      code: "release_check_failed",
      message: "network unreachable",
    });
    await render();

    await act(async () => button("检查更新").click());

    expect(container.querySelector("[role='alert']")?.textContent).toContain(
      "network unreachable",
    );
    expect(container.querySelector("[aria-label='检查更新结果']")).toBeNull();
  });

  it("contacts nothing until the user asks for a Core update check", async () => {
    await render();

    expect(checkCoreUpdateMock).not.toHaveBeenCalled();

    checkCoreUpdateMock.mockResolvedValue({
      singBox: {
        name: "sing-box",
        current: "1.13.18",
        latest: "1.14.0",
        url: "https://example.invalid/sing-box",
        updateAvailable: true,
        fromBinary: true,
      },
      xray: {
        name: "Xray",
        current: "26.3.27",
        latest: "26.3.27",
        url: "https://example.invalid/xray",
        updateAvailable: false,
        fromBinary: false,
      },
      install: {
        directory: "/tmp/cores",
        singBox: undefined,
        xray: undefined,
      },
    });
    await act(async () => button("检查 Core 更新").click());

    expect(checkCoreUpdateMock).toHaveBeenCalledTimes(1);
    const dialog = container.querySelector("[aria-label='检查 Core 更新结果']");
    expect(dialog?.textContent).toContain("sing-box 有新版本 1.14.0");
    expect(dialog?.textContent).toContain("https://example.invalid/sing-box");
  });

  it("renders the window in the saved language", async () => {
    loadAppSettingsMock.mockResolvedValue({
      ...DEFAULT_SETTINGS,
      locale: "en" as const,
    });
    await render();

    // The language is a saved setting, so the window opens in it rather than
    // starting Chinese and switching once the settings arrive.
    expect(container.textContent).toContain("Connect");
    expect(container.textContent).not.toContain("检查更新");
  });

  it("switches language without a restart", async () => {
    saveAppSettingsMock.mockImplementation(
      async (next: unknown) => next as typeof DEFAULT_SETTINGS,
    );
    await render();
    expect(container.textContent).toContain("检查更新");

    const picker = container.querySelector<HTMLSelectElement>(
      "[aria-label='界面语言']",
    );
    if (!picker) {
      throw new Error("no language picker");
    }
    await act(async () => selectValue("en", picker));

    expect(saveAppSettingsMock).toHaveBeenCalledWith(
      expect.objectContaining({ locale: "en" }),
    );
    expect(container.textContent).toContain("Check for updates");
  });

  it("leaves an untranslated label in the source language", async () => {
    loadAppSettingsMock.mockResolvedValue({
      ...DEFAULT_SETTINGS,
      locale: "en" as const,
    });
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    await render();

    // A gap in the dictionary has to be visible rather than blank: the label
    // still names its control, and the missing entry is obvious to whoever
    // reports it.
    const table = container.querySelector("[aria-label='Nodes']");
    expect(table).not.toBeNull();
  });

  it("keeps subscription-owned nodes read-only", async () => {
    const managed = {
      ...SELECTED.node,
      deletable: false,
      enabled: true,
    name: "Managed Tokyo",
    };
    loadNodesMock.mockResolvedValue([managed]);
    await render();

    const opener = container.querySelector<HTMLButtonElement>(
      "[aria-label='操作 Managed Tokyo']",
    );
    await act(async () => opener?.click());
    const remove = Array.from(
      container
        .querySelector("[aria-label='节点操作 Managed Tokyo']")!
        .querySelectorAll<HTMLButtonElement>("button"),
    ).find((button) => button.textContent?.includes("移除"));

    expect(remove?.disabled).toBe(true);
    expect(remove?.textContent).toBe("订阅节点不可移除");
    // Editing a node the subscription owns is not offered at all. The menu has
    // an 编辑 group heading, so the absence has to be read off the actions.
    const actions = [
      ...container
        .querySelector("[aria-label='节点操作 Managed Tokyo']")!
        .querySelectorAll<HTMLButtonElement>("button"),
    ].map((button) => button.textContent);
    expect(actions).not.toContain("编辑");
  });

  it("adds a subscription without exposing its URL in the list", async () => {
    createSubscriptionMock.mockResolvedValue(SUBSCRIPTION);
    await render();

    const name = container.querySelector<HTMLInputElement>(
      "[aria-label='订阅名称']",
    );
    const url = container.querySelector<HTMLTextAreaElement>(
      "[aria-label='订阅地址']",
    );
    if (!name || !url) {
      throw new Error("subscription form is missing");
    }
    await act(async () => {
      typeInput("Airport", name);
      type("https://example.com/secret", url);
    });
    await act(async () => button("添加订阅").click());

    expect(createSubscriptionMock).toHaveBeenCalledWith({
      autoUpdate: true,
      excludeKeywords: "",
      includeKeywords: "",
      name: "Airport",
      updateIntervalMinutes: 60,
      url: "https://example.com/secret",
      userAgent: null,
      subconverterUrl: null,
    });
    expect(
      container.querySelector("[aria-label='订阅列表']")?.textContent,
    ).toContain("Airport");
    expect(container.textContent).not.toContain("https://example.com/secret");
  });

  it("edits subscription metadata without replacing its saved URL", async () => {
    loadSubscriptionsMock.mockResolvedValue([SUBSCRIPTION]);
    updateSubscriptionMock.mockResolvedValue({
      ...SUBSCRIPTION,
      name: "Airport 2",
    });
    await render();

    const editSubscription = container.querySelector<HTMLButtonElement>(
      "[aria-label='编辑 Airport']",
    );
    if (!editSubscription) {
      throw new Error("no Airport edit button");
    }
    await act(async () => editSubscription.click());

    const name = container.querySelector<HTMLInputElement>(
      "[aria-label='订阅名称']",
    );
    if (!name) {
      throw new Error("subscription name field is missing");
    }
    await act(async () => typeInput("Airport 2", name));
    await act(async () => button("保存修改").click());

    expect(updateSubscriptionMock).toHaveBeenCalledWith({
      autoUpdate: true,
      enabled: true,
    excludeKeywords: "",
      id: SUBSCRIPTION.id,
      includeKeywords: "",
      name: "Airport 2",
      updateIntervalMinutes: 60,
      url: null,
      userAgent: null,
      subconverterUrl: null,
    });
  });

  it("refreshes and deletes a subscription", async () => {
    const managedNode = {
      ...SELECTED.node,
      deletable: false,
      enabled: true,
    name: "Subscription Tokyo",
    };
    loadSubscriptionsMock.mockResolvedValue([SUBSCRIPTION]);
    loadNodesMock.mockResolvedValueOnce([]).mockResolvedValue([managedNode]);
    refreshSubscriptionMock.mockResolvedValue({
      ...SUBSCRIPTION,
      lastUpdatedAt: 1_723_456_789,
      nodeCount: 4,
    });
    deleteSubscriptionMock.mockResolvedValue(undefined);
    await render();

    const refresh = container.querySelector<HTMLButtonElement>(
      "[aria-label='刷新 Airport']",
    );
    if (!refresh) {
      throw new Error("subscription refresh button is missing");
    }
    await act(async () => refresh.click());
    expect(refreshSubscriptionMock).toHaveBeenCalledWith(SUBSCRIPTION.id);
    expect(loadNodesMock).toHaveBeenCalledTimes(2);
    expect(container.textContent).toContain("Subscription Tokyo");
    expect(container.textContent).toContain("4");

    const remove = container.querySelector<HTMLButtonElement>(
      "[aria-label='删除订阅 Airport']",
    );
    if (!remove) {
      throw new Error("subscription delete button is missing");
    }
    await act(async () => remove.click());
    expect(deleteSubscriptionMock).toHaveBeenCalledWith(SUBSCRIPTION.id);
    expect(container.querySelector("[aria-label='订阅列表']")).toBeNull();
  });

  it("waits for disconnect before editing or deleting a subscription", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    loadSubscriptionsMock.mockResolvedValue([SUBSCRIPTION]);
    await render();

    // Editing or removing a subscription can pull the running node out from
    // under the session; pulling fresh nodes into the list cannot.
    for (const label of ["编辑 Airport", "删除订阅 Airport"]) {
      const action = container.querySelector<HTMLButtonElement>(
        `[aria-label='${label}']`,
      );
      expect(action?.disabled).toBe(true);
    }
    expect(
      container.querySelector<HTMLButtonElement>("[aria-label='刷新 Airport']")
        ?.disabled,
    ).toBe(false);
  });

  it("refreshes all subscriptions and shows individual failures", async () => {
    loadSubscriptionsMock.mockResolvedValue([SUBSCRIPTION]);
    refreshAllSubscriptionsMock.mockResolvedValue([
      { ...SUBSCRIPTION, lastError: "subscription request timed out" },
    ]);
    await render();

    await act(async () => button("更新全部订阅 (不通过代理)").click());

    expect(refreshAllSubscriptionsMock).toHaveBeenCalledOnce();
    expect(container.textContent).toContain("subscription request timed out");
  });

  it("refuses to import a blank sharing URI", async () => {
    await render();

    await act(async () => button("导入").click());

    expect(importNodeMock).not.toHaveBeenCalled();
    expect(container.querySelector("[role='alert']")?.textContent).toContain(
      "请先粘贴分享链接",
    );
  });

  it("connects and then disconnects the session", async () => {
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    connectSessionMock.mockResolvedValue(CONNECTED);
    disconnectSessionMock.mockResolvedValue(SELECTED);
    await render();

    await toggleConnect();

    expect(container.textContent).toContain("已连接");

    await toggleConnect();

    expect(disconnectSessionMock).toHaveBeenCalledOnce();
    expect(container.textContent).toContain("未连接");
  });

  it("switches routing mode while disconnected", async () => {
    const ruleStatus: SessionStatus = { ...IDLE, mode: "rule" };
    setRoutingModeMock.mockResolvedValue(ruleStatus);
    await render();
    const field = container.querySelector<HTMLSelectElement>(
      "select[aria-label='状态栏路由模式']",
    );
    expect(field).not.toBeNull();

    await act(async () => selectValue("rule", field!));

    expect(setRoutingModeMock).toHaveBeenCalledWith("rule");
    expect(field?.value).toBe("rule");
  });

  it("changes routing mode while connected", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    setRoutingModeMock.mockResolvedValue({ ...CONNECTED, mode: "rule" });
    await render();

    const mode = container.querySelector<HTMLSelectElement>(
      "select[aria-label='状态栏路由模式']",
    );
    expect(mode?.disabled).toBe(false);
    await act(async () => selectValue("rule", mode!));

    expect(setRoutingModeMock).toHaveBeenCalledWith("rule");
  });

  it("shows and saves the compact DNS settings while disconnected", async () => {
    const saved: SessionStatus = {
      ...IDLE,
      dns: {
        ...IDLE.dns,
        dohPath: "/custom-query",
        mode: "doh",
        port: 443,
        server: "cloudflare-dns.com",
        systemDomains: ["lan", "corp.example"],
      },
    };
    setDnsSettingsMock.mockResolvedValue(saved);
    await render();

    const mode = container.querySelector<HTMLSelectElement>(
      "select[aria-label='DNS 模式']",
    );
    expect(mode).not.toBeNull();
    await act(async () => selectValue("doh", mode!));
    const path = container.querySelector<HTMLInputElement>(
      "input[aria-label='DoH 路径']",
    );
    const domains = container.querySelector<HTMLTextAreaElement>(
      "textarea[aria-label='系统 DNS 域名后缀']",
    );
    expect(path).not.toBeNull();
    expect(domains).not.toBeNull();
    await act(async () => {
      typeInput("/custom-query", path!);
      type("lan\ncorp.example", domains!);
    });

    await act(async () => button("保存 DNS").click());

    expect(setDnsSettingsMock).toHaveBeenCalledWith({
      ...saved.dns,
      fakeIpEnabled: false,
      ipv6Enabled: false,
      strategy: "preferIpv4",
    });
  });

  it("saves DNS settings while connected", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    setDnsSettingsMock.mockResolvedValue(CONNECTED);
    await render();

    const mode = container.querySelector<HTMLSelectElement>(
      "select[aria-label='DNS 模式']",
    );
    expect(mode?.disabled).toBe(false);
    await act(async () => selectValue("doh", mode!));

    await act(async () => button("保存 DNS").click());

    expect(setDnsSettingsMock).toHaveBeenCalledTimes(1);
  });

  it("shows runtime order and saves compact route settings", async () => {
    const rules = [
      {
      enabled: true,
    kind: "domainSuffix" as const,
        outbound: "direct" as const,
        value: "cn",
      },
      {
      enabled: true,
    kind: "geoIp" as const,
        outbound: "direct" as const,
        value: "cn",
      },
    ];
    const saved: SessionStatus = {
      ...IDLE,
      route: { finalOutbound: "direct", providers: [], rules },
    };
    setRouteSettingsMock.mockResolvedValue(saved);
    await render();

    const kind = container.querySelector<HTMLSelectElement>(
      "select[aria-label='规则类型']",
    );
    const value = container.querySelector<HTMLInputElement>(
      "input[aria-label='规则值']",
    );
    const outbound = container.querySelector<HTMLSelectElement>(
      "select[aria-label='规则出口']",
    );
    expect(kind).not.toBeNull();
    expect(value).not.toBeNull();
    expect(outbound).not.toBeNull();
    await act(async () => {
      selectValue("geoIp", kind!);
      selectValue("direct", outbound!);
      typeInput("cn", value!);
    });
    await act(async () => button("添加规则").click());
    await act(async () => {
      selectValue("domainSuffix", kind!);
      typeInput("cn", value!);
    });
    await act(async () => button("添加规则").click());
    const finalOutbound = container.querySelector<HTMLSelectElement>(
      "select[aria-label='默认出口']",
    );
    await act(async () => selectValue("direct", finalOutbound!));

    const rows = container.querySelectorAll("[aria-label='路由规则列表'] tbody tr");
    expect(rows[0]?.textContent).toContain("域名后缀");
    expect(rows[1]?.textContent).toContain("GeoIP");
    await act(async () => button("保存路由").click());

    expect(setRouteSettingsMock).toHaveBeenCalledWith({
      finalOutbound: "direct",
      providers: [],
      rules,
    });
  });

  it("says Xray ignores remote rule sets", async () => {
    loadSessionStatusMock.mockResolvedValue({ ...IDLE, core: "xray" });
    await render();

    expect(container.textContent).toContain("Xray 会忽略远程规则集");
  });

  it("adds a remote rule provider and saves it with the route", async () => {
    setRouteSettingsMock.mockResolvedValue(IDLE);
    await render();

    const name = container.querySelector<HTMLInputElement>(
      "input[aria-label='规则集名称']",
    );
    const url = container.querySelector<HTMLInputElement>(
      "input[aria-label='规则集地址']",
    );
    if (!name || !url) {
      throw new Error("the rule provider form is missing");
    }
    await act(async () => {
      typeInput("ads", name);
      typeInput("https://example.com/ads.srs", url);
    });
    await act(async () => button("添加规则集").click());

    expect(
      container.querySelectorAll("[aria-label='规则集列表'] tbody tr"),
    ).toHaveLength(1);

    await act(async () => button("保存路由").click());

    expect(setRouteSettingsMock).toHaveBeenCalledWith(
      expect.objectContaining({
        providers: [
          {
            enabled: true,
            format: "binary",
            name: "ads",
            outbound: "direct",
            url: "https://example.com/ads.srs",
          },
        ],
      }),
    );
  });

  it("saves route settings while connected", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    setRouteSettingsMock.mockResolvedValue(CONNECTED);
    await render();

    const kind = container.querySelector<HTMLSelectElement>(
      "select[aria-label='规则类型']",
    );
    expect(kind?.disabled).toBe(false);
    const value = container.querySelector<HTMLInputElement>(
      "input[aria-label='规则值']",
    );
    await act(async () => typeInput("cn", value!));
    await act(async () => button("添加规则").click());

    await act(async () => button("保存路由").click());

    expect(setRouteSettingsMock).toHaveBeenCalledTimes(1);
  });

  it("picks up a reconnect that automatic recovery performed", async () => {
    vi.useFakeTimers();
    loadSessionStatusMock
      .mockResolvedValueOnce(SELECTED)
      .mockResolvedValue(CONNECTED);
    try {
      await render();
      expect(container.textContent).toContain("未连接");

      await act(async () => {
        await vi.advanceTimersByTimeAsync(3000);
      });

      expect(container.textContent).toContain("已连接");
    } finally {
      vi.useRealTimers();
    }
  });

  it("does not refresh over an in-flight command", async () => {
    vi.useFakeTimers();
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    let release = (_: SessionStatus) => {};
    connectSessionMock.mockReturnValue(
      new Promise<SessionStatus>((resolve) => {
        release = resolve;
      }),
    );
    try {
      await render();
      await toggleConnect();

      const before = loadSessionStatusMock.mock.calls.length;
      await act(async () => {
        await vi.advanceTimersByTimeAsync(9000);
      });
      expect(loadSessionStatusMock.mock.calls.length).toBe(before);

      await act(async () => release(CONNECTED));
      expect(container.textContent).toContain("已连接");
    } finally {
      vi.useRealTimers();
    }
  });

  it("adds a rule that routes by the protocol the Core sniffs", async () => {
    const rules = [
      {
        enabled: true,
        kind: "protocol" as const,
        outbound: "direct" as const,
        value: "bittorrent",
      },
    ];
    setRouteSettingsMock.mockResolvedValue({
      ...IDLE,
      route: { finalOutbound: "proxy", providers: [], rules },
    });
    await render();

    const kind = container.querySelector<HTMLSelectElement>(
      "select[aria-label='规则类型']",
    );
    const value = container.querySelector<HTMLInputElement>(
      "input[aria-label='规则值']",
    );
    const outbound = container.querySelector<HTMLSelectElement>(
      "select[aria-label='规则出口']",
    );
    await act(async () => {
      selectValue("protocol", kind!);
      selectValue("direct", outbound!);
      typeInput("bittorrent", value!);
    });
    // v2rayN's protocol column: BitTorrent off the proxy without naming a
    // single tracker domain.
    await act(async () => button("添加规则").click());

    const rows = container.querySelectorAll("[aria-label='路由规则列表'] tbody tr");
    expect(rows[0]?.textContent).toContain("协议");
    expect(rows[0]?.textContent).toContain("bittorrent");

    await act(async () => button("保存路由").click());

    expect(setRouteSettingsMock).toHaveBeenCalledWith({
      finalOutbound: "proxy",
      providers: [],
      rules,
    });
  });

  it("adds a rule that routes by the inbound the traffic arrived on", async () => {
    const rules = [
      {
        enabled: true,
        kind: "inbound" as const,
        outbound: "direct" as const,
        value: "socks",
      },
    ];
    setRouteSettingsMock.mockResolvedValue({
      ...IDLE,
      route: { finalOutbound: "proxy", providers: [], rules },
    });
    await render();

    const kind = container.querySelector<HTMLSelectElement>(
      "select[aria-label='规则类型']",
    );
    const value = container.querySelector<HTMLInputElement>(
      "input[aria-label='规则值']",
    );
    const outbound = container.querySelector<HTMLSelectElement>(
      "select[aria-label='规则出口']",
    );
    // The machine's own apps through the local SOCKS go direct while the TUN
    // device's traffic is proxied — v2rayN's inbound-tag column.
    await act(async () => {
      selectValue("inbound", kind!);
      selectValue("direct", outbound!);
      typeInput("socks", value!);
    });
    await act(async () => button("添加规则").click());

    const rows = container.querySelectorAll("[aria-label='路由规则列表'] tbody tr");
    expect(rows[0]?.textContent).toContain("入站");

    await act(async () => button("保存路由").click());

    expect(setRouteSettingsMock).toHaveBeenCalledWith({
      finalOutbound: "proxy",
      providers: [],
      rules,
    });
  });

  it("previews the generated config and saves an edited one as an override", async () => {
    previewCoreConfigMock.mockResolvedValue('{\n  "log": {\n    "level": "warn"\n  }\n}');
    saveAppSettingsMock.mockResolvedValue({
      ...DEFAULT_SETTINGS,
      configOverride: '{"log":{"level":"trace"}}',
    });
    await render();
    await act(async () => button("完整配置模板").click());
    await act(async () => {});

    const editor = container.querySelector<HTMLTextAreaElement>(
      "textarea[aria-label='生成的 Core 配置']",
    );
    // What it would run, not a separately assembled guess.
    expect(editor?.value).toContain('"level": "warn"');

    await act(async () => {
      type('{"log":{"level":"trace"}}', editor!);
    });
    await act(async () => button("保存为覆盖配置").click());

    expect(saveAppSettingsMock).toHaveBeenCalledWith(
      expect.objectContaining({ configOverride: '{"log":{"level":"trace"}}' }),
    );
  });

  it("shows the Core config template that is already saved", async () => {
    loadAppSettingsMock.mockResolvedValue({
      ...DEFAULT_SETTINGS,
      configTemplate: '{"log":{"level":"debug"}}',
    });
    await render();
    await act(async () => button("参数设置").click());

    // Opening the panel has to show what is in force, not an empty box that
    // reads as "no template" and saves as one.
    const template = container.querySelector<HTMLTextAreaElement>(
      "textarea[aria-label='Core 配置模板']",
    );
    expect(template?.value).toBe('{"log":{"level":"debug"}}');
  });

  it("saves a Core config template and reports one the app cannot use", async () => {
    saveAppSettingsMock.mockResolvedValue({
      ...DEFAULT_SETTINGS,
      configTemplate: "{}",
    });
    await render();
    await act(async () => button("参数设置").click());

    const template = container.querySelector<HTMLTextAreaElement>(
      "textarea[aria-label='Core 配置模板']",
    );
    expect(template).not.toBeNull();

    await act(async () => {
      type('{"log":{"level":"debug"}}', template!);
    });
    await act(async () => button("保存配置模板").click());

    expect(saveAppSettingsMock).toHaveBeenCalledWith(
      expect.objectContaining({ configTemplate: '{"log":{"level":"debug"}}' }),
    );

    // The refusal comes from the command, which is where the JSON is parsed;
    // the panel only has to show it.
    saveAppSettingsMock.mockRejectedValueOnce({
      code: "settings_unavailable",
      message: "the Core config template must be a JSON object: expected a JSON object",
    });
    await act(async () => {
      type("[1,2]", template!);
    });
    await act(async () => button("保存配置模板").click());

    expect(container.textContent).toContain("must be a JSON object");
  });

  it("keeps a visible error when a background refresh fails", async () => {
    vi.useFakeTimers();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    loadSessionStatusMock.mockResolvedValueOnce(SELECTED);
    connectSessionMock.mockRejectedValue({
      code: "core_not_configured",
      message: "the pinned sing-box binary is not configured",
    });
    try {
      await render();
      await toggleConnect();
      loadSessionStatusMock.mockRejectedValue(new Error("refresh failed"));

      await act(async () => {
        await vi.advanceTimersByTimeAsync(3000);
      });

      expect(container.querySelector("[role='alert']")?.textContent).toContain(
        "the pinned sing-box binary is not configured",
      );
      expect(warn).toHaveBeenCalled();
    } finally {
      warn.mockRestore();
      vi.useRealTimers();
    }
  });

  it("exports a diagnostic bundle and shows where it went", async () => {
    exportDiagnosticsMock.mockResolvedValue("/data/mgclash-diagnostics-1.json");
    await render();

    await act(async () => button("导出诊断").click());

    expect(exportDiagnosticsMock).toHaveBeenCalledOnce();
    expect(container.querySelector("[role='status']")?.textContent).toContain(
      "/data/mgclash-diagnostics-1.json",
    );
  });

  it("exports preferences to a JSON file path", async () => {
    exportPreferencesMock.mockResolvedValue("/data/mgclash-preferences-1.json");
    await render();

    await act(async () => button("导出设置").click());

    expect(exportPreferencesMock).toHaveBeenCalledOnce();
    expect(container.querySelector("[role='status']")?.textContent).toContain(
      "/data/mgclash-preferences-1.json",
    );
  });

  it("exports every node share link from the menu", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const osaka = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
    };
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    exportNodeLinkMock.mockReset();
    exportNodeLinkMock.mockImplementation(async (id: string) => {
      if (id === SELECTED.node!.id) {
        return "vless://one";
      }
      if (id === osaka.id) {
        return "vless://two";
      }
      throw new Error(`unexpected node ${id}`);
    });
    await render();
    exportNodeLinkMock.mockClear();
    writeText.mockClear();

    await act(async () =>
      container.querySelector<HTMLInputElement>("[aria-label='全选节点']")?.click(),
    );
    await nodeMenuAction("Tokyo Edge", "导出分享链接 (2)");

    expect(exportNodeLinkMock).toHaveBeenCalledWith(SELECTED.node!.id);
    expect(exportNodeLinkMock).toHaveBeenCalledWith(osaka.id);
    expect(writeText).toHaveBeenCalledWith("vless://one\nvless://two");
  });

  it("steps to the next enabled node", async () => {
    const osaka = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
    };
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    switchNodeMock.mockResolvedValue({ ...SELECTED, node: osaka });
    await render();

    await act(async () => {
      document.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "]",
          ctrlKey: true,
          bubbles: true,
        }),
      );
    });

    expect(switchNodeMock).toHaveBeenCalledWith(osaka.id);
  });

  it("shows a failed export as an error", async () => {
    exportDiagnosticsMock.mockRejectedValue({
      code: "diagnostics_write_failed",
      message: "failed to write the diagnostic bundle",
    });
    await render();

    await act(async () => button("导出诊断").click());

    expect(container.querySelector("[role='alert']")?.textContent).toContain(
      "failed to write the diagnostic bundle",
    );
    expect(container.querySelector("[role='status']")).toBeNull();
  });

  it("shows the typed message when a command fails", async () => {
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    connectSessionMock.mockRejectedValue({
      code: "core_not_configured",
      message: "the pinned sing-box binary is not configured",
    });
    await render();

    await toggleConnect();

    expect(container.querySelector("[role='alert']")?.textContent).toContain(
      "the pinned sing-box binary is not configured",
    );
    expect(container.textContent).toContain("未连接");
  });

  it("shows an untyped rejection as plain text", async () => {
    loadSessionStatusMock.mockRejectedValue(new Error("command failed"));

    await render();

    expect(container.querySelector("[role='alert']")?.textContent).toContain(
      "command failed",
    );
  });

  it("shows platform command failures", async () => {
    loadPlatformSummaryMock.mockRejectedValue(new Error("platform failed"));

    await render();

    expect(container.textContent).toContain("platform failed");
  });

  it("lays the window out with a menu bar and node workspace", async () => {
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    await render();

    const nav = container.querySelector("[aria-label='主菜单']");
    expect(nav).not.toBeNull();
    expect(nav?.textContent).toContain("配置项");
    expect(nav?.textContent).toContain("订阅分组");
    expect(nav?.textContent).toContain("设置");
    expect(nav?.textContent).toContain("帮助");
    expect(nav?.textContent).toContain("重启服务");
    expect(nav?.textContent).toContain("退出");
    expect(container.querySelector("[aria-label='节点列表']")).not.toBeNull();
    expect(container.querySelector(".group-chips")).not.toBeNull();
    expect(container.querySelector("[aria-label='节点详情']")).toBeNull();
  });

  it("imports a share link from the clipboard via the server menu", async () => {
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        readText: vi.fn().mockResolvedValue(
          "ss://aes-128-gcm:secret@edge.example.com:8388#Tokyo Edge",
        ),
      },
    });
    importNodesMock.mockResolvedValue({
      duplicates: 0,
      failures: [],
      imported: 1,
      status: SELECTED,
    });
    loadNodesMock
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([SELECTED.node]);
    await render();

    await act(async () => button("从剪贴板导入分享链接").click());

    expect(importNodesMock).toHaveBeenCalledWith(
      "ss://aes-128-gcm:secret@edge.example.com:8388#Tokyo Edge",
    );
  });

  it("changes the font size from the theme flyout", async () => {
    await render();
    const size = container.querySelector<HTMLSelectElement>(
      "[aria-label='字体大小']",
    );
    if (!size) {
      throw new Error("font size control is missing");
    }
    await act(async () => selectValue("16", size));
    expect(document.documentElement.style.fontSize).toBe("16px");
    expect(localStorage.getItem("mgclash.fontSize")).toBe("16");
  });

  it("filters the node table by name, server, or protocol", async () => {
    loadNodesMock.mockResolvedValue([
      SELECTED.node,
      {
        ...SELECTED.node!,
        id: "00000000-0000-0000-0000-000000000002",
        name: "Osaka",
        protocol: "trojan",
        server: "osaka.example.com",
      },
    ]);
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    await render();

    const search = container.querySelector<HTMLInputElement>(
      "[aria-label='搜索节点名称 / 服务器 / 协议']",
    );
    if (!search) {
      throw new Error("node search field is missing");
    }
    await act(async () => typeInput("trojan", search));

    const rows = [...container.querySelectorAll("[aria-label='节点列表'] tbody tr")];
    expect(rows).toHaveLength(1);
    expect(rows[0]?.textContent).toContain("Osaka");
    expect(rows[0]?.textContent).not.toContain("Tokyo Edge");
  });

  it("marks latency quality on a tested node", async () => {
    loadNodesMock.mockResolvedValue([
      { ...SELECTED.node!, latencyMs: 42, lastTestedAt: 1 },
    ]);
    loadSessionStatusMock.mockResolvedValue({
      ...SELECTED,
      node: { ...SELECTED.node!, latencyMs: 42, lastTestedAt: 1 },
    });
    await render();

    const row = container.querySelector("[aria-label='节点列表'] tbody tr");
    expect(row?.textContent).toContain("42 ms");
    expect(row?.textContent).toContain("优秀");
  });

  it("opens dialogs from the menu bar without losing hidden forms", async () => {
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    await render();

    // The status bar and node table are part of the workspace, always present.
    expect(container.querySelector("[aria-label='下载速率']")).not.toBeNull();
    expect(container.querySelector("[aria-label='节点列表']")).not.toBeNull();

    await act(async () => button("订阅分组设置").click());
    expect(container.querySelector("[aria-label='订阅名称']")).not.toBeNull();

    expect(container.querySelector("[aria-label='分享链接']")).not.toBeNull();
    expect(container.querySelector("[aria-label='节点列表']")).not.toBeNull();
  });

  it("opens the settings, routing, and DNS dialogs from the menu", async () => {
    await render();

    for (const [label, dialogLabel] of [
      ["参数设置", "设置"],
      ["路由设置", "路由规则"],
    ] as const) {
      const dialog = container.querySelector(`[aria-label='${dialogLabel}']`);
      expect(dialog?.hasAttribute("hidden")).toBe(true);
      await act(async () => button(label).click());
      expect(dialog?.hasAttribute("hidden")).toBe(false);
    }

    const dns = container.querySelector("[aria-label='DNS']");
    expect(dns?.hasAttribute("hidden")).toBe(true);
    await act(async () => button("DNS").click());
    expect(dns?.hasAttribute("hidden")).toBe(false);
  });

  it("presets the protocol when adding a server from the menu", async () => {
    await render();

    await act(async () => button("添加 [Trojan]").click());

    const dialog = container.querySelector("[aria-label='手动创建节点']");
    expect(dialog?.hasAttribute("hidden")).toBe(false);
    expect(
      container.querySelector<HTMLSelectElement>("[aria-label='节点协议']")
        ?.value,
    ).toBe("trojan");
  });

  it("reloads nodes, subscriptions, and status from the menu", async () => {
    await render();

    expect(loadNodesMock).toHaveBeenCalledTimes(1);
    expect(loadSubscriptionsMock).toHaveBeenCalledTimes(1);
    expect(loadSessionStatusMock).toHaveBeenCalledTimes(1);

    await act(async () => button("重启服务").click());

    expect(loadNodesMock).toHaveBeenCalledTimes(2);
    expect(loadSubscriptionsMock).toHaveBeenCalledTimes(2);
    expect(loadSessionStatusMock).toHaveBeenCalledTimes(2);
  });

  it("shows the about dialog with the close-to-tray state", async () => {
    await render();

    await act(async () => button("关于").click());

    const dialog = container.querySelector("[aria-label='关于']");
    expect(dialog?.hasAttribute("hidden")).toBe(false);
    expect(dialog?.textContent).toContain("MgClash");
    expect(dialog?.textContent).toContain("已启用");
  });

  it("switches between light and dark theme from the menu", async () => {
    await render();

    expect(document.documentElement.dataset.theme).toBe("light");

    const theme = container.querySelector<HTMLSelectElement>("select[aria-label='主题']");
    if (!theme) {
      throw new Error("theme picker is missing");
    }
    await act(async () => selectValue("dark", theme));

    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(localStorage.getItem("mgclash.theme")).toBe("dark");

    await act(async () => selectValue("light", theme));

    expect(document.documentElement.dataset.theme).toBe("light");
  });

  it("groups nodes by policy group in the proxies tab", async () => {
    const work = {
      id: "00000000-0000-0000-0000-000000000020",
      name: "Work",
      strategy: "urlTest" as const,
    };
    const osaka = {
      ...SELECTED.node!,
      groupId: work.id,
      id: "00000000-0000-0000-0000-000000000002",
      latencyMs: 88,
      name: "Osaka",
    };
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodeGroupsMock.mockResolvedValue([work]);
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    setNodeGroupStrategyMock.mockResolvedValue([{ ...work, strategy: "select" }]);
    connectSessionMock.mockResolvedValue({ ...SELECTED, node: osaka, connected: true });
    await render();

    await act(async () => button("当前代理").click());

    const rows = [
      ...container.querySelectorAll("[aria-label='代理组节点'] tbody tr"),
    ];
    expect(rows.map((row) => row.textContent)).toEqual([
      expect.stringContaining("Osaka"),
    ]);
    expect(rows[0]?.textContent).toContain("88 ms");

    const strategy = container.querySelector<HTMLSelectElement>(
      "[aria-label='代理组策略']",
    );
    await act(async () => selectValue("select", strategy!));
    expect(setNodeGroupStrategyMock).toHaveBeenCalledWith(work.id, "select");

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>("[aria-label='设为活动 Osaka']")
        ?.click(),
    );
    expect(connectSessionMock).toHaveBeenCalled();
  });

  it("tests every node of the selected policy group", async () => {
    const work = {
      id: "00000000-0000-0000-0000-000000000020",
      name: "Work",
      strategy: "select" as const,
    };
    const osaka = {
      ...SELECTED.node!,
      groupId: work.id,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
    };
    loadNodeGroupsMock.mockResolvedValue([work]);
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    await render();

    await act(async () => button("当前代理").click());
    await act(async () => button("测试本组延迟").click());

    expect(testAllNodesMock).toHaveBeenCalledWith(
      [osaka.id],
      expect.any(Function),
      expect.any(Function),
    );
  });

  it("lists live connections and closes them from the connections tab", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    loadConnectionsMock.mockResolvedValue({
      uploadTotalBytes: 2_048,
      downloadTotalBytes: 4_096,
      connections: [
        {
          id: "0f9c1f7e-0000-4000-8000-000000000001",
          host: "example.com",
          destination: "93.184.216.34:443",
          network: "tcp",
          process: "Safari",
          rule: "rule_set(geosite-geolocation-!cn)",
          chain: "tokyo → proxy",
          uploadBytes: 120,
          downloadBytes: 340,
          start: "2026-08-13T00:00:00Z",
        },
      ],
    });
    await render();

    await act(async () => button("当前连接").click());

    const rows = [
      ...container.querySelectorAll("[aria-label='连接列表'] tbody tr"),
    ];
    expect(rows).toHaveLength(1);
    expect(rows[0]?.textContent).toContain("example.com");
    expect(rows[0]?.textContent).toContain("Safari");
    expect(rows[0]?.textContent).toContain("tokyo → proxy");

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>(
          "[aria-label='关闭连接 example.com']",
        )
        ?.click(),
    );
    expect(closeConnectionMock).toHaveBeenCalledWith(
      "0f9c1f7e-0000-4000-8000-000000000001",
    );

    await act(async () => button("全部关闭").click());
    expect(closeConnectionsMock).toHaveBeenCalledTimes(1);
  });

  it("says the running Core has no connection API instead of an empty table", async () => {
    loadSessionStatusMock.mockResolvedValue({ ...CONNECTED, core: "xray" });
    await render();

    await act(async () => button("当前连接").click());

    expect(container.querySelector("[aria-label='连接列表']")).toBeNull();
    expect(container.textContent).toContain("Xray 不提供连接列表");
    expect(loadConnectionsMock).not.toHaveBeenCalled();
  });

  it("explains an empty connection list rather than showing a blank table", async () => {
    loadSessionStatusMock.mockResolvedValue(IDLE);
    await render();

    await act(async () => button("当前连接").click());

    expect(container.querySelector("[aria-label='连接列表']")).toBeNull();
    expect(container.textContent).toContain("连接后才会有连接记录");
  });


  it("shows the release check result and closes it", async () => {
    checkUpdateMock.mockResolvedValue({
      current: "0.1.0",
      latest: "0.2.0",
      updateAvailable: true,
      url: "https://example.com/releases",
    });
    await render();

    await act(async () => button("检查更新").click());

    const dialog = container.querySelector("[aria-label='检查更新结果']");
    expect(dialog?.textContent).toContain("0.2.0");
    expect(dialog?.textContent).toContain("https://example.com/releases");

    await act(async () =>
      dialog!.querySelector<HTMLButtonElement>(".dialog-head button")?.click(),
    );
    expect(container.querySelector("[aria-label='检查更新结果']")).toBeNull();
  });

  it("downloads a Core from the update dialog", async () => {
    const check = {
      singBox: {
        name: "sing-box",
        current: "1.13.18",
        latest: "1.14.0",
        url: "https://example.com/sing-box",
        updateAvailable: true,
        fromBinary: true,
      },
      xray: {
        name: "xray",
        current: "26.3.27",
        latest: "26.3.27",
        url: "https://example.com/xray",
        updateAvailable: false,
        fromBinary: false,
      },
      install: { directory: "/tmp/cores" },
    };
    checkCoreUpdateMock.mockResolvedValue(check);
    downloadCoreUpdateMock.mockResolvedValue({ directory: "/tmp/cores" });
    await render();

    await act(async () => button("检查 Core 更新").click());
    const dialog = container.querySelector("[aria-label='检查 Core 更新结果']");
    expect(dialog?.textContent).toContain("1.14.0");
    // Only the Core with an update offers the button.
    expect(button("下载 Xray").disabled).toBe(true);

    await act(async () => button("下载 sing-box").click());

    expect(downloadCoreUpdateMock).toHaveBeenCalledWith("sing-box");
    // The dialog re-reads the versions so the installed line is not stale.
    expect(checkCoreUpdateMock.mock.calls.length).toBeGreaterThanOrEqual(2);
  });

  it("reads and updates the Geo files", async () => {
    const status = {
      directory: "/tmp/geo",
      geoip: { name: "geoip.dat", present: false, bytes: 0, modifiedAt: null },
      geosite: {
        name: "geosite.dat",
        present: true,
        bytes: 2_048,
        modifiedAt: 1_760_000_000_000,
      },
      assetEnvApplied: false,
    };
    loadGeoAssetsStatusMock.mockResolvedValue(status);
    updateGeoAssetsMock.mockResolvedValue({ ...status, assetEnvApplied: true });
    await render();

    await act(async () => button("更新 Geo 文件").click());
    expect(container.textContent).toContain("/tmp/geo");

    await act(async () => button("立即更新").click());

    expect(updateGeoAssetsMock).toHaveBeenCalledTimes(1);
    expect(container.textContent).toContain("Geo 文件已更新");
  });

  it("saves the free-text settings fields", async () => {
    await render();

    const fields: [string, string, string][] = [
      ["设置中的 URL 测试地址", "https://probe.example.com", "urlTestAddress"],
      ["默认 TLS 指纹", "firefox", "defFingerprint"],
      ["热键：连接/断开", "Ctrl+Enter", "hotkeyConnect"],
      ["热键：上一节点", "Ctrl+[", "hotkeyPrevious"],
      ["热键：下一节点", "Ctrl+]", "hotkeyNext"],
    ];
    for (const [label, value, key] of fields) {
      const field = container.querySelector<HTMLInputElement>(
        `input[aria-label='${label}']`,
      );
      if (!field) {
        throw new Error(`no ${label} field`);
      }
      await act(async () => typeInput(value, field));
      // Each field commits on blur rather than on every keystroke.
      await act(async () =>
        field.dispatchEvent(new FocusEvent("focusout", { bubbles: true })),
      );
      expect(saveAppSettingsMock).toHaveBeenCalledWith(
        expect.objectContaining({ [key]: value }),
      );
    }
  });


  it("switches, creates and deletes routing schemes", async () => {
    const schemes = [
      { id: "default", name: "默认" },
      { id: "work", name: "Work" },
    ];
    const withSchemes = {
      ...IDLE,
      routeSchemeId: "default",
      routeSchemes: schemes,
    };
    loadSessionStatusMock.mockResolvedValue(withSchemes);
    setRouteSchemeMock.mockResolvedValue({
      ...withSchemes,
      routeSchemeId: "work",
    });
    createRouteSchemeMock.mockResolvedValue(withSchemes);
    deleteRouteSchemeMock.mockResolvedValue(withSchemes);
    const prompt = vi
      .spyOn(window, "prompt")
      .mockReturnValue("  Travel  ");
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    try {
      await render();

      const picker = container.querySelector<HTMLSelectElement>(
        "select[aria-label='路由方案']",
      );
      if (!picker) {
        throw new Error("no scheme picker");
      }
      await act(async () => selectValue("work", picker));
      expect(setRouteSchemeMock).toHaveBeenCalledWith("work");

      await act(async () => button("新建方案").click());
      // The typed name is trimmed before it reaches the command.
      expect(createRouteSchemeMock).toHaveBeenCalledWith("Travel");

      await act(async () => button("删除方案").click());
      expect(deleteRouteSchemeMock).toHaveBeenCalledWith("default");
    } finally {
      prompt.mockRestore();
      confirm.mockRestore();
    }
  });

  it("keeps a routing scheme the user declined to create or delete", async () => {
    loadSessionStatusMock.mockResolvedValue({
      ...IDLE,
      routeSchemeId: "default",
      routeSchemes: [
        { id: "default", name: "默认" },
        { id: "work", name: "Work" },
      ],
    });
    const prompt = vi.spyOn(window, "prompt").mockReturnValue(null);
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    try {
      await render();

      await act(async () => button("新建方案").click());
      await act(async () => button("删除方案").click());

      expect(createRouteSchemeMock).not.toHaveBeenCalled();
      expect(deleteRouteSchemeMock).not.toHaveBeenCalled();
    } finally {
      prompt.mockRestore();
      confirm.mockRestore();
    }
  });


  it("filters the connection list and switches proxy groups", async () => {
    const work = {
      id: "00000000-0000-0000-0000-000000000020",
      name: "Work",
      strategy: "select" as const,
    };
    const home = {
      id: "00000000-0000-0000-0000-000000000021",
      name: "Home",
      strategy: "select" as const,
    };
    const osaka = {
      ...SELECTED.node!,
      groupId: home.id,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
    };
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    loadNodeGroupsMock.mockResolvedValue([work, home]);
    loadNodesMock.mockResolvedValue([CONNECTED.node, osaka]);
    loadConnectionsMock.mockResolvedValue({
      uploadTotalBytes: 0,
      downloadTotalBytes: 0,
      connections: [
        {
          id: "c1",
          host: "example.com",
          destination: "93.184.216.34:443",
          network: "tcp",
          process: "Safari",
          rule: "final",
          chain: "proxy",
          uploadBytes: 0,
          downloadBytes: 0,
          start: "2026-08-13T00:00:00Z",
        },
        {
          id: "c2",
          host: "cdn.example.net",
          destination: "1.1.1.1:443",
          network: "tcp",
          process: "curl",
          rule: "final",
          chain: "direct",
          uploadBytes: 0,
          downloadBytes: 0,
          start: "2026-08-13T00:00:00Z",
        },
      ],
    });
    await render();

    // The second group only shows once it is picked from the rail.
    await act(async () => button("当前代理").click());
    expect(
      container.querySelectorAll("[aria-label='代理组节点'] tbody tr"),
    ).toHaveLength(0);
    await act(async () =>
      [...container.querySelectorAll<HTMLButtonElement>(
        ".proxies-pane .group-rail button",
      )]
        .find((entry) => entry.textContent?.includes("Home"))
        ?.click(),
    );
    expect(
      [...container.querySelectorAll("[aria-label='代理组节点'] tbody tr")].map(
        (row) => row.textContent,
      ),
    ).toEqual([expect.stringContaining("Osaka")]);

    await act(async () => button("当前连接").click());
    const search = container.querySelector<HTMLInputElement>(
      "input[aria-label='搜索连接']",
    );
    if (!search) {
      throw new Error("no connection search box");
    }
    await act(async () => typeInput("curl", search));

    expect(
      [...container.querySelectorAll("[aria-label='连接列表'] tbody tr")].map(
        (row) => row.textContent,
      ),
    ).toEqual([expect.stringContaining("cdn.example.net")]);
  });


  it("offers a blocked outbound for rules but not as the default", async () => {
    setRouteSettingsMock.mockResolvedValue(IDLE);
    await render();

    const ruleOutbound = container.querySelector<HTMLSelectElement>(
      "select[aria-label='规则出口']",
    );
    const setOutbound = container.querySelector<HTMLSelectElement>(
      "select[aria-label='规则集出口']",
    );
    const defaultOutbound = container.querySelector<HTMLSelectElement>(
      "select[aria-label='默认出口']",
    );
    const values = (select: HTMLSelectElement | null) =>
      [...(select?.options ?? [])].map((option) => option.value);
    expect(values(ruleOutbound)).toEqual(["proxy", "direct", "block"]);
    expect(values(setOutbound)).toEqual(["proxy", "direct", "block"]);
    // Blocking everything by default is not a routing mode the window offers.
    expect(values(defaultOutbound)).toEqual(["proxy", "direct"]);

    await act(async () => selectValue("block", ruleOutbound!));
    const value = container.querySelector<HTMLInputElement>(
      "input[aria-label='规则值']",
    );
    await act(async () => typeInput("ads.example", value!));
    await act(async () => button("添加规则").click());

    const rows = [
      ...container.querySelectorAll("[aria-label='路由规则列表'] tbody tr"),
    ];
    expect(rows[0]?.textContent).toContain("拦截");

    await act(async () => button("保存路由").click());
    expect(setRouteSettingsMock).toHaveBeenCalledWith(
      expect.objectContaining({
        rules: [
          {
            enabled: true,
            kind: "domainSuffix",
            outbound: "block",
            value: "ads.example",
          },
        ],
      }),
    );
  });


  it("drafts a routing rule from a connection", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    loadNodesMock.mockResolvedValue([CONNECTED.node]);
    loadConnectionsMock.mockResolvedValue({
      uploadTotalBytes: 0,
      downloadTotalBytes: 0,
      connections: [
        {
          id: "c1",
          host: "cdn.example.com",
          destination: "93.184.216.34:443",
          network: "tcp",
          process: "Safari",
          rule: "final",
          chain: "proxy",
          uploadBytes: 0,
          downloadBytes: 0,
          start: "2026-08-13T00:00:00Z",
        },
      ],
    });
    await render();

    await act(async () => button("当前连接").click());
    await act(async () =>
      container
        .querySelector<HTMLButtonElement>("[aria-label='为 cdn.example.com 添加规则']")
        ?.click(),
    );

    // The routing dialog opens with the connection's host already filled in.
    expect(
      container
        .querySelector("[aria-label='路由规则']")
        ?.hasAttribute("hidden"),
    ).toBe(false);
    expect(
      container.querySelector<HTMLSelectElement>("select[aria-label='规则类型']")
        ?.value,
    ).toBe("domainSuffix");
    expect(
      container.querySelector<HTMLInputElement>("input[aria-label='规则值']")
        ?.value,
    ).toBe("cdn.example.com");
  });

  it("saves the policy group probe interval and tolerance", async () => {
    await render();

    const interval = container.querySelector<HTMLInputElement>(
      "input[aria-label='自动测速间隔（秒）']",
    );
    const tolerance = container.querySelector<HTMLInputElement>(
      "input[aria-label='切换容差（毫秒）']",
    );
    if (!interval || !tolerance) {
      throw new Error("the probe fields are missing");
    }
    await act(async () => typeInput("600", interval));
    await act(async () =>
      interval.dispatchEvent(new FocusEvent("focusout", { bubbles: true })),
    );
    expect(saveAppSettingsMock).toHaveBeenCalledWith(
      expect.objectContaining({ urlTestIntervalSeconds: 600 }),
    );

    await act(async () => typeInput("150", tolerance));
    await act(async () =>
      tolerance.dispatchEvent(new FocusEvent("focusout", { bubbles: true })),
    );
    expect(saveAppSettingsMock).toHaveBeenCalledWith(
      expect.objectContaining({ urlTestToleranceMs: 150 }),
    );
  });

  it("refreshes subscriptions while connected", async () => {
    const subscription = {
      id: "00000000-0000-0000-0000-0000000000a1",
      name: "Vendor",
      url: "https://example.com/sub",
      enabled: true,
      autoUpdate: true,
      updateIntervalMinutes: 1_440,
      lastUpdatedAt: null,
      nodeCount: 3,
      userAgent: null,
    };
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    loadNodesMock.mockResolvedValue([CONNECTED.node]);
    loadSubscriptionsMock.mockResolvedValue([subscription]);
    refreshSubscriptionMock.mockResolvedValue(subscription);
    refreshAllSubscriptionsMock.mockResolvedValue([subscription]);
    await render();

    // A vendor list changing is unrelated to the server the Core is running.
    await act(async () => button("更新全部订阅 (不通过代理)").click());
    expect(refreshAllSubscriptionsMock).toHaveBeenCalledTimes(1);

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>("[aria-label='刷新 Vendor']")
        ?.click(),
    );
    expect(refreshSubscriptionMock).toHaveBeenCalledWith(subscription.id);
  });

  it("downloads a rule set and shows what is cached", async () => {
    const provider = {
      enabled: true,
      format: "binary" as const,
      name: "ads",
      outbound: "block" as const,
      url: "https://example.com/ads.srs",
    };
    loadSessionStatusMock.mockResolvedValue({
      ...IDLE,
      route: { finalOutbound: "proxy", providers: [provider], rules: [] },
    });
    loadRuleSetsMock.mockResolvedValue([
      { bytes: 0, cached: false, name: "ads", path: "/cache/ads.srs", updatedAt: null },
    ]);
    updateRuleSetMock.mockResolvedValue([
      {
        bytes: 2_048,
        cached: true,
        name: "ads",
        path: "/cache/ads.srs",
        updatedAt: 1_760_000_000,
      },
    ]);
    await render();

    const row = container.querySelector("[aria-label='规则集列表'] tbody tr");
    expect(row?.textContent).toContain("未缓存");

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>("[aria-label='更新规则集 ads']")
        ?.click(),
    );

    expect(updateRuleSetMock).toHaveBeenCalledWith("ads");
    expect(
      container.querySelector("[aria-label='规则集列表'] tbody tr")?.textContent,
    ).toContain("2.0 KB");
  });


  it("updates every rule set at once and reports a failure", async () => {
    const provider = {
      enabled: true,
      format: "binary" as const,
      name: "ads",
      outbound: "block" as const,
      url: "https://example.com/ads.srs",
    };
    loadSessionStatusMock.mockResolvedValue({
      ...IDLE,
      route: { finalOutbound: "proxy", providers: [provider], rules: [] },
    });
    loadRuleSetsMock.mockResolvedValue([
      { bytes: 0, cached: false, name: "ads", path: "/cache/ads.srs", updatedAt: null },
    ]);
    updateRuleSetsMock.mockResolvedValue([
      { bytes: 4_096, cached: true, name: "ads", path: "/cache/ads.srs", updatedAt: null },
    ]);
    await render();

    await act(async () => button("全部更新规则集").click());

    expect(updateRuleSetsMock).toHaveBeenCalledTimes(1);
    expect(
      container.querySelector("[aria-label='规则集列表'] tbody tr")?.textContent,
    ).toContain("4.0 KB");

    // A vendor that is down surfaces rather than looking like success.
    updateRuleSetsMock.mockRejectedValue({
      code: "rule_set_download_failed",
      message: "failed to download https://example.com/ads.srs",
    });
    await act(async () => button("全部更新规则集").click());
    expect(container.querySelector("[role='alert']")?.textContent).toContain(
      "failed to download",
    );
  });

  it("surfaces a rule set download that fails on its own", async () => {
    const provider = {
      enabled: true,
      format: "binary" as const,
      name: "ads",
      outbound: "block" as const,
      url: "https://example.com/ads.srs",
    };
    loadSessionStatusMock.mockResolvedValue({
      ...IDLE,
      route: { finalOutbound: "proxy", providers: [provider], rules: [] },
    });
    loadRuleSetsMock.mockResolvedValue([
      { bytes: 0, cached: false, name: "ads", path: "/cache/ads.srs", updatedAt: null },
    ]);
    updateRuleSetMock.mockRejectedValue({
      code: "rule_set_download_failed",
      message: "failed to download https://example.com/ads.srs",
    });
    await render();

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>("[aria-label='更新规则集 ads']")
        ?.click(),
    );

    expect(container.querySelector("[role='alert']")?.textContent).toContain(
      "failed to download",
    );
  });


  it("adds up traffic per policy group and per live program", async () => {
    const work = {
      id: "00000000-0000-0000-0000-000000000020",
      name: "Work",
      strategy: "select" as const,
    };
    const osaka = {
      ...SELECTED.node!,
      groupId: work.id,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
    };
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    loadNodeGroupsMock.mockResolvedValue([work]);
    loadNodesMock.mockResolvedValue([CONNECTED.node, osaka]);
    loadNodeTrafficMock.mockResolvedValue({
      [osaka.id]: {
        todayUploadBytes: 1_024,
        todayDownloadBytes: 2_048,
        totalUploadBytes: 4_096,
        totalDownloadBytes: 8_192,
      },
    });
    loadConnectionsMock.mockResolvedValue({
      uploadTotalBytes: 0,
      downloadTotalBytes: 0,
      connections: [
        {
          id: "c1",
          host: "a.example.com",
          destination: "1.1.1.1:443",
          network: "tcp",
          process: "Safari",
          rule: "final",
          chain: "proxy",
          uploadBytes: 1_024,
          downloadBytes: 2_048,
          start: "2026-08-13T00:00:00Z",
        },
        {
          id: "c2",
          host: "b.example.com",
          destination: "1.1.1.1:443",
          network: "tcp",
          process: "Safari",
          rule: "final",
          chain: "proxy",
          uploadBytes: 1_024,
          downloadBytes: 2_048,
          start: "2026-08-13T00:00:00Z",
        },
      ],
    });
    await render();

    await act(async () => button("当前代理").click());
    const groupTotals = container.querySelector("[aria-label='代理组流量']");
    expect(groupTotals?.textContent).toContain("2.0 KB");
    expect(groupTotals?.textContent).toContain("8.0 KB");

    await act(async () => button("当前连接").click());
    const rows = [
      ...container.querySelectorAll("[aria-label='进程流量'] tbody tr"),
    ];
    expect(rows[0]?.textContent).toContain("Safari");
    // Two connections of the same program add up.
    expect(rows[0]?.textContent).toContain("4.0 KB");
  });

  it("chains a node through a front proxy from the context menu", async () => {
    const osaka = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
    };
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    setNodeFrontMock.mockResolvedValue([
      { ...SELECTED.node!, frontNodeId: osaka.id },
      osaka,
    ]);
    await render();

    await nodeMenuAction("Tokyo Edge", "前置代理");
    const picker = container.querySelector<HTMLSelectElement>(
      "[aria-label='前置代理节点']",
    );
    if (!picker) {
      throw new Error("the front proxy picker is missing");
    }
    // A node cannot front itself, so it is not on offer.
    expect([...picker.options].map((option) => option.value)).toEqual([
      "",
      osaka.id,
    ]);

    await act(async () => selectValue(osaka.id, picker));
    await act(async () => button("保存前置代理").click());

    expect(setNodeFrontMock).toHaveBeenCalledWith(SELECTED.node!.id, osaka.id);
  });

  it("offers to fetch a Core when none is installed", async () => {
    loadCoreReadinessMock.mockResolvedValue({ singBox: false, xray: false });
    downloadCoreUpdateMock.mockResolvedValue({ directory: "/tmp/cores" });
    checkCoreUpdateMock.mockResolvedValue({
      singBox: {
        name: "sing-box",
        current: "1.13.18",
        latest: "1.13.18",
        url: "https://example.com/sing-box",
        updateAvailable: false,
        fromBinary: true,
      },
      xray: {
        name: "xray",
        current: "26.3.27",
        latest: "26.3.27",
        url: "https://example.com/xray",
        updateAvailable: false,
        fromBinary: false,
      },
      install: { directory: "/tmp/cores" },
    });
    await render();

    const notice = container.querySelector("[aria-label='缺少 Core']");
    expect(notice?.textContent).toContain("还没有可用的 Core");

    // The install makes the Core usable, and the prompt stops nagging.
    loadCoreReadinessMock.mockResolvedValue({ singBox: true, xray: false });
    await act(async () => button("下载 sing-box").click());

    expect(downloadCoreUpdateMock).toHaveBeenCalledWith("sing-box");
    expect(container.querySelector("[aria-label='缺少 Core']")).toBeNull();
  });

  it("says nothing when a Core is already usable", async () => {
    loadCoreReadinessMock.mockResolvedValue({ singBox: true, xray: false });
    await render();

    expect(container.querySelector("[aria-label='缺少 Core']")).toBeNull();
  });


  it("scans a sharing link off the camera", async () => {
    const track = { stop: vi.fn() };
    const stream = { getTracks: () => [track] } as unknown as MediaStream;
    const getUserMedia = vi.fn().mockResolvedValue(stream);
    Object.defineProperty(navigator, "mediaDevices", {
      configurable: true,
      value: { getUserMedia },
    });
    await render();

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>("[aria-label='用摄像头扫描二维码']")
        ?.click(),
    );

    expect(getUserMedia).toHaveBeenCalledWith({
      video: { facingMode: "environment" },
    });
    expect(container.querySelector("[aria-label='摄像头预览']")).not.toBeNull();

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>("[aria-label='停止扫描']")
        ?.click(),
    );

    // Stopping releases the camera rather than leaving the light on.
    expect(track.stop).toHaveBeenCalledTimes(1);
    expect(container.querySelector("[aria-label='摄像头预览']")).toBeNull();
  });

  it("reports a camera the system will not open", async () => {
    Object.defineProperty(navigator, "mediaDevices", {
      configurable: true,
      value: {
        getUserMedia: vi.fn().mockRejectedValue(new Error("Permission denied")),
      },
    });
    await render();

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>("[aria-label='用摄像头扫描二维码']")
        ?.click(),
    );

    expect(container.querySelector("[role='alert']")?.textContent).toContain(
      "Permission denied",
    );
    expect(container.querySelector("[aria-label='摄像头预览']")).toBeNull();
  });


  it("imports the link a scanned frame decodes to", async () => {
    const track = { stop: vi.fn() };
    Object.defineProperty(navigator, "mediaDevices", {
      configurable: true,
      value: {
        getUserMedia: vi
          .fn()
          .mockResolvedValue({ getTracks: () => [track] } as unknown as MediaStream),
      },
    });
    // A frame only exists once the camera reports a picture size.
    Object.defineProperty(HTMLVideoElement.prototype, "videoWidth", {
      configurable: true,
      get: () => 640,
    });
    Object.defineProperty(HTMLVideoElement.prototype, "videoHeight", {
      configurable: true,
      get: () => 480,
    });
    const context = { drawImage: vi.fn() };
    const getContext = vi
      .spyOn(HTMLCanvasElement.prototype, "getContext")
      .mockReturnValue(context as unknown as CanvasRenderingContext2D);
    const toDataURL = vi
      .spyOn(HTMLCanvasElement.prototype, "toDataURL")
      .mockReturnValue("data:image/png;base64,AAECAw==");
    readQrCodeMock.mockResolvedValue("ss://aes-128-gcm:secret@edge.example.com:8388#Scanned");
    importNodesMock.mockResolvedValue({
      duplicates: 0,
      failures: [],
      imported: 1,
      status: SELECTED,
    });
    vi.useFakeTimers();
    try {
      await render();
      await act(async () =>
        container
          .querySelector<HTMLButtonElement>("[aria-label='用摄像头扫描二维码']")
          ?.click(),
      );

      await act(async () => {
        await vi.advanceTimersByTimeAsync(500);
      });

      // The frame reaches the same decoder the image-file import uses.
      expect(readQrCodeMock).toHaveBeenCalledWith(
        new Uint8Array([0, 1, 2, 3]),
      );
      expect(importNodesMock).toHaveBeenCalledWith(
        "ss://aes-128-gcm:secret@edge.example.com:8388#Scanned",
      );
    } finally {
      vi.useRealTimers();
      getContext.mockRestore();
      toDataURL.mockRestore();
    }
  });


  it("sorts the server table by a header and back to manual order", async () => {
    const osaka = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000002",
      latencyMs: 20,
      name: "Osaka",
    };
    const tokyo = { ...SELECTED.node!, latencyMs: 200 };
    loadNodesMock.mockResolvedValue([tokyo, osaka]);
    await render();

    const names = () =>
      [...container.querySelectorAll("[aria-label='节点列表'] tbody tr")].map(
        (row) => row.querySelectorAll("td")[2]?.textContent,
      );
    expect(names()).toEqual([
      expect.stringContaining("Tokyo Edge"),
      expect.stringContaining("Osaka"),
    ]);

    const header = container.querySelector<HTMLButtonElement>(
      "[aria-label='按延迟排序表头']",
    );
    if (!header) {
      throw new Error("the latency header is not sortable");
    }
    await act(async () => header.click());
    expect(names()).toEqual([
      expect.stringContaining("Osaka"),
      expect.stringContaining("Tokyo Edge"),
    ]);
    expect(header.closest("th")?.getAttribute("aria-sort")).toBe("ascending");

    await act(async () => header.click());
    expect(header.closest("th")?.getAttribute("aria-sort")).toBe("descending");

    // A third click returns to the order the move actions maintain.
    await act(async () => header.click());
    expect(header.closest("th")?.getAttribute("aria-sort")).toBe("none");
    expect(names()).toEqual([
      expect.stringContaining("Tokyo Edge"),
      expect.stringContaining("Osaka"),
    ]);
  });

  it("will not reorder rows by hand while a sort is applied", async () => {
    const osaka = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
    };
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    await render();

    await act(async () =>
      container
        .querySelector<HTMLButtonElement>("[aria-label='按别名排序表头']")
        ?.click(),
    );

    // Moving a row means nothing when the view is not in the stored order.
    expect(await nodeMenuItemDisabled("Osaka", "上移")).toBe(true);
  });

  it("widens a column by dragging its grip and remembers it", async () => {
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    await render();

    const grip = container.querySelector<HTMLElement>(
      "[aria-label='调整别名列宽']",
    );
    if (!grip) {
      throw new Error("the name column has no resize grip");
    }
    const header = grip.closest("th");

    // Separate ticks: the drag listeners only exist once the mousedown has
    // committed, which is also how a real pointer behaves.
    await act(async () => {
      grip.dispatchEvent(
        new MouseEvent("mousedown", { bubbles: true, clientX: 100 }),
      );
    });
    await act(async () => {
      window.dispatchEvent(new MouseEvent("mousemove", { clientX: 180 }));
      window.dispatchEvent(new MouseEvent("mouseup", {}));
    });

    // The header started at the browser default width; the drag adds its delta.
    expect(header?.style.width).not.toBe("");
    const stored = JSON.parse(
      localStorage.getItem("mgclash.columnWidths") ?? "{}",
    ) as Record<string, number>;
    expect(stored["别名"]).toBeGreaterThan(48);
  });


  it("groups the node menu instead of stacking thirteen items", async () => {
    loadNodesMock.mockResolvedValue([SELECTED.node]);
    await render();

    const opener = container.querySelector<HTMLButtonElement>(
      "[aria-label='操作 Tokyo Edge']",
    );
    await act(async () => opener?.click());
    const menu = container.querySelector("[aria-label='节点操作 Tokyo Edge']");
    if (!menu) {
      throw new Error("the node menu did not open");
    }

    const labels = [...menu.querySelectorAll(".menu-group-label")].map(
      (entry) => entry.textContent,
    );
    expect(labels).toEqual(["连接", "编辑", "分享", "排序", "移除"]);
    // Grouping is presentation: every action is still one click away.
    const actions = [...menu.querySelectorAll("button")].map(
      (entry) => entry.textContent,
    );
    expect(actions).toContain("设为活动");
    expect(actions).toContain("显示二维码");
    expect(actions).toContain("移除重复");
  });

  it("changes the main window layout from the menu", async () => {
    await render();

    expect(container.querySelector(".app-shell")?.getAttribute("data-layout")).toBe(
      "horizontal",
    );

    await act(async () => button("标签页").click());

    expect(container.querySelector(".app-shell")?.getAttribute("data-layout")).toBe(
      "tab",
    );
    expect(localStorage.getItem("mgclash.mainLayout")).toBe("tab");
    expect(button("信息")).not.toBeNull();

    await act(async () => button("左右分栏").click());

    expect(container.querySelector(".app-shell")?.getAttribute("data-layout")).toBe(
      "horizontal",
    );
  });

  it("shows the message pane as a side tab", async () => {
    await render();

    expect(container.querySelector("[aria-label='消息窗口']")).not.toBeNull();
    await act(async () => button("当前代理").click());
    expect(container.querySelector("[aria-label='消息窗口']")).toBeNull();
    await act(async () => button("信息").click());
    expect(container.querySelector("[aria-label='消息窗口']")).not.toBeNull();
  });

  it("adds and imports servers while connected", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    loadNodesMock.mockResolvedValue([CONNECTED.node]);
    await render();

    expect(button("添加 [VLESS]").disabled).toBe(false);
    expect(button("从剪贴板导入分享链接").disabled).toBe(false);
    expect(button("扫描屏幕上的二维码").disabled).toBe(false);
  });

  it("edits other servers while connected but not the running one", async () => {
    const osaka = {
      ...SELECTED.node!,
      id: "00000000-0000-0000-0000-000000000002",
      name: "Osaka",
    };
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    loadNodesMock.mockResolvedValue([CONNECTED.node, osaka]);
    await render();

    expect(await nodeMenuItemDisabled("Osaka", "编辑")).toBe(false);
    expect(await nodeMenuItemDisabled("Osaka", "移除所选")).toBe(false);
    expect(await nodeMenuItemDisabled("Osaka", "禁用节点")).toBe(false);
    expect(await nodeMenuItemDisabled("Tokyo Edge", "编辑")).toBe(true);
    expect(await nodeMenuItemDisabled("Tokyo Edge", "移除所选")).toBe(true);
    expect(await nodeMenuItemDisabled("Tokyo Edge", "禁用节点")).toBe(true);
  });

  it("opens create forms for each protocol and the vertical layout", async () => {
    await render();

    for (const label of [
      "添加 [VLESS]",
      "添加 [VMess]",
      "添加 [Shadowsocks]",
      "添加 [Hysteria2]",
      "添加 [TUIC]",
      "添加 [SOCKS]",
      "添加 [HTTP]",
      "添加 [WireGuard]",
      "添加 [AnyTLS]",
      "添加 [Naive]",
    ]) {
      await act(async () => button(label).click());
      expect(
        container.querySelector("[aria-label='手动创建节点']")?.hasAttribute("hidden"),
      ).toBe(false);
      await act(async () => {
        container
          .querySelector<HTMLButtonElement>("[aria-label='手动创建节点'] .dialog-head button")
          ?.click();
      });
    }

    await act(async () => button("上下分栏").click());
    expect(container.querySelector(".app-shell")?.getAttribute("data-layout")).toBe(
      "vertical",
    );
  });

  it("saves editable SOCKS, HTTP, and Clash API ports from settings", async () => {
    saveAppSettingsMock.mockImplementation(async (next: unknown) => next);
    await render();

    await act(async () => button("参数设置").click());
    const socks = container.querySelector<HTMLInputElement>("[aria-label='SOCKS 端口']");
    const http = container.querySelector<HTMLInputElement>("[aria-label='HTTP 端口']");
    const clash = container.querySelector<HTMLInputElement>(
      "[aria-label='Clash API 端口']",
    );
    if (!socks || !http || !clash) {
      throw new Error("inbound port fields missing");
    }
    await act(async () => {
      typeInput("20808", socks);
      typeInput("20809", http);
      typeInput("29090", clash);
    });

    expect(saveAppSettingsMock).toHaveBeenCalledWith(
      expect.objectContaining({
        socksPort: 20808,
        httpPort: 20809,
        clashApiPort: 29090,
      }),
    );
  });
});
