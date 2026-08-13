import { describe, expect, it } from "vitest";

import {
  blankManualNodeForm,
  buildManualNodeDraft,
  emptyManualNodeForm,
  formFromManualNodeDraft,
  usesStreamTransport,
  type ManualNodeForm,
} from "./manualNode";

const USER_ID = "b0dd64e4-0fbd-4038-9139-d1f32a68a0dc";

describe("blankManualNodeForm", () => {
  it("applies TLS create defaults without changing the empty baseline", () => {
    expect(blankManualNodeForm()).toEqual(emptyManualNodeForm);
    expect(
      blankManualNodeForm({ allowInsecure: true, fingerprint: "chrome" }),
    ).toEqual({
      ...emptyManualNodeForm,
      allowInsecure: true,
      fingerprint: "chrome",
    });
  });
});

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
      pinnedSha256: null,
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

  it("builds an HTTPUpgrade transport", () => {
    const draft = draftOf({
      transport: "httpupgrade",
      wsHost: "cdn.example.com",
      wsPath: "/upgrade",
    });

    expect(draft.transport).toEqual({
      host: "cdn.example.com",
      path: "/upgrade",
      type: "httpupgrade",
    });
  });

  it("builds an XHTTP transport with its mode", () => {
    const draft = draftOf({
      transport: "xhttp",
      wsHost: "cdn.example.com",
      wsPath: "/xh",
      xhttpMode: "stream-one",
    });

    expect(draft.transport).toEqual({
      host: "cdn.example.com",
      mode: "stream-one",
      path: "/xh",
      type: "xhttp",
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
    expect(errorOf({ transport: "httpupgrade", wsPath: "" })).toBe(
      "请填写 HTTPUpgrade 路径",
    );
    expect(errorOf({ transport: "xhttp", wsPath: "" })).toBe(
      "请填写 XHTTP 路径",
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
  it("is false for Hysteria2 and TUIC", () => {
    expect(usesStreamTransport("vless")).toBe(true);
    expect(usesStreamTransport("vmess")).toBe(true);
    expect(usesStreamTransport("trojan")).toBe(true);
    expect(usesStreamTransport("shadowsocks")).toBe(true);
    expect(usesStreamTransport("hysteria2")).toBe(false);
    expect(usesStreamTransport("tuic")).toBe(false);
  });
});

describe("formFromManualNodeDraft", () => {
  it("round-trips a Reality VLESS draft into the form", () => {
    const form = formFromManualNodeDraft({
      credential: { flow: "xtls-rprx-vision", protocol: "vless", userId: USER_ID },
      name: "Reality Edge",
      port: 443,
      server: "edge.example.com",
      tls: {
        alpn: ["h2"],
        fingerprint: "chrome",
        publicKey: "abc",
        serverName: "www.example.com",
        shortId: "01",
        spiderX: "/",
        type: "reality",
      },
      transport: { type: "tcp" },
      udpEnabled: true,
    });

    expect(form.protocol).toBe("vless");
    expect(form.realityEnabled).toBe(true);
    expect(form.publicKey).toBe("abc");
    expect(form.serverName).toBe("www.example.com");
    expect(form.shortId).toBe("01");
    expect(form.spiderX).toBe("/");
    expect(form.flow).toBe("xtls-rprx-vision");
  });

  it("round-trips websocket Trojan and hysteria2 credentials", () => {
    const trojan = formFromManualNodeDraft({
      credential: { password: "hunter2", protocol: "trojan" },
      name: "Trojan",
      port: 443,
      server: "edge.example.com",
      tls: {
        allowInsecure: true,
        alpn: [],
        fingerprint: null,
        pinnedSha256: null,
        serverName: null,
        type: "tls",
      },
      transport: { host: "cdn.example.com", path: "/ws", type: "websocket" },
      udpEnabled: false,
    });
    expect(trojan.protocol).toBe("trojan");
    expect(trojan.password).toBe("hunter2");
    expect(trojan.transport).toBe("websocket");
    expect(trojan.wsPath).toBe("/ws");
    expect(trojan.tlsEnabled).toBe(true);
    expect(trojan.allowInsecure).toBe(true);

    const hy2 = formFromManualNodeDraft({
      credential: {
        authentication: "token",
        obfuscation: { method: "Salamander", password: "obfs" },
        protocol: "hysteria2",
      },
      name: "HY2",
      port: 443,
      server: "edge.example.com",
      tls: {
        allowInsecure: false,
        alpn: ["h3"],
        fingerprint: null,
        pinnedSha256: null,
        serverName: "edge.example.com",
        type: "tls",
      },
      transport: null,
      udpEnabled: true,
    });
    expect(hy2.protocol).toBe("hysteria2");
    expect(hy2.obfsEnabled).toBe(true);
    expect(hy2.obfsPassword).toBe("obfs");
    expect(hy2.authentication).toBe("token");
  });
});

describe("Reality TLS draft", () => {
  it("builds a Reality TLS payload", () => {
    const draft = draftOf({
      protocol: "vless",
      realityEnabled: true,
      tlsEnabled: true,
      serverName: "www.example.com",
      publicKey: "pubkey",
      shortId: "abcd",
      spiderX: "/",
      fingerprint: "chrome",
      alpn: "h2,http/1.1",
    });

    expect(draft.tls).toEqual({
      alpn: ["h2", "http/1.1"],
      fingerprint: "chrome",
      publicKey: "pubkey",
      serverName: "www.example.com",
      shortId: "abcd",
      spiderX: "/",
      type: "reality",
    });
  });

  it("rejects Reality without publicKey or SNI", () => {
    expect(
      errorOf({
        protocol: "vless",
        realityEnabled: true,
        serverName: "www.example.com",
        publicKey: "",
      }),
    ).toContain("publicKey");
    expect(
      errorOf({
        protocol: "vless",
        realityEnabled: true,
        serverName: "",
        publicKey: "pubkey",
      }),
    ).toContain("SNI");
  });
});

describe("formFromManualNodeDraft protocol coverage", () => {
  it("fills VMess, Shadowsocks, and gRPC fields", () => {
    const vmess = formFromManualNodeDraft({
      credential: {
        alterId: 2,
        protocol: "vmess",
        security: "Auto",
        userId: USER_ID,
      },
      name: "VMess",
      port: 443,
      server: "edge.example.com",
      tls: null,
      transport: {
        authority: "auth.example.com",
        mode: "multi",
        serviceName: "GunService",
        type: "grpc",
      },
      udpEnabled: true,
    });
    expect(vmess.protocol).toBe("vmess");
    expect(vmess.alterId).toBe("2");
    expect(vmess.transport).toBe("grpc");
    expect(vmess.grpcServiceName).toBe("GunService");
    expect(vmess.grpcMode).toBe("multi");
    expect(vmess.grpcAuthority).toBe("auth.example.com");

    const ss = formFromManualNodeDraft({
      credential: {
        method: "aes-256-gcm",
        password: "secret",
        protocol: "shadowsocks",
      },
      name: "SS",
      port: 8388,
      server: "edge.example.com",
      tls: null,
      transport: { type: "tcp" },
      udpEnabled: true,
    });
    expect(ss.protocol).toBe("shadowsocks");
    expect(ss.method).toBe("aes-256-gcm");
    expect(ss.password).toBe("secret");
  });
});
