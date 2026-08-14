import { describe, expect, it } from "vitest";

import { ruleDraftFromConnection } from "./appHelpers";

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
