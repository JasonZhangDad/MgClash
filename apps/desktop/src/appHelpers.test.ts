import { describe, expect, it } from "vitest";

import {
  groupTraffic,
  processTraffic,
  ruleDraftFromConnection,
} from "./appHelpers";

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
