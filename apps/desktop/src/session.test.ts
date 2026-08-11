import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  connectSession,
  dismissSystemProxyRecovery,
  disconnectSession,
  exportDiagnostics,
  importNode,
  isCommandError,
  deleteNode,
  loadNodes,
  loadSessionStatus,
  loadSystemProxyStartupStatus,
  recoverSystemProxy,
  selectNode,
  type SessionStatus,
} from "./session";

const IDLE: SessionStatus = {
  connected: false,
  core: "sing-box",
  httpPort: 10809,
  mode: "global",
  node: null,
  socksPort: 10808,
  systemProxy: true,
};

describe("session commands", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(IDLE);
  });

  it("reads the current status through the Rust command", async () => {
    await expect(loadSessionStatus()).resolves.toEqual(IDLE);
    expect(invokeMock).toHaveBeenCalledWith("session_status");
  });

  it("sends the sharing URI when importing a node", async () => {
    await importNode("ss://aes-128-gcm:secret@edge.example.com:8388");

    expect(invokeMock).toHaveBeenCalledWith("session_import_node", {
      uri: "ss://aes-128-gcm:secret@edge.example.com:8388",
    });
  });

  it("lists, selects, and deletes persisted nodes", async () => {
    await loadNodes();
    await selectNode("00000000-0000-0000-0000-000000000001");
    await deleteNode("00000000-0000-0000-0000-000000000002");

    expect(invokeMock).toHaveBeenNthCalledWith(1, "session_nodes");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "session_select_node", {
      id: "00000000-0000-0000-0000-000000000001",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(3, "session_delete_node", {
      id: "00000000-0000-0000-0000-000000000002",
    });
  });

  it("connects and disconnects through their own commands", async () => {
    await connectSession();
    await disconnectSession();

    expect(invokeMock).toHaveBeenNthCalledWith(1, "session_connect");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "session_disconnect");
  });

  it("exports diagnostics through its own command", async () => {
    invokeMock.mockResolvedValue("/data/mgclash-diagnostics-1.json");

    await expect(exportDiagnostics()).resolves.toBe(
      "/data/mgclash-diagnostics-1.json",
    );
    expect(invokeMock).toHaveBeenCalledWith("export_diagnostics");
  });

  it("reads and resolves startup System Proxy recovery", async () => {
    invokeMock.mockResolvedValue("restoreRequired");

    await expect(loadSystemProxyStartupStatus()).resolves.toBe(
      "restoreRequired",
    );
    expect(invokeMock).toHaveBeenCalledWith("system_proxy_startup_status");

    invokeMock.mockResolvedValue("clean");
    await expect(recoverSystemProxy()).resolves.toBe("clean");
    await expect(dismissSystemProxyRecovery()).resolves.toBe("clean");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "system_proxy_recover");
    expect(invokeMock).toHaveBeenNthCalledWith(3, "system_proxy_dismiss");
  });

  it("surfaces command failures", async () => {
    const failure = { code: "no_selected_node", message: "no node yet" };
    invokeMock.mockRejectedValue(failure);

    await expect(connectSession()).rejects.toBe(failure);
  });
});

describe("isCommandError", () => {
  it("recognizes the typed error the Rust commands return", () => {
    expect(isCommandError({ code: "invalid_share_link", message: "bad" })).toBe(
      true,
    );
  });

  it("rejects anything else", () => {
    expect(isCommandError(null)).toBe(false);
    expect(isCommandError(new Error("boom"))).toBe(false);
    expect(isCommandError({ code: 1, message: "bad" })).toBe(false);
    expect(isCommandError({ code: "only-code" })).toBe(false);
  });
});
