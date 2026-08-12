import { describe, expect, it } from "vitest";

import {
  buildManualNodeDraft,
  emptyManualNodeForm,
  usesStreamTransport,
  type ManualNodeForm,
} from "./manualNode";

const USER_ID = "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc";

function form(overrides: Partial<ManualNodeForm> = {}): ManualNodeForm {
  return {
    ...emptyManualNodeForm,
    name: "Tokyo",
    port: "443",
    server: "edge.example.com",
    userId: USER_ID,
    ...overrides,
  };
}

function draftOf(overrides: Partial<ManualNodeForm> = {}) {
  const result = buildManualNodeDraft(form(overrides));
  if ("error" in result) {
    throw new Error(`expected a draft, got: ${result.error}`);
  }
  return result.draft;
}

function errorOf(overrides: Partial<ManualNodeForm> = {}): string {
  const result = buildManualNodeDraft(form(overrides));
  if ("draft" in result) {
    throw new Error("expected an error");
  }
  return result.error;
}

describe("buildManualNodeDraft", () => {
  it("builds a VLESS draft with TCP transport by default", () => {
    const draft = draftOf();

    expect(draft.name).toBe("Tokyo");
    expect(draft.server).toBe("edge.example.com");
    expect(draft.port).toBe(443);
    expect(draft.udpEnabled).toBe(true);
    expect(draft.transport).toEqual({ type: "tcp" });
    expect(draft.tls).toBeNull();
    expect(draft.credential).toEqual({
      flow: null,
      protocol: "vless",
      userId: USER_ID,
    });
  });

  it("trims the shared fields", () => {
    const draft = draftOf({ name: "  Tokyo  ", server: "  edge.example.com " });

    expect(draft.name).toBe("Tokyo");
    expect(draft.server).toBe("edge.example.com");
  });

  it("builds a VMess draft", () => {
    const draft = draftOf({
      alterId: "4",
      protocol: "vmess",
      security: "Aes128Gcm",
    });

    expect(draft.credential).toEqual({
      alterId: 4,
      protocol: "vmess",
      security: "Aes128Gcm",
      userId: USER_ID,
    });
  });

  it("builds a Trojan draft", () => {
    const draft = draftOf({ password: "hunter2", protocol: "trojan" });

    expect(draft.credential).toEqual({
      password: "hunter2",
      protocol: "trojan",
    });
  });

  it("builds a Shadowsocks draft", () => {
    const draft = draftOf({
      method: "aes-256-gcm",
      password: "hunter2",
      protocol: "shadowsocks",
    });

    expect(draft.credential).toEqual({
      method: "aes-256-gcm",
      password: "hunter2",
      protocol: "shadowsocks",
    });
  });

  it("always sends TLS and no transport for Hysteria2", () => {
    const draft = draftOf({
      authentication: "token",
      protocol: "hysteria2",
      serverName: "edge.example.com",
      tlsEnabled: false,
    });

    expect(draft.transport).toBeNull();
    expect(draft.tls).toEqual({
      allowInsecure: false,
      alpn: [],
      fingerprint: null,
      serverName: "edge.example.com",
      type: "tls",
    });
    expect(draft.credential).toEqual({
      authentication: "token",
      obfuscation: null,
      protocol: "hysteria2",
    });
  });

  it("includes Hysteria2 obfuscation when enabled", () => {
    const draft = draftOf({
      obfsEnabled: true,
      obfsMethod: "Gecko",
      obfsPassword: "obfs-secret",
      protocol: "hysteria2",
    });

    expect(draft.credential).toEqual({
      authentication: null,
      obfuscation: { method: "Gecko", password: "obfs-secret" },
      protocol: "hysteria2",
    });
  });

  it("builds a WebSocket transport", () => {
    const draft = draftOf({
      transport: "websocket",
      wsHost: "cdn.example.com",
      wsPath: "/ray",
    });

    expect(draft.transport).toEqual({
      host: "cdn.example.com",
      path: "/ray",
      type: "websocket",
    });
  });

  it("builds a gRPC transport", () => {
    const draft = draftOf({
      grpcMode: "multi",
      grpcServiceName: "tunnel",
      transport: "grpc",
    });

    expect(draft.transport).toEqual({
      authority: null,
      mode: "multi",
      serviceName: "tunnel",
      type: "grpc",
    });
  });

  it("splits the ALPN list and drops blank entries", () => {
    const draft = draftOf({ alpn: "h2, http/1.1 , ", tlsEnabled: true });

    expect(draft.tls?.alpn).toEqual(["h2", "http/1.1"]);
  });

  it("treats blank optional fields as absent", () => {
    const draft = draftOf({
      fingerprint: "  ",
      flow: "   ",
      serverName: "",
      tlsEnabled: true,
    });

    expect(draft.credential).toEqual({
      flow: null,
      protocol: "vless",
      userId: USER_ID,
    });
    expect(draft.tls?.fingerprint).toBeNull();
    expect(draft.tls?.serverName).toBeNull();
  });

  it("rejects a missing name or server", () => {
    expect(errorOf({ name: "  " })).toBe("请填写节点名称和服务器");
    expect(errorOf({ server: "" })).toBe("请填写节点名称和服务器");
  });

  it("rejects a port outside 1-65535", () => {
    const message = "节点端口必须是 1 到 65535 的整数";
    expect(errorOf({ port: "0" })).toBe(message);
    expect(errorOf({ port: "65536" })).toBe(message);
    expect(errorOf({ port: "443.5" })).toBe(message);
    expect(errorOf({ port: "abc" })).toBe(message);
  });

  it("rejects a malformed UUID", () => {
    expect(errorOf({ userId: "not-a-uuid" })).toBe("请填写合法的 UUID");
    expect(errorOf({ protocol: "vmess", userId: "" })).toBe(
      "请填写合法的 UUID",
    );
  });

  it("rejects a negative or fractional alterId", () => {
    const message = "alterId 必须是不小于 0 的整数";
    expect(errorOf({ alterId: "-1", protocol: "vmess" })).toBe(message);
    expect(errorOf({ alterId: "1.5", protocol: "vmess" })).toBe(message);
  });

  it("rejects missing protocol secrets", () => {
    expect(errorOf({ password: " ", protocol: "trojan" })).toBe(
      "请填写 Trojan 密码",
    );
    expect(errorOf({ password: "", protocol: "shadowsocks" })).toBe(
      "请填写 Shadowsocks 密码",
    );
    expect(errorOf({ method: " ", protocol: "shadowsocks" })).toBe(
      "请选择 Shadowsocks 加密方式",
    );
  });

  it("rejects an incomplete transport", () => {
    expect(errorOf({ transport: "websocket", wsPath: " " })).toBe(
      "请填写 WebSocket 路径",
    );
    expect(errorOf({ grpcServiceName: "", transport: "grpc" })).toBe(
      "请填写 gRPC serviceName",
    );
  });

  it("rejects obfuscation without a password", () => {
    expect(
      errorOf({ obfsEnabled: true, obfsPassword: " ", protocol: "hysteria2" }),
    ).toBe("启用混淆时必须填写混淆密码");
  });
});

describe("usesStreamTransport", () => {
  it("is false only for Hysteria2", () => {
    expect(usesStreamTransport("vless")).toBe(true);
    expect(usesStreamTransport("vmess")).toBe(true);
    expect(usesStreamTransport("trojan")).toBe(true);
    expect(usesStreamTransport("shadowsocks")).toBe(true);
    expect(usesStreamTransport("hysteria2")).toBe(false);
  });
});
