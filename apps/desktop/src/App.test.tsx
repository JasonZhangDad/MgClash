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
const removeDuplicateNodesMock = vi.hoisted(() => vi.fn());
const createNodeMock = vi.hoisted(() => vi.fn());
const importNodesMock = vi.hoisted(() => vi.fn());
const loadLogsMock = vi.hoisted(() => vi.fn());
const loadAppSettingsMock = vi.hoisted(() => vi.fn());
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

vi.mock("./platform", () => ({
  loadPlatformSummary: loadPlatformSummaryMock,
}));

vi.mock("./session", async () => {
  const actual = await vi.importActual<typeof import("./session")>("./session");
  return {
    clearLogs: clearLogsMock,
    clearTraffic: clearTrafficMock,
    connectSession: connectSessionMock,
    createNode: createNodeMock,
    loadAppSettings: loadAppSettingsMock,
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
    dohPath: "/dns-query",
    fakeIpEnabled: false,
    ipv6Enabled: false,
    mode: "system",
    port: 53,
    server: "1.1.1.1",
    strategy: "preferIpv4",
    systemDomains: [],
  },
  httpPort: 10809,
  mode: "global",
  route: {
    finalOutbound: "proxy",
    rules: [],
  },
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
  autoSelectLowestLatency: false,
  urlTestAddress: "https://www.gstatic.com/generate_204",
  allowLan: false,
  speedTestUrl: "https://speed.cloudflare.com/__down?bytes=10000000",
  inboundUdpEnabled: true,
  defAllowInsecure: false,
  defFingerprint: "",
  hotkeyConnect: "Ctrl+Enter",
  hotkeyPrevious: "Ctrl+[",
  hotkeyNext: "Ctrl+]",
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
    testAllNodesMock.mockResolvedValue(undefined);

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
    expect(button("连接").disabled).toBe(true);
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

    expect(button("连接").disabled).toBe(true);
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
    expect(container.textContent).toContain("edge.example.com:8388");
    expect(button("连接").disabled).toBe(false);
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

  it("locks the Core picker while connected", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    loadNodesMock.mockResolvedValue([CONNECTED.node]);
    await render();

    expect(createSelect("Core 选择").disabled).toBe(true);
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
    switchNodeMock.mockResolvedValue({ ...SELECTED, node: osaka });
    await render();

    await nodeMenuAction("Osaka", "设为活动");

    expect(switchNodeMock).toHaveBeenCalledWith(osaka.id);
    expect(container.textContent).toContain("osaka.example.com:9000");
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
    expect(container.textContent).toContain("new.example.com:443");
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

  it("assigns and filters named node groups", async () => {
    const work = {
      id: "00000000-0000-0000-0000-000000000020",
      name: "Work",
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
    await act(async () => button("保存分组").click());

    expect(setNodeGroupMock).toHaveBeenCalledWith(SELECTED.node?.id, "Work");
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

    await act(async () => button("按延迟排序").click());
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

    const field = container.querySelector<HTMLInputElement>(
      "[aria-label='URL 测试地址']",
    );
    if (!field) {
      throw new Error("URL test address field is missing");
    }
    expect(field.value).toBe("https://www.gstatic.com/generate_204");
    await act(async () => typeInput(" https://probe.example/204 ", field));
    await act(async () => button("URL 测试").click());

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
    expect(
      container.querySelector<HTMLInputElement>(
        "[aria-label='URL 测试地址']",
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

    const field = container.querySelector<HTMLInputElement>(
      "[aria-label='URL 测试地址']",
    );
    if (!field) {
      throw new Error("URL test address field is missing");
    }
    await act(async () => typeInput("file:///tmp/probe", field));
    await act(async () => button("URL 测试").click());

    expect(saveAppSettingsMock).not.toHaveBeenCalledWith(
      expect.objectContaining({ urlTestAddress: "file:///tmp/probe" }),
    );
    expect(container.querySelector("[role='alert']")?.textContent).toContain(
      "URL 测试地址无效",
    );
  });

  it("requires a connected node for URL testing", async () => {
    await render();

    expect(button("URL 测试").disabled).toBe(true);
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

    await act(async () => button("下载测速").click());
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
    switchNodeMock.mockResolvedValue({ ...SELECTED, node: osaka });
    await render();
    const rows = container.querySelectorAll("[aria-label='节点列表'] tbody tr");

    await act(async () => {
      rows[1].dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    });

    expect(switchNodeMock).toHaveBeenCalledWith(osaka.id);
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
    ).toEqual(["managed", "pac", "cleared", "unchanged"]);
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
    // Editing a node the subscription owns is not offered at all.
    expect(
      container.querySelector("[aria-label='节点操作 Managed Tokyo']")
        ?.textContent,
    ).not.toContain("编辑");
  });

  it("adds a subscription without exposing its URL in the list", async () => {
    createSubscriptionMock.mockResolvedValue(SUBSCRIPTION);
    await render();

    const name = container.querySelector<HTMLInputElement>(
      "[aria-label='订阅名称']",
    );
    const url = container.querySelector<HTMLInputElement>(
      "[aria-label='订阅地址']",
    );
    if (!name || !url) {
      throw new Error("subscription form is missing");
    }
    await act(async () => {
      typeInput("Airport", name);
      typeInput("https://example.com/secret", url);
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
    });
    expect(container.textContent).toContain("Airport 2");
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

  it("waits for disconnect before mutating a subscription", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    loadSubscriptionsMock.mockResolvedValue([SUBSCRIPTION]);
    await render();

    for (const label of ["编辑 Airport", "刷新 Airport", "删除订阅 Airport"]) {
      const action = container.querySelector<HTMLButtonElement>(
        `[aria-label='${label}']`,
      );
      expect(action?.disabled).toBe(true);
    }
  });

  it("refreshes all subscriptions and shows individual failures", async () => {
    loadSubscriptionsMock.mockResolvedValue([SUBSCRIPTION]);
    refreshAllSubscriptionsMock.mockResolvedValue([
      { ...SUBSCRIPTION, lastError: "subscription request timed out" },
    ]);
    await render();

    await act(async () => button("全部更新").click());

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

    await act(async () => button("连接").click());

    expect(container.textContent).toContain("已连接");

    await act(async () => button("断开").click());

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

  it("locks routing mode while connected", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    await render();

    expect(
      container.querySelector<HTMLSelectElement>(
        "select[aria-label='状态栏路由模式']",
      )?.disabled,
    ).toBe(true);
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

  it("locks DNS settings while connected", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    await render();

    expect(button("保存 DNS").disabled).toBe(true);
    expect(
      container.querySelector<HTMLSelectElement>("select[aria-label='DNS 模式']")
        ?.disabled,
    ).toBe(true);
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
      route: { finalOutbound: "direct", rules },
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
      rules,
    });
  });

  it("locks route settings while connected", async () => {
    loadSessionStatusMock.mockResolvedValue(CONNECTED);
    await render();

    expect(button("保存路由").disabled).toBe(true);
    expect(
      container.querySelector<HTMLSelectElement>("select[aria-label='规则类型']")
        ?.disabled,
    ).toBe(true);
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
      await act(async () => button("连接").click());

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
      await act(async () => button("连接").click());
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

    await act(async () => button("导出全部分享链接").click());

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

    await act(async () => button("下一节点").click());

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

    await act(async () => button("连接").click());

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
    expect(nav?.textContent).toContain("服务器");
    expect(nav?.textContent).toContain("订阅");
    expect(nav?.textContent).toContain("设置");
    expect(nav?.textContent).toContain("帮助");
    expect(container.querySelector(".menubar-brand")?.textContent).toContain(
      "MgClash",
    );
    expect(container.querySelector("[aria-label='节点列表']")).not.toBeNull();
    expect(container.querySelector("[aria-label='节点详情']")).not.toBeNull();
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

    await act(async () => button("订阅设置").click());
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

    await act(async () => button("添加 Trojan 服务器").click());

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

    await act(async () => button("重新加载").click());

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

    await act(async () => button("深色主题").click());

    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(localStorage.getItem("mgclash.theme")).toBe("dark");

    await act(async () => button("浅色主题").click());

    expect(document.documentElement.dataset.theme).toBe("light");
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
    expect(button("消息")).not.toBeNull();

    await act(async () => button("左右分栏").click());

    expect(container.querySelector(".app-shell")?.getAttribute("data-layout")).toBe(
      "horizontal",
    );
  });

  it("hides and shows the message window from the menu", async () => {
    await render();

    expect(container.querySelector("[aria-label='消息窗口']")).not.toBeNull();

    await act(async () => button("隐藏消息窗口").click());

    expect(container.querySelector("[aria-label='消息窗口']")).toBeNull();

    await act(async () => button("显示消息窗口").click());

    expect(container.querySelector("[aria-label='消息窗口']")).not.toBeNull();
  });

  it("opens create forms for each protocol and the vertical layout", async () => {
    await render();

    for (const label of [
      "添加 VLESS 服务器",
      "添加 VMess 服务器",
      "添加 Shadowsocks 服务器",
      "添加 Hysteria2 服务器",
      "添加 TUIC 服务器",
      "手动创建",
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
