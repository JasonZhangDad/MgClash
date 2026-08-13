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
  it("rejects invalid Xray finalmask JSON", () => {
    expect(
      errorOf({ xrayFinalmaskJson: "{not-json" }),
    ).toBe("Xray finalmask JSON 格式无效");
  });

  it("passes through valid Xray finalmask JSON", () => {
    const json =
      '{"type":"fragment","settings":{"packets":"tlshello","lengths":["80-120"]}}';
    const draft = draftOf({ xrayFinalmaskJson: json });
    expect(draft.xrayFinalmaskJson).toBe(json);
  });

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

  it("builds SOCKS and HTTP drafts with optional auth", () => {
    expect(
      draftOf({
        password: "secret",
        protocol: "socks",
        username: "alice",
      }).credential,
    ).toEqual({
      password: "secret",
      protocol: "socks",
      username: "alice",
    });
    expect(
      draftOf({
        protocol: "http",
        tlsEnabled: true,
        username: "",
      }),
    ).toMatchObject({
      credential: { password: null, protocol: "http", username: null },
      tls: expect.objectContaining({ type: "tls" }),
      transport: { type: "tcp" },
    });
  });

  it("rejects a proxy password without a username", () => {
    expect(errorOf({ password: "secret", protocol: "socks" })).toBe(
      "填写密码时必须同时填写用户名",
    );
    expect(errorOf({ password: "secret", protocol: "http" })).toBe(
      "填写密码时必须同时填写用户名",
    );
  });

  it("builds a WireGuard draft with optional fields", () => {
    const draft = draftOf({
      localAddress: "10.0.0.2/32, fd00::1/128",
      mtu: "1420",
      peerPublicKey: "peer-pub",
      preSharedKey: "psk",
      privateKey: "priv",
      protocol: "wireguard",
      reserved: "1,2,3",
    });

    expect(draft.transport).toBeNull();
    expect(draft.tls).toBeNull();
    expect(draft.credential).toEqual({
      localAddress: ["10.0.0.2/32", "fd00::1/128"],
      mtu: 1420,
      peerPublicKey: "peer-pub",
      preSharedKey: "psk",
      privateKey: "priv",
      protocol: "wireguard",
      reserved: [1, 2, 3],
    });
  });

  it("builds a minimal WireGuard draft without optional fields", () => {
    const draft = draftOf({
      localAddress: "10.0.0.2/32",
      peerPublicKey: "peer-pub",
      privateKey: "priv",
      protocol: "wireguard",
    });

    expect(draft.credential).toEqual({
      localAddress: ["10.0.0.2/32"],
      mtu: null,
      peerPublicKey: "peer-pub",
      preSharedKey: null,
      privateKey: "priv",
      protocol: "wireguard",
      reserved: null,
    });
  });

  it("rejects incomplete or invalid WireGuard fields", () => {
    expect(
      errorOf({ peerPublicKey: "pub", privateKey: "", protocol: "wireguard" }),
    ).toBe("请填写 WireGuard 私钥");
    expect(
      errorOf({ peerPublicKey: "", privateKey: "priv", protocol: "wireguard" }),
    ).toBe("请填写 WireGuard 对端公钥");
    expect(
      errorOf({
        localAddress: " , ",
        peerPublicKey: "pub",
        privateKey: "priv",
        protocol: "wireguard",
      }),
    ).toBe("请填写至少一个本地地址");
    expect(
      errorOf({
        localAddress: "10.0.0.2/32",
        mtu: "-1",
        peerPublicKey: "pub",
        privateKey: "priv",
        protocol: "wireguard",
      }),
    ).toBe("MTU 必须是不小于 0 的整数");
    expect(
      errorOf({
        localAddress: "10.0.0.2/32",
        peerPublicKey: "pub",
        privateKey: "priv",
        protocol: "wireguard",
        reserved: "1,2",
      }),
    ).toBe("reserved 必须是 3 个 0-255 的整数，以逗号分隔");
    expect(
      errorOf({
        localAddress: "10.0.0.2/32",
        peerPublicKey: "pub",
        privateKey: "priv",
        protocol: "wireguard",
        reserved: "1,2,300",
      }),
    ).toBe("reserved 必须是 3 个 0-255 的整数，以逗号分隔");
  });

  it("builds an AnyTLS draft with required TLS", () => {
    const draft = draftOf({
      password: "hunter2",
      protocol: "anytls",
      tlsEnabled: true,
    });

    expect(draft.transport).toBeNull();
    expect(draft.tls).toEqual(
      expect.objectContaining({ type: "tls" }),
    );
    expect(draft.credential).toEqual({
      password: "hunter2",
      protocol: "anytls",
    });
  });

  it("rejects an empty AnyTLS password", () => {
    expect(errorOf({ password: " ", protocol: "anytls", tlsEnabled: true })).toBe(
      "请填写 AnyTLS 密码",
    );
  });

  it("builds an mKCP transport draft", () => {
    const draft = draftOf({
      kcpCongestion: true,
      kcpHeaderType: "srtp",
      kcpMtu: "1350",
      kcpSeed: "secret",
      kcpTti: "50",
      password: "hunter2",
      protocol: "trojan",
      tlsEnabled: true,
      transport: "kcp",
    });

    expect(draft.transport).toEqual({
      congestion: true,
      downlinkCapacity: null,
      headerType: "srtp",
      mtu: 1350,
      seed: "secret",
      tti: 50,
      type: "kcp",
      uplinkCapacity: null,
    });
  });

  it("builds a Naive draft with SNI-only TLS and optional QUIC", () => {
    const draft = draftOf({
      password: "hunter2",
      protocol: "naive",
      quic: true,
      quicCongestionControl: "bbr2",
      serverName: "cdn.example.com",
      tlsEnabled: true,
      username: "alice",
    });

    expect(draft.transport).toBeNull();
    expect(draft.tls).toEqual({
      allowInsecure: false,
      alpn: [],
      fingerprint: null,
      pinnedSha256: null,
      serverName: "cdn.example.com",
      type: "tls",
    });
    expect(draft.credential).toEqual({
      password: "hunter2",
      protocol: "naive",
      quic: true,
      quicCongestionControl: "bbr2",
      username: "alice",
    });
  });

  it("rejects Naive Reality and TLS extras", () => {
    expect(
      errorOf({
        protocol: "naive",
        realityEnabled: true,
        serverName: "cdn.example.com",
        publicKey: "key",
        tlsEnabled: true,
      }),
    ).toBe("Naive 不支持 Reality");
    expect(
      errorOf({
        allowInsecure: true,
        protocol: "naive",
        tlsEnabled: true,
      }),
    ).toBe("Naive 不支持跳过证书校验");
  });

  it("builds a custom Core JSON node with placeholder endpoint", () => {
    const document = '{"inbounds":[],"outbounds":[{"type":"direct"}]}';
    const draft = draftOf({
      customCore: "xray",
      customDocument: document,
      name: "My JSON",
      protocol: "custom",
      server: "ignored.example.com",
      port: "8443",
    });

    expect(draft.server).toBe("127.0.0.1");
    expect(draft.port).toBe(443);
    expect(draft.transport).toBeNull();
    expect(draft.tls).toBeNull();
    expect(draft.credential).toEqual({
      core: "xray",
      document,
      protocol: "custom",
    });
  });

  it("rejects empty or invalid custom JSON", () => {
    expect(
      errorOf({
        customDocument: " ",
        name: "Empty",
        protocol: "custom",
      }),
    ).toBe("请填写完整的 Core JSON 配置");
    expect(
      errorOf({
        customDocument: "{bad",
        name: "Bad",
        protocol: "custom",
      }),
    ).toBe("Core JSON 格式无效");
    expect(
      errorOf({
        customDocument: "[]",
        name: "Array",
        protocol: "custom",
      }),
    ).toBe("Core JSON 必须是 JSON 对象");
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
    expect(errorOf({ name: "  " })).toBe("请填写节点名称");
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
  it("is false for Hysteria2, TUIC, and WireGuard", () => {
    expect(usesStreamTransport("vless")).toBe(true);
    expect(usesStreamTransport("vmess")).toBe(true);
    expect(usesStreamTransport("trojan")).toBe(true);
    expect(usesStreamTransport("shadowsocks")).toBe(true);
    expect(usesStreamTransport("hysteria2")).toBe(false);
    expect(usesStreamTransport("tuic")).toBe(false);
    expect(usesStreamTransport("wireguard")).toBe(false);
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

  it("round-trips a WireGuard credential", () => {
    const form = formFromManualNodeDraft({
      credential: {
        localAddress: ["10.0.0.2/32"],
        mtu: 1420,
        peerPublicKey: "peer-pub",
        preSharedKey: "psk",
        privateKey: "priv",
        protocol: "wireguard",
        reserved: [1, 2, 3],
      },
      name: "WG",
      port: 51820,
      server: "edge.example.com",
      tls: null,
      transport: null,
      udpEnabled: true,
    });

    expect(form.protocol).toBe("wireguard");
    expect(form.privateKey).toBe("priv");
    expect(form.peerPublicKey).toBe("peer-pub");
    expect(form.preSharedKey).toBe("psk");
    expect(form.localAddress).toBe("10.0.0.2/32");
    expect(form.mtu).toBe("1420");
    expect(form.reserved).toBe("1,2,3");
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

describe("round-tripping every protocol through the form", () => {
  /// A draft the window can edit has to survive the trip back into the form:
  /// a missing switch arm silently drops the credential the user typed.
  it("keeps TUIC, SOCKS, HTTP, AnyTLS, Naive and custom credentials", () => {
    const tuic = formFromManualNodeDraft(
      draftOf({
        protocol: "tuic",
        password: "tuic-secret",
        congestionControl: "bbr",
        udpRelayMode: "quic",
        zeroRttHandshake: true,
      }),
    );
    expect(tuic.protocol).toBe("tuic");
    expect(tuic.userId).toBe(USER_ID);
    expect(tuic.password).toBe("tuic-secret");
    expect(tuic.congestionControl).toBe("bbr");
    expect(tuic.udpRelayMode).toBe("quic");
    expect(tuic.zeroRttHandshake).toBe(true);

    const socks = formFromManualNodeDraft(
      draftOf({ protocol: "socks", username: "user", password: "pass" }),
    );
    expect(socks.protocol).toBe("socks");
    expect(socks.username).toBe("user");
    expect(socks.password).toBe("pass");

    const http = formFromManualNodeDraft(
      draftOf({ protocol: "http", username: "user", password: "pass" }),
    );
    expect(http.protocol).toBe("http");
    expect(http.username).toBe("user");

    const anytls = formFromManualNodeDraft(
      draftOf({
        protocol: "anytls",
        password: "anytls-secret",
        tlsEnabled: true,
        serverName: "edge.example.com",
      }),
    );
    expect(anytls.protocol).toBe("anytls");
    expect(anytls.password).toBe("anytls-secret");

    const naive = formFromManualNodeDraft(
      draftOf({
        protocol: "naive",
        username: "user",
        password: "pass",
        quic: true,
        quicCongestionControl: "bbr",
        tlsEnabled: true,
        serverName: "edge.example.com",
      }),
    );
    expect(naive.protocol).toBe("naive");
    expect(naive.username).toBe("user");
    expect(naive.quic).toBe(true);
    expect(naive.quicCongestionControl).toBe("bbr");

    const custom = formFromManualNodeDraft(
      draftOf({
        protocol: "custom",
        customCore: "xray",
        customDocument: '{"outbounds":[]}',
      }),
    );
    expect(custom.protocol).toBe("custom");
    expect(custom.customCore).toBe("xray");
    expect(custom.customDocument).toBe('{"outbounds":[]}');
  });

  it("keeps an mKCP transport and a pinned TLS certificate", () => {
    const kcp = formFromManualNodeDraft(
      draftOf({
        transport: "kcp",
        kcpSeed: "seed",
        kcpHeaderType: "wechat-video",
        kcpMtu: "1350",
        kcpTti: "20",
        kcpUplinkCapacity: "5",
        kcpDownlinkCapacity: "20",
        kcpCongestion: true,
      }),
    );
    expect(kcp.transport).toBe("kcp");
    expect(kcp.kcpSeed).toBe("seed");
    expect(kcp.kcpMtu).toBe("1350");
    expect(kcp.kcpCongestion).toBe(true);

    const pinned = formFromManualNodeDraft(
      draftOf({
        tlsEnabled: true,
        serverName: "edge.example.com",
        alpn: "h2",
        pinnedSha256: "a".repeat(64),
      }),
    );
    expect(pinned.pinnedSha256).toBe("a".repeat(64));
    expect(pinned.alpn).toBe("h2");
  });
});

describe("the validation paths the window relies on", () => {
  function errorOf(overrides: Partial<ManualNodeForm>): string {
    const result = buildManualNodeDraft(form(overrides));
    if (!("error" in result)) {
      throw new Error("expected a validation error");
    }
    return result.error;
  }

  it("rejects a TUIC node that is neither a UUID nor a single UDP mode", () => {
    expect(errorOf({ protocol: "tuic", userId: "not-a-uuid" })).toContain(
      "UUID",
    );
    expect(
      errorOf({
        protocol: "tuic",
        udpRelayMode: "quic",
        udpOverStream: true,
      }),
    ).toContain("udp_relay_mode");
  });

  it("rejects a Naive password without a username", () => {
    expect(
      errorOf({
        protocol: "naive",
        password: "pass",
        tlsEnabled: true,
        serverName: "edge.example.com",
      }),
    ).toContain("用户名");
  });

  it("rejects every non-integer mKCP number", () => {
    for (const field of [
      "kcpMtu",
      "kcpTti",
      "kcpUplinkCapacity",
      "kcpDownlinkCapacity",
    ] as const) {
      expect(errorOf({ transport: "kcp", [field]: "-1" })).toContain("mKCP");
    }
  });

  it("rejects the TLS options Naive cannot carry", () => {
    const naive = {
      protocol: "naive" as const,
      username: "user",
      password: "pass",
      tlsEnabled: true,
      serverName: "edge.example.com",
    };
    expect(errorOf({ ...naive, allowInsecure: true })).toContain("证书校验");
    expect(errorOf({ ...naive, alpn: "h2" })).toContain("ALPN");
    expect(errorOf({ ...naive, fingerprint: "chrome" })).toContain("指纹");
    expect(errorOf({ ...naive, pinnedSha256: "a".repeat(64) })).toContain(
      "证书固定",
    );
  });

  it("rejects Reality on an HTTP proxy", () => {
    expect(
      errorOf({
        protocol: "http",
        realityEnabled: true,
        serverName: "edge.example.com",
        publicKey: "key",
      }),
    ).toContain("HTTP");
  });

  it("rejects finalmask JSON that is not a mask entry", () => {
    expect(errorOf({ xrayFinalmaskJson: "[]" })).toContain("JSON 对象");
    expect(errorOf({ xrayFinalmaskJson: '{"other":1}' })).toContain("mask");
  });
});
