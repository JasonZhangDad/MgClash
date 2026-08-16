// @vitest-environment jsdom

import { beforeEach, describe, expect, it } from "vitest";

import {
  clampColumnWidth,
  describeFailure,
  groupTraffic,
  isBackendMissing,
  isCoreStartFailure,
  loadColumnWidths,
  saveColumnWidths,
  nextNodeSort,
  sortNodes,
  processTraffic,
  ruleDraftFromConnection,
  savedFontSize,
} from "./appHelpers";

describe("isCoreStartFailure", () => {
  it("recognizes a typed Core start failure and not a settings parse error", () => {
    expect(
      isCoreStartFailure({
        code: "core_not_configured",
        message: "the pinned sing-box binary is not configured",
      }),
    ).toBe(true);
    expect(
      isCoreStartFailure(new Error("sing-box process failed to start")),
    ).toBe(true);
    expect(
      isCoreStartFailure({
        code: "settings_unavailable",
        message: "the Core config template must be a JSON object",
      }),
    ).toBe(false);
    expect(describeFailure(new Error("refresh failed"))).toBe("refresh failed");
  });
});

describe("isBackendMissing", () => {
  it("recognizes a missing Tauri IPC and not a typed command error", () => {
    expect(
      isBackendMissing(
        new TypeError("Cannot read properties of undefined (reading 'invoke')"),
      ),
    ).toBe(true);
    expect(
      isBackendMissing(
        new Error("window.__TAURI_INTERNALS__ is not available"),
      ),
    ).toBe(true);
    expect(
      isBackendMissing({
        code: "invalid_share_link",
        message: "the share link is not a supported URI",
      }),
    ).toBe(false);
    expect(isBackendMissing(new Error("refresh failed"))).toBe(false);
  });
});

describe("ruleDraftFromConnection", () => {
  it("turns a hostname into a domain-suffix rule", () => {
    expect(ruleDraftFromConnection("cdn.example.com")).toEqual({
      kind: "domainSuffix",
      value: "cdn.example.com",
    });
  });

  it("turns an address into a single-host CIDR", () => {
    // A connection reports one address; the rule has to be a network, and the
    // only network that means "this one host" is a full-length prefix.
    expect(ruleDraftFromConnection("93.184.216.34")).toEqual({
      kind: "ipCidr",
      value: "93.184.216.34/32",
    });
    expect(ruleDraftFromConnection("2606:2800:220:1::")).toEqual({
      kind: "ipCidr6",
      value: "2606:2800:220:1::/128",
    });
  });

  it("drops the port the connection table shows", () => {
    expect(ruleDraftFromConnection("93.184.216.34:443")).toEqual({
      kind: "ipCidr",
      value: "93.184.216.34/32",
    });
    expect(ruleDraftFromConnection("cdn.example.com:443")).toEqual({
      kind: "domainSuffix",
      value: "cdn.example.com",
    });
  });

  it("has nothing to offer for an empty host", () => {
    expect(ruleDraftFromConnection("")).toBeNull();
    expect(ruleDraftFromConnection("   ")).toBeNull();
  });
});

describe("groupTraffic", () => {
  const nodes = [
    { frontNodeId: null, groupId: "g1", id: "n1", name: "Tokyo" },
    { frontNodeId: null, groupId: "g1", id: "n2", name: "Osaka" },
    { frontNodeId: null, groupId: null, id: "n3", name: "Loose" },
  ];
  const traffic = {
    n1: {
      todayUploadBytes: 10,
      todayDownloadBytes: 20,
      totalUploadBytes: 100,
      totalDownloadBytes: 200,
    },
    n2: {
      todayUploadBytes: 1,
      todayDownloadBytes: 2,
      totalUploadBytes: 10,
      totalDownloadBytes: 20,
    },
  };

  it("adds up every member of a group", () => {
    expect(groupTraffic(nodes, traffic, "g1")).toEqual({
      todayUploadBytes: 11,
      todayDownloadBytes: 22,
      totalUploadBytes: 110,
      totalDownloadBytes: 220,
    });
  });

  it("counts a member with no recorded traffic as zero", () => {
    expect(groupTraffic(nodes, traffic, null)).toEqual({
      todayUploadBytes: 0,
      todayDownloadBytes: 0,
      totalUploadBytes: 0,
      totalDownloadBytes: 0,
    });
  });
});

describe("processTraffic", () => {
  const connections = [
    { downloadBytes: 30, host: "a", process: "Safari", uploadBytes: 3 },
    { downloadBytes: 40, host: "b", process: "Safari", uploadBytes: 4 },
    { downloadBytes: 5, host: "c", process: "", uploadBytes: 1 },
  ];

  it("adds up the live connections of each program, busiest first", () => {
    expect(processTraffic(connections)).toEqual([
      { downloadBytes: 70, name: "Safari", uploadBytes: 7 },
      { downloadBytes: 5, name: "—", uploadBytes: 1 },
    ]);
  });

  it("has nothing to show without connections", () => {
    expect(processTraffic([])).toEqual([]);
  });
});

describe("sortNodes", () => {
  const nodes = [
    {
      frontNodeId: null,
      groupId: null,
      id: "n1",
      latencyMs: 120,
      name: "Bravo",
      port: 443,
      protocol: "vless" as const,
      server: "b.example.com",
    },
    {
      frontNodeId: null,
      groupId: null,
      id: "n2",
      latencyMs: null,
      name: "alpha",
      port: 443,
      protocol: "trojan" as const,
      server: "a.example.com",
    },
    {
      frontNodeId: null,
      groupId: null,
      id: "n3",
      latencyMs: 30,
      name: "Charlie",
      port: 443,
      protocol: "vmess" as const,
      server: "c.example.com",
    },
  ];

  it("keeps the manual order when nothing is sorted", () => {
    expect(sortNodes(nodes, null).map((node) => node.id)).toEqual([
      "n1",
      "n2",
      "n3",
    ]);
  });

  it("sorts names without minding case", () => {
    expect(
      sortNodes(nodes, { column: "name", direction: "asc" }).map(
        (node) => node.name,
      ),
    ).toEqual(["alpha", "Bravo", "Charlie"]);
    expect(
      sortNodes(nodes, { column: "name", direction: "desc" }).map(
        (node) => node.name,
      ),
    ).toEqual(["Charlie", "Bravo", "alpha"]);
  });

  it("puts untested nodes last whichever way latency is sorted", () => {
    // An untested node has no latency to compare; burying it keeps the fastest
    // and the slowest both reachable from the top.
    expect(
      sortNodes(nodes, { column: "latency", direction: "asc" }).map(
        (node) => node.id,
      ),
    ).toEqual(["n3", "n1", "n2"]);
    expect(
      sortNodes(nodes, { column: "latency", direction: "desc" }).map(
        (node) => node.id,
      ),
    ).toEqual(["n1", "n3", "n2"]);
  });

  it("leaves the input untouched", () => {
    const order = nodes.map((node) => node.id);
    sortNodes(nodes, { column: "name", direction: "desc" });
    expect(nodes.map((node) => node.id)).toEqual(order);
  });
});

describe("nextNodeSort", () => {
  it("cycles a column through ascending, descending and back to manual", () => {
    expect(nextNodeSort(null, "name")).toEqual({
      column: "name",
      direction: "asc",
    });
    expect(nextNodeSort({ column: "name", direction: "asc" }, "name")).toEqual({
      column: "name",
      direction: "desc",
    });
    expect(nextNodeSort({ column: "name", direction: "desc" }, "name")).toBeNull();
  });

  it("starts a different column ascending", () => {
    expect(nextNodeSort({ column: "name", direction: "desc" }, "latency")).toEqual({
      column: "latency",
      direction: "asc",
    });
  });
});

describe("column widths", () => {
  beforeEach(() => localStorage.clear());

  it("keeps a dragged width between launches", () => {
    saveColumnWidths({ name: 220 });

    expect(loadColumnWidths()).toEqual({ name: 220 });
  });

  it("ignores a stored value that is not a width table", () => {
    localStorage.setItem("mgclash.columnWidths", "not json");
    expect(loadColumnWidths()).toEqual({});

    localStorage.setItem("mgclash.columnWidths", '{"name":"wide"}');
    expect(loadColumnWidths()).toEqual({});
  });

  it("refuses to shrink a column into nothing", () => {
    // A column dragged to zero cannot be grabbed again.
    expect(clampColumnWidth(10)).toBe(48);
    expect(clampColumnWidth(3_000)).toBe(1_200);
    expect(clampColumnWidth(220)).toBe(220);
  });
});

describe("savedFontSize", () => {
  beforeEach(() => localStorage.clear());

  it("defaults to 13 when nothing is stored", () => {
    expect(savedFontSize()).toBe(13);
  });

  it("reads a stored size the picker offers", () => {
    localStorage.setItem("mgclash.fontSize", "16");
    expect(savedFontSize()).toBe(16);
  });

  it("ignores a size the picker does not offer", () => {
    localStorage.setItem("mgclash.fontSize", "11");
    expect(savedFontSize()).toBe(13);
  });
});
