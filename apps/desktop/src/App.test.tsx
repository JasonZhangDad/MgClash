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
const removeDuplicateNodesMock = vi.hoisted(() => vi.fn());
const createNodeMock = vi.hoisted(() => vi.fn());
const importNodesMock = vi.hoisted(() => vi.fn());
const loadLogsMock = vi.hoisted(() => vi.fn());
const loadAppSettingsMock = vi.hoisted(() => vi.fn());
const saveAppSettingsMock = vi.hoisted(() => vi.fn());
const clearLogsMock = vi.hoisted(() => vi.fn());
const selectNodeMock = vi.hoisted(() => vi.fn());
const deleteNodeMock = vi.hoisted(() => vi.fn());
const editNodeMock = vi.hoisted(() => vi.fn());
const moveNodeMock = vi.hoisted(() => vi.fn());
const setNodeGroupMock = vi.hoisted(() => vi.fn());
const testNodeMock = vi.hoisted(() => vi.fn());
const testAllNodesMock = vi.hoisted(() => vi.fn());
const testUrlMock = vi.hoisted(() => vi.fn());
const connectSessionMock = vi.hoisted(() => vi.fn());
const disconnectSessionMock = vi.hoisted(() => vi.fn());
const setRoutingModeMock = vi.hoisted(() => vi.fn());
const setDnsSettingsMock = vi.hoisted(() => vi.fn());
const setRouteSettingsMock = vi.hoisted(() => vi.fn());
const exportDiagnosticsMock = vi.hoisted(() => vi.fn());
const loadSystemProxyStartupStatusMock = vi.hoisted(() => vi.fn());
const recoverSystemProxyMock = vi.hoisted(() => vi.fn());
const dismissSystemProxyRecoveryMock = vi.hoisted(() => vi.fn());
const loadSubscriptionsMock = vi.hoisted(() => vi.fn());
const createSubscriptionMock = vi.hoisted(() => vi.fn());
const updateSubscriptionMock = vi.hoisted(() => vi.fn());
const refreshSubscriptionMock = vi.hoisted(() => vi.fn());
const refreshAllSubscriptionsMock = vi.hoisted(() => vi.fn());
const deleteSubscriptionMock = vi.hoisted(() => vi.fn());

vi.mock("./platform", () => ({
  loadPlatformSummary: loadPlatformSummaryMock,
}));

vi.mock("./session", async () => {
  const actual = await vi.importActual<typeof import("./session")>("./session");
  return {
    clearLogs: clearLogsMock,
    connectSession: connectSessionMock,
    createNode: createNodeMock,
    loadAppSettings: loadAppSettingsMock,
    loadLogs: loadLogsMock,
    saveAppSettings: saveAppSettingsMock,
    disconnectSession: disconnectSessionMock,
    dismissSystemProxyRecovery: dismissSystemProxyRecoveryMock,
    exportDiagnostics: exportDiagnosticsMock,
    editNode: editNodeMock,
    importNode: importNodeMock,
    importNodes: importNodesMock,
    isCommandError: actual.isCommandError,
    deleteNode: deleteNodeMock,
  exportNodeLink: exportNodeLinkMock,
  cloneNode: cloneNodeMock,
  removeDuplicateNodes: removeDuplicateNodesMock,
    loadNodeGroups: loadNodeGroupsMock,
    loadNodes: loadNodesMock,
    loadSessionStatus: loadSessionStatusMock,
    loadTraffic: loadTrafficMock,
  loadNodeTraffic: loadNodeTrafficMock,
    moveNode: moveNodeMock,
    loadSystemProxyStartupStatus: loadSystemProxyStartupStatusMock,
    recoverSystemProxy: recoverSystemProxyMock,
    selectNode: selectNodeMock,
    setRoutingMode: setRoutingModeMock,
    setNodeGroup: setNodeGroupMock,
    setDnsSettings: setDnsSettingsMock,
    setRouteSettings: setRouteSettingsMock,
    testAllNodes: testAllNodesMock,
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
  systemProxy: true,
  systemProxyMode: "managed" as const,
};

const SELECTED: SessionStatus = {
  ...IDLE,
  node: {
    deletable: true,
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
};

const SUBSCRIPTION = {
  autoUpdate: true,
  enabled: true,
  id: "00000000-0000-0000-0000-000000000010",
  lastUpdatedAt: null,
  lastError: null,
  name: "Airport",
  nodeCount: 3,
  updateIntervalMinutes: 60,
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
    deleteNodeMock.mockReset();
    editNodeMock.mockReset();
    moveNodeMock.mockReset();
    setNodeGroupMock.mockReset();
    testNodeMock.mockReset();
    testAllNodesMock.mockReset();
    testUrlMock.mockReset();
    connectSessionMock.mockReset();
    disconnectSessionMock.mockReset();
    setRoutingModeMock.mockReset();
    setDnsSettingsMock.mockReset();
    setRouteSettingsMock.mockReset();
    exportDiagnosticsMock.mockReset();
    loadSystemProxyStartupStatusMock.mockReset();
    recoverSystemProxyMock.mockReset();
    dismissSystemProxyRecoveryMock.mockReset();
    loadLogsMock.mockReset();
    loadAppSettingsMock.mockReset();
    saveAppSettingsMock.mockReset();
    clearLogsMock.mockReset();
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

  it("shows persisted traffic totals while disconnected", async () => {
    loadTrafficMock.mockResolvedValue({
      downloadBytesPerSecond: 0,
      monthBytes: 1_048_576,
      todayBytes: 2_048,
      totalBytes: 1_073_741_824,
      uploadBytesPerSecond: 0,
    });

    await render();

    expect(container.querySelector("[aria-label='今日流量']")?.textContent).toBe(
      "2.0 KB",
    );
    expect(container.querySelector("[aria-label='本月流量']")?.textContent).toBe(
      "1.0 MB",
    );
    expect(container.querySelector("[aria-label='累计流量']")?.textContent).toBe(
      "1.0 GB",
    );
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
    ).toBe("2.0 KB/s");
    expect(
      container.querySelector("[aria-label='上传速率']")?.textContent,
    ).toBe("1.0 MB/s");
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
    await render();

    await nodeMenuAction("Osaka", "设为活动");

    expect(selectNodeMock).toHaveBeenCalledWith(osaka.id);
    expect(container.textContent).toContain("osaka.example.com:9000");
  });

  it("edits a manual node with a compact inline form", async () => {
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
    editNodeMock.mockResolvedValue({ ...SELECTED, node: editedNode });
    await render();

    await nodeMenuAction("Tokyo Edge", "编辑");

    const name = container.querySelector<HTMLInputElement>(
      "[aria-label='节点名称']",
    );
    const server = container.querySelector<HTMLInputElement>(
      "[aria-label='节点服务器']",
    );
    const port = container.querySelector<HTMLInputElement>(
      "[aria-label='节点端口']",
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

    expect(editNodeMock).toHaveBeenCalledWith(SELECTED.node?.id, {
      name: "Tokyo 2",
      port: 443,
      server: "new.example.com",
    });
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

    const filter = container.querySelector<HTMLSelectElement>(
      "[aria-label='节点分组筛选']",
    );
    if (!filter) {
      throw new Error("node group filter is missing");
    }
    await act(async () => selectValue(work.id, filter));
    expect(
      [...container.querySelectorAll("[aria-label='节点列表'] tbody tr")].map(
        (row) => row.textContent,
      ),
    ).toEqual([expect.stringContaining("Osaka")]);

    await act(async () => selectValue("all", filter));
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
    expect(localStorage.getItem("mgclash.urlTestAddress")).toBe(
      "https://probe.example/204",
    );
    expect(
      container.querySelector("[aria-label='节点列表']")?.textContent,
    ).toContain("55 ms");

    act(() => root.unmount());
    container = document.createElement("div");
    document.body.append(container);
    root = createRoot(container);
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

    expect(localStorage.getItem("mgclash.urlTestAddress")).toBeNull();
    expect(container.querySelector("[role='alert']")?.textContent).toContain(
      "URL 测试地址无效",
    );
  });

  it("requires a connected node for URL testing", async () => {
    await render();

    expect(button("URL 测试").disabled).toBe(true);
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
    await render();
    const rows = container.querySelectorAll("[aria-label='节点列表'] tbody tr");

    await act(async () => {
      rows[1].dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    });

    expect(selectNodeMock).toHaveBeenCalledWith(osaka.id);
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
    // which is why there are three values rather than a checkbox.
    expect(saveAppSettingsMock).toHaveBeenCalledWith(
      expect.objectContaining({ systemProxyMode: "unchanged" }),
    );
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

  it("keeps subscription-owned nodes read-only", async () => {
    const managed = {
      ...SELECTED.node,
      deletable: false,
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
      name: "Airport",
      updateIntervalMinutes: 60,
      url: "https://example.com/secret",
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
      id: SUBSCRIPTION.id,
      name: "Airport 2",
      updateIntervalMinutes: 60,
      url: null,
    });
    expect(container.textContent).toContain("Airport 2");
  });

  it("refreshes and deletes a subscription", async () => {
    const managedNode = {
      ...SELECTED.node,
      deletable: false,
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
    expect(container.textContent).toContain("已接管系统代理");

    await act(async () => button("断开").click());

    expect(disconnectSessionMock).toHaveBeenCalledOnce();
    expect(container.textContent).toContain("未连接");
  });

  it("switches routing mode while disconnected", async () => {
    const ruleStatus: SessionStatus = { ...IDLE, mode: "rule" };
    setRoutingModeMock.mockResolvedValue(ruleStatus);
    await render();
    const field = container.querySelector<HTMLSelectElement>(
      "select[aria-label='路由模式']",
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
        "select[aria-label='路由模式']",
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
});
