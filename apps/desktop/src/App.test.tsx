// @vitest-environment jsdom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const loadPlatformSummaryMock = vi.hoisted(() => vi.fn());
const loadSessionStatusMock = vi.hoisted(() => vi.fn());
const loadNodesMock = vi.hoisted(() => vi.fn());
const importNodeMock = vi.hoisted(() => vi.fn());
const selectNodeMock = vi.hoisted(() => vi.fn());
const deleteNodeMock = vi.hoisted(() => vi.fn());
const connectSessionMock = vi.hoisted(() => vi.fn());
const disconnectSessionMock = vi.hoisted(() => vi.fn());
const exportDiagnosticsMock = vi.hoisted(() => vi.fn());
const loadSystemProxyStartupStatusMock = vi.hoisted(() => vi.fn());
const recoverSystemProxyMock = vi.hoisted(() => vi.fn());
const dismissSystemProxyRecoveryMock = vi.hoisted(() => vi.fn());
const loadSubscriptionsMock = vi.hoisted(() => vi.fn());
const createSubscriptionMock = vi.hoisted(() => vi.fn());
const updateSubscriptionMock = vi.hoisted(() => vi.fn());
const refreshSubscriptionMock = vi.hoisted(() => vi.fn());
const deleteSubscriptionMock = vi.hoisted(() => vi.fn());

vi.mock("./platform", () => ({
  loadPlatformSummary: loadPlatformSummaryMock,
}));

vi.mock("./session", async () => {
  const actual = await vi.importActual<typeof import("./session")>("./session");
  return {
    connectSession: connectSessionMock,
    disconnectSession: disconnectSessionMock,
    dismissSystemProxyRecovery: dismissSystemProxyRecoveryMock,
    exportDiagnostics: exportDiagnosticsMock,
    importNode: importNodeMock,
    isCommandError: actual.isCommandError,
    deleteNode: deleteNodeMock,
    loadNodes: loadNodesMock,
    loadSessionStatus: loadSessionStatusMock,
    loadSystemProxyStartupStatus: loadSystemProxyStartupStatusMock,
    recoverSystemProxy: recoverSystemProxyMock,
    selectNode: selectNodeMock,
  };
});

vi.mock("./subscriptions", () => ({
  createSubscription: createSubscriptionMock,
  deleteSubscription: deleteSubscriptionMock,
  loadSubscriptions: loadSubscriptionsMock,
  refreshSubscription: refreshSubscriptionMock,
  updateSubscription: updateSubscriptionMock,
}));

import App from "./App";
import type { SessionStatus } from "./session";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

const IDLE: SessionStatus = {
  connected: false,
  core: "sing-box",
  httpPort: 10809,
  mode: "global",
  node: null,
  socksPort: 10808,
  systemProxy: true,
};

const SELECTED: SessionStatus = {
  ...IDLE,
  node: {
    deletable: true,
    id: "00000000-0000-0000-0000-000000000001",
    name: "Tokyo Edge",
    port: 8388,
    protocol: "shadowsocks",
    server: "edge.example.com",
  },
};

const CONNECTED: SessionStatus = { ...SELECTED, connected: true };

const SUBSCRIPTION = {
  autoUpdate: true,
  enabled: true,
  id: "00000000-0000-0000-0000-000000000010",
  lastUpdatedAt: null,
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
    loadNodesMock.mockReset();
    importNodeMock.mockReset();
    selectNodeMock.mockReset();
    deleteNodeMock.mockReset();
    connectSessionMock.mockReset();
    disconnectSessionMock.mockReset();
    exportDiagnosticsMock.mockReset();
    loadSystemProxyStartupStatusMock.mockReset();
    recoverSystemProxyMock.mockReset();
    dismissSystemProxyRecoveryMock.mockReset();
    loadSubscriptionsMock.mockReset();
    createSubscriptionMock.mockReset();
    updateSubscriptionMock.mockReset();
    refreshSubscriptionMock.mockReset();
    deleteSubscriptionMock.mockReset();

    loadPlatformSummaryMock.mockResolvedValue({
      artifactIdentifier: "macos-x86_64",
      tunAvailability: "unavailableInUnsignedBuild",
    });
    loadSessionStatusMock.mockResolvedValue(IDLE);
    loadNodesMock.mockResolvedValue([]);
    loadSystemProxyStartupStatusMock.mockResolvedValue("clean");
    recoverSystemProxyMock.mockResolvedValue("clean");
    dismissSystemProxyRecoveryMock.mockResolvedValue("clean");
    loadSubscriptionsMock.mockResolvedValue([]);

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

    const field = container.querySelector("textarea");
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

  it("lists persisted nodes and changes the selection", async () => {
    const osaka = {
      id: "00000000-0000-0000-0000-000000000002",
      deletable: true,
      name: "Osaka",
      port: 9000,
      protocol: "shadowsocks" as const,
      server: "osaka.example.com",
    };
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock.mockResolvedValue([SELECTED.node, osaka]);
    selectNodeMock.mockResolvedValue({ ...SELECTED, node: osaka });
    await render();

    const select = container.querySelector<HTMLButtonElement>(
      "[aria-label='选择 Osaka']",
    );
    if (!select) {
      throw new Error("no Osaka selection button");
    }
    await act(async () => select.click());

    expect(selectNodeMock).toHaveBeenCalledWith(osaka.id);
    expect(container.textContent).toContain("osaka.example.com:9000");
  });

  it("deletes a persisted node", async () => {
    loadSessionStatusMock.mockResolvedValue(SELECTED);
    loadNodesMock
      .mockResolvedValueOnce([SELECTED.node])
      .mockResolvedValueOnce([]);
    deleteNodeMock.mockResolvedValue(IDLE);
    await render();

    const remove = container.querySelector<HTMLButtonElement>(
      "[aria-label='删除 Tokyo Edge']",
    );
    if (!remove) {
      throw new Error("no Tokyo deletion button");
    }
    await act(async () => remove.click());

    expect(deleteNodeMock).toHaveBeenCalledWith(SELECTED.node?.id);
    expect(container.querySelector("[aria-label='节点列表']")).toBeNull();
    expect(container.textContent).toContain("尚未导入节点");
  });

  it("keeps subscription-owned nodes read-only", async () => {
    const managed = {
      ...SELECTED.node,
      deletable: false,
      name: "Managed Tokyo",
    };
    loadNodesMock.mockResolvedValue([managed]);
    await render();

    const remove = container.querySelector<HTMLButtonElement>(
      "[aria-label='删除 Managed Tokyo']",
    );

    expect(remove?.disabled).toBe(true);
    expect(remove?.textContent).toBe("订阅管理");
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

    const edit = container.querySelector<HTMLButtonElement>(
      "[aria-label='编辑 Airport']",
    );
    if (!edit) {
      throw new Error("subscription edit button is missing");
    }
    await act(async () => edit.click());

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
