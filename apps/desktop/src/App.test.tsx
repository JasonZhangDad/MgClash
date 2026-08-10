// @vitest-environment jsdom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const loadPlatformSummaryMock = vi.hoisted(() => vi.fn());
const loadSessionStatusMock = vi.hoisted(() => vi.fn());
const importNodeMock = vi.hoisted(() => vi.fn());
const connectSessionMock = vi.hoisted(() => vi.fn());
const disconnectSessionMock = vi.hoisted(() => vi.fn());

vi.mock("./platform", () => ({
  loadPlatformSummary: loadPlatformSummaryMock,
}));

vi.mock("./session", async () => {
  const actual = await vi.importActual<typeof import("./session")>("./session");
  return {
    connectSession: connectSessionMock,
    disconnectSession: disconnectSessionMock,
    importNode: importNodeMock,
    isCommandError: actual.isCommandError,
    loadSessionStatus: loadSessionStatusMock,
  };
});

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
    name: "Tokyo Edge",
    port: 8388,
    protocol: "shadowsocks",
    server: "edge.example.com",
  },
};

const CONNECTED: SessionStatus = { ...SELECTED, connected: true };

describe("App", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    loadPlatformSummaryMock.mockReset();
    loadSessionStatusMock.mockReset();
    importNodeMock.mockReset();
    connectSessionMock.mockReset();
    disconnectSessionMock.mockReset();

    loadPlatformSummaryMock.mockResolvedValue({
      artifactIdentifier: "macos-x86_64",
      tunAvailability: "unavailableInUnsignedBuild",
    });
    loadSessionStatusMock.mockResolvedValue(IDLE);

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

  function button(label: string): HTMLButtonElement {
    const found = [...container.querySelectorAll("button")].find(
      (candidate) => candidate.textContent === label,
    );
    if (!found) {
      throw new Error(`no "${label}" button in ${container.innerHTML}`);
    }
    return found;
  }

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

  it("imports a share link and enables connecting", async () => {
    importNodeMock.mockResolvedValue(SELECTED);
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
