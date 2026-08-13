import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import {
  connectSession,
  createNode,
  dismissSystemProxyRecovery,
  disconnectSession,
  editNode,
  exportDiagnostics,
  importNode,
  isCommandError,
  deleteNode,
  loadTraffic,
  loadNodes,
  loadNodeDraft,
  loadNodeGroups,
  loadSessionStatus,
  loadSystemProxyStartupStatus,
  moveNode,
  reorderNodes,
  recoverSystemProxy,
  selectNode,
  setNodeEnabled,
  setNodeGroup,
  setDnsSettings,
  setRouteSettings,
  setRoutingMode,
  testAllNodes,
  testNode,
  testUrl,
  updateNode,
  type NodeTestResult,
  type SessionStatus,
} from "./session";

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

describe("session commands", () => {
  it("creates a node from a manual draft", async () => {
    await createNode({
      credential: {
        method: "aes-256-gcm",
        password: "secret",
        protocol: "shadowsocks",
      },
      name: "Tokyo",
      port: 8388,
      server: "edge.example.com",
      tls: null,
      transport: { type: "tcp" },
      udpEnabled: true,
    });
    expect(invokeMock).toHaveBeenCalledWith("session_create_node", {
      draft: expect.objectContaining({ name: "Tokyo" }),
    });
  });

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

  it("lists, selects, edits, updates, and deletes persisted nodes", async () => {
    await loadNodes();
    await loadNodeGroups();
    await selectNode("00000000-0000-0000-0000-000000000001");
    await setNodeEnabled("00000000-0000-0000-0000-000000000001", false);
    await editNode("00000000-0000-0000-0000-000000000001", {
      name: "Tokyo 2",
      port: 443,
      server: "new.example.com",
    });
    await loadNodeDraft("00000000-0000-0000-0000-000000000001");
    await updateNode("00000000-0000-0000-0000-000000000001", {
      credential: {
        method: "aes-256-gcm",
        password: "secret",
        protocol: "shadowsocks",
      },
      name: "Tokyo 3",
      port: 8443,
      server: "edge.example.com",
      tls: null,
      transport: { type: "tcp" },
      udpEnabled: true,
    });
    await moveNode("00000000-0000-0000-0000-000000000001", "down");
    await reorderNodes([
      "00000000-0000-0000-0000-000000000002",
      "00000000-0000-0000-0000-000000000001",
    ]);
    await setNodeGroup(
      "00000000-0000-0000-0000-000000000001",
      " Work ",
    );
    await deleteNode("00000000-0000-0000-0000-000000000002");

    expect(invokeMock).toHaveBeenNthCalledWith(1, "session_nodes");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "session_node_groups");
    expect(invokeMock).toHaveBeenNthCalledWith(3, "session_select_node", {
      id: "00000000-0000-0000-0000-000000000001",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(4, "session_set_node_enabled", {
      enabled: false,
      id: "00000000-0000-0000-0000-000000000001",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(5, "session_edit_node", {
      id: "00000000-0000-0000-0000-000000000001",
      name: "Tokyo 2",
      port: 443,
      server: "new.example.com",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(6, "session_node_draft", {
      id: "00000000-0000-0000-0000-000000000001",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(7, "session_update_node", {
      id: "00000000-0000-0000-0000-000000000001",
      draft: expect.objectContaining({ name: "Tokyo 3" }),
    });
    expect(invokeMock).toHaveBeenNthCalledWith(8, "session_move_node", {
      direction: "down",
      id: "00000000-0000-0000-0000-000000000001",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(9, "session_reorder_nodes", {
      ids: [
        "00000000-0000-0000-0000-000000000002",
        "00000000-0000-0000-0000-000000000001",
      ],
    });
    expect(invokeMock).toHaveBeenNthCalledWith(10, "session_set_node_group", {
      groupName: " Work ",
      id: "00000000-0000-0000-0000-000000000001",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(11, "session_delete_node", {
      id: "00000000-0000-0000-0000-000000000002",
    });
  });

  it("tests one node through the Rust command", async () => {
    const result: NodeTestResult = {
      id: "00000000-0000-0000-0000-000000000001",
      latencyMs: 42,
      status: "success",
    };
    invokeMock.mockResolvedValue(result);

    await expect(testNode(result.id)).resolves.toEqual(result);
    expect(invokeMock).toHaveBeenCalledWith("session_test_node", {
      id: result.id,
    });
  });

  it("tests a URL through the running local HTTP proxy", async () => {
    const result: NodeTestResult = {
      id: "00000000-0000-0000-0000-000000000001",
      latencyMs: 55,
      status: "success",
    };
    invokeMock.mockResolvedValue(result);

    await expect(testUrl("https://www.gstatic.com/generate_204")).resolves.toEqual(
      result,
    );
    expect(invokeMock).toHaveBeenCalledWith("session_url_test", {
      url: "https://www.gstatic.com/generate_204",
    });
  });

  it("reads live rates and persisted traffic totals", async () => {
    const traffic = {
      downloadBytesPerSecond: 2_048,
      monthBytes: 4_096,
      todayBytes: 3_072,
      totalBytes: 8_192,
      uploadBytesPerSecond: 1_024,
    };
    invokeMock.mockResolvedValue(traffic);

    await expect(loadTraffic()).resolves.toEqual(traffic);
    expect(invokeMock).toHaveBeenCalledWith("session_traffic");
  });

  it("tests at most eight nodes concurrently and cancels queued tests", async () => {
    const ids = Array.from(
      { length: 10 },
      (_, index) => `00000000-0000-0000-0000-${String(index).padStart(12, "0")}`,
    );
    const releases = new Map<string, (result: NodeTestResult) => void>();
    invokeMock.mockImplementation(
      (_command: string, arguments_: { id: string }) =>
        new Promise<NodeTestResult>((resolve) => {
          releases.set(arguments_.id, resolve);
        }),
    );
    let cancelled = false;
    const onResult = vi.fn();

    const batch = testAllNodes(ids, onResult, () => cancelled);
    expect(invokeMock).toHaveBeenCalledTimes(8);

    releases.get(ids[0])?.({ id: ids[0], latencyMs: 10, status: "success" });
    await Promise.resolve();
    await Promise.resolve();
    expect(invokeMock).toHaveBeenCalledTimes(9);

    cancelled = true;
    for (const [id, release] of releases) {
      release({ id, latencyMs: null, status: "timeout" });
    }
    await batch;

    expect(invokeMock).toHaveBeenCalledTimes(9);
    expect(onResult).toHaveBeenCalledTimes(9);
  });

  it("stops queuing after a command failure and waits for in-flight tests", async () => {
    const ids = Array.from(
      { length: 10 },
      (_, index) => `00000000-0000-0000-0001-${String(index).padStart(12, "0")}`,
    );
    const pending = new Map<
      string,
      {
        reject: (error: unknown) => void;
        resolve: (result: NodeTestResult) => void;
      }
    >();
    invokeMock.mockImplementation(
      (_command: string, arguments_: { id: string }) =>
        new Promise<NodeTestResult>((resolve, reject) => {
          pending.set(arguments_.id, { reject, resolve });
        }),
    );
    const failure = { code: "node_store_failed", message: "write failed" };
    let settled = false;

    const batch = testAllNodes(ids, vi.fn(), () => false).catch(() => {
      settled = true;
    });
    pending.get(ids[0])?.reject(failure);
    await Promise.resolve();
    await Promise.resolve();

    expect(settled).toBe(false);
    for (const [id, operation] of pending) {
      operation.resolve({ id, latencyMs: 10, status: "success" });
    }
    await batch;

    expect(settled).toBe(true);
    expect(invokeMock).toHaveBeenCalledTimes(8);
  });

  it("connects and disconnects through their own commands", async () => {
    await connectSession();
    await disconnectSession();

    expect(invokeMock).toHaveBeenNthCalledWith(1, "session_connect");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "session_disconnect");
  });

  it("changes the persisted routing mode through its own command", async () => {
    await setRoutingMode("rule");

    expect(invokeMock).toHaveBeenCalledWith("session_set_routing_mode", {
      mode: "rule",
    });
  });

  it("saves ordered route settings through their own command", async () => {
    const settings = {
      finalOutbound: "proxy" as const,
      providers: [],
      rules: [
        {
          enabled: true,
          kind: "domainSuffix" as const,
          outbound: "direct" as const,
          value: "cn",
        },
      ],
    };

    await setRouteSettings(settings);

    expect(invokeMock).toHaveBeenCalledWith("session_set_route_settings", {
      settings,
    });
  });

  it("saves the complete DNS settings through its own command", async () => {
    const settings = {
      ...IDLE.dns,
      mode: "doh" as const,
      port: 443,
      server: "cloudflare-dns.com",
    };

    await setDnsSettings(settings);

    expect(invokeMock).toHaveBeenCalledWith("session_set_dns_settings", {
      settings,
    });
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
