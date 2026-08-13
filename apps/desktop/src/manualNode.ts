import type {
  ManualCredentialDraft,
  ManualNodeDraft,
  ObfuscationMethod,
  ProxyProtocol,
  TlsDraft,
  TransportDraft,
  VmessSecurity,
} from "./session";

export type TransportKind =
  | "tcp"
  | "websocket"
  | "httpupgrade"
  | "xhttp"
  | "grpc"
  | "kcp";

export type GrpcMode = "gun" | "multi" | "guna";

export type XhttpMode = "auto" | "packet-up" | "stream-up" | "stream-one";

/** mKCP obfuscation header types Xray recognizes; `""` means unset/default. */
export type KcpHeaderType =
  | ""
  | "none"
  | "srtp"
  | "utp"
  | "wechat-video"
  | "dtls"
  | "wireguard";

/** Every field the manual creation form can collect, as raw strings. */
export interface ManualNodeForm {
  alterId: string;
  allowInsecure: boolean;
  alpn: string;
  authentication: string;
  congestionControl: "" | "cubic" | "new_reno" | "bbr";
  fingerprint: string;
  flow: string;
  grpcAuthority: string;
  grpcMode: GrpcMode;
  grpcServiceName: string;
  kcpCongestion: boolean;
  kcpDownlinkCapacity: string;
  kcpHeaderType: KcpHeaderType;
  kcpMtu: string;
  kcpSeed: string;
  kcpTti: string;
  kcpUplinkCapacity: string;
  localAddress: string;
  method: string;
  mtu: string;
  name: string;
  obfsEnabled: boolean;
  obfsMethod: ObfuscationMethod;
  obfsPassword: string;
  password: string;
  peerPublicKey: string;
  pinnedSha256: string;
  port: string;
  preSharedKey: string;
  privateKey: string;
  protocol: ProxyProtocol;
  publicKey: string;
  quic: boolean;
  quicCongestionControl: "" | "bbr" | "bbr2" | "cubic" | "reno";
  realityEnabled: boolean;
  reserved: string;
  security: VmessSecurity;
  server: string;
  serverName: string;
  shortId: string;
  spiderX: string;
  tlsEnabled: boolean;
  transport: TransportKind;
  udpEnabled: boolean;
  udpOverStream: boolean;
  udpRelayMode: "" | "native" | "quic";
  userId: string;
  username: string;
  wsHost: string;
  wsPath: string;
  xhttpMode: XhttpMode;
  zeroRttHandshake: boolean;
}

export const emptyManualNodeForm: ManualNodeForm = {
  alterId: "0",
  allowInsecure: false,
  alpn: "",
  authentication: "",
  congestionControl: "",
  fingerprint: "",
  flow: "",
  grpcAuthority: "",
  grpcMode: "gun",
  grpcServiceName: "",
  kcpCongestion: false,
  kcpDownlinkCapacity: "",
  kcpHeaderType: "",
  kcpMtu: "",
  kcpSeed: "",
  kcpTti: "",
  kcpUplinkCapacity: "",
  localAddress: "",
  method: "aes-256-gcm",
  mtu: "",
  name: "",
  obfsEnabled: false,
  obfsMethod: "Salamander",
  obfsPassword: "",
  password: "",
  peerPublicKey: "",
  pinnedSha256: "",
  port: "",
  preSharedKey: "",
  privateKey: "",
  protocol: "vless",
  publicKey: "",
  quic: false,
  quicCongestionControl: "",
  realityEnabled: false,
  reserved: "",
  security: "Auto",
  server: "",
  serverName: "",
  shortId: "",
  spiderX: "",
  tlsEnabled: false,
  transport: "tcp",
  udpEnabled: true,
  udpOverStream: false,
  udpRelayMode: "",
  userId: "",
  username: "",
  wsHost: "",
  wsPath: "",
  xhttpMode: "auto",
  zeroRttHandshake: false,
};

/** Seeds a blank form, optionally applying saved create-node TLS defaults. */
export function blankManualNodeForm(defaults?: {
  allowInsecure?: boolean;
  fingerprint?: string;
}): ManualNodeForm {
  return {
    ...emptyManualNodeForm,
    allowInsecure: defaults?.allowInsecure ?? false,
    fingerprint: defaults?.fingerprint ?? "",
  };
}

export type ManualNodeDraftResult =
  | { draft: ManualNodeDraft }
  | { error: string };

/** Mirrors the cipher allowlist the Rust draft validates against. */
export const SHADOWSOCKS_METHODS = [
  "2022-blake3-aes-128-gcm",
  "2022-blake3-aes-256-gcm",
  "2022-blake3-chacha20-poly1305",
  "aes-128-gcm",
  "aes-192-gcm",
  "aes-256-gcm",
  "chacha20-poly1305",
  "chacha20-ietf-poly1305",
  "xchacha20-poly1305",
  "xchacha20-ietf-poly1305",
  "aes-128-ctr",
  "aes-192-ctr",
  "aes-256-ctr",
  "aes-128-cfb",
  "aes-192-cfb",
  "aes-256-cfb",
  "rc4-md5",
  "chacha20-ietf",
  "xchacha20",
  "none",
  "plain",
] as const;

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu;

/**
 * Hysteria2 / TUIC / WireGuard / AnyTLS / Naive carry their own tunnel and do
 * not use a selectable stream transport.
 */
export function usesStreamTransport(protocol: ProxyProtocol): boolean {
  return (
    protocol !== "hysteria2" &&
    protocol !== "tuic" &&
    protocol !== "wireguard" &&
    protocol !== "anytls" &&
    protocol !== "naive"
  );
}

function optional(value: string): string | null {
  const trimmed = value.trim();
  return trimmed === "" ? null : trimmed;
}

function buildCredential(
  form: ManualNodeForm,
): ManualCredentialDraft | { error: string } {
  switch (form.protocol) {
    case "vless": {
      if (!UUID_PATTERN.test(form.userId.trim())) {
        return { error: "请填写合法的 UUID" };
      }
      return {
        flow: optional(form.flow),
        protocol: "vless",
        userId: form.userId.trim(),
      };
    }
    case "vmess": {
      if (!UUID_PATTERN.test(form.userId.trim())) {
        return { error: "请填写合法的 UUID" };
      }
      const alterId = Number(form.alterId);
      if (!Number.isInteger(alterId) || alterId < 0) {
        return { error: "alterId 必须是不小于 0 的整数" };
      }
      return {
        alterId,
        protocol: "vmess",
        security: form.security,
        userId: form.userId.trim(),
      };
    }
    case "trojan": {
      if (form.password.trim() === "") {
        return { error: "请填写 Trojan 密码" };
      }
      return { password: form.password.trim(), protocol: "trojan" };
    }
    case "shadowsocks": {
      if (form.method.trim() === "") {
        return { error: "请选择 Shadowsocks 加密方式" };
      }
      if (form.password.trim() === "") {
        return { error: "请填写 Shadowsocks 密码" };
      }
      return {
        method: form.method.trim(),
        password: form.password.trim(),
        protocol: "shadowsocks",
      };
    }
    case "hysteria2": {
      if (form.obfsEnabled && form.obfsPassword.trim() === "") {
        return { error: "启用混淆时必须填写混淆密码" };
      }
      return {
        authentication: optional(form.authentication),
        obfuscation: form.obfsEnabled
          ? {
              method: form.obfsMethod,
              password: form.obfsPassword.trim(),
            }
          : null,
        protocol: "hysteria2",
      };
    }
    case "tuic": {
      if (!UUID_PATTERN.test(form.userId.trim())) {
        return { error: "请填写合法的 UUID" };
      }
      if (form.udpRelayMode !== "" && form.udpOverStream) {
        return { error: "不能同时设置 udp_relay_mode 与 udp_over_stream" };
      }
      return {
        congestionControl:
          form.congestionControl === "" ? null : form.congestionControl,
        password: optional(form.password),
        protocol: "tuic",
        udpOverStream: form.udpOverStream,
        udpRelayMode: form.udpRelayMode === "" ? null : form.udpRelayMode,
        uuid: form.userId.trim(),
        zeroRttHandshake: form.zeroRttHandshake,
      };
    }
    case "socks":
    case "http": {
      const username = optional(form.username);
      const password = optional(form.password);
      if (password !== null && username === null) {
        return { error: "填写密码时必须同时填写用户名" };
      }
      return {
        password,
        protocol: form.protocol,
        username,
      };
    }
    case "wireguard": {
      const privateKey = form.privateKey.trim();
      if (privateKey === "") {
        return { error: "请填写 WireGuard 私钥" };
      }
      const peerPublicKey = form.peerPublicKey.trim();
      if (peerPublicKey === "") {
        return { error: "请填写 WireGuard 对端公钥" };
      }
      const localAddress = form.localAddress
        .split(",")
        .map((entry) => entry.trim())
        .filter((entry) => entry !== "");
      if (localAddress.length === 0) {
        return { error: "请填写至少一个本地地址" };
      }
      let mtu: number | null = null;
      if (form.mtu.trim() !== "") {
        mtu = Number(form.mtu);
        if (!Number.isInteger(mtu) || mtu < 0) {
          return { error: "MTU 必须是不小于 0 的整数" };
        }
      }
      const reserved = parseReserved(form.reserved);
      if (hasError(reserved)) {
        return reserved;
      }
      return {
        localAddress,
        mtu,
        peerPublicKey,
        preSharedKey: optional(form.preSharedKey),
        privateKey,
        protocol: "wireguard",
        reserved,
      };
    }
    case "anytls": {
      if (form.password.trim() === "") {
        return { error: "请填写 AnyTLS 密码" };
      }
      return { password: form.password.trim(), protocol: "anytls" };
    }
    case "naive": {
      const username = optional(form.username);
      const password = optional(form.password);
      if (password !== null && username === null) {
        return { error: "填写密码时必须同时填写用户名" };
      }
      return {
        password,
        protocol: "naive",
        quic: form.quic,
        quicCongestionControl:
          form.quic && form.quicCongestionControl !== ""
            ? form.quicCongestionControl
            : null,
        username,
      };
    }
  }
}

/** Parses the `1,2,3` reserved-bytes field, or `null` when left blank. */
function parseReserved(
  value: string,
): [number, number, number] | null | { error: string } {
  if (value.trim() === "") {
    return null;
  }
  const parts = value.split(",").map((entry) => Number(entry.trim()));
  if (
    parts.length !== 3 ||
    parts.some((entry) => !Number.isInteger(entry) || entry < 0 || entry > 255)
  ) {
    return { error: "reserved 必须是 3 个 0-255 的整数，以逗号分隔" };
  }
  return [parts[0], parts[1], parts[2]];
}

function buildTransport(
  form: ManualNodeForm,
): TransportDraft | null | { error: string } {
  if (!usesStreamTransport(form.protocol)) {
    return null;
  }
  // Shadowsocks / SOCKS / HTTP accept nothing but plain TCP, so a transport
  // left over from a previously selected protocol must not leak into the payload.
  if (
    form.protocol === "shadowsocks" ||
    form.protocol === "socks" ||
    form.protocol === "http"
  ) {
    return { type: "tcp" };
  }
  switch (form.transport) {
    case "tcp":
      return { type: "tcp" };
    case "websocket": {
      if (form.wsPath.trim() === "") {
        return { error: "请填写 WebSocket 路径" };
      }
      return {
        host: optional(form.wsHost),
        path: form.wsPath.trim(),
        type: "websocket",
      };
    }
    case "httpupgrade": {
      if (form.wsPath.trim() === "") {
        return { error: "请填写 HTTPUpgrade 路径" };
      }
      return {
        host: optional(form.wsHost),
        path: form.wsPath.trim(),
        type: "httpupgrade",
      };
    }
    case "xhttp": {
      if (form.wsPath.trim() === "") {
        return { error: "请填写 XHTTP 路径" };
      }
      return {
        host: optional(form.wsHost),
        mode: form.xhttpMode,
        path: form.wsPath.trim(),
        type: "xhttp",
      };
    }
    case "grpc": {
      if (form.grpcServiceName.trim() === "") {
        return { error: "请填写 gRPC serviceName" };
      }
      return {
        authority: optional(form.grpcAuthority),
        mode: form.grpcMode,
        serviceName: form.grpcServiceName.trim(),
        type: "grpc",
      };
    }
    case "kcp": {
      const mtu = parseOptionalUint(form.kcpMtu);
      if (hasError(mtu)) {
        return { error: "mKCP mtu 必须是不小于 0 的整数" };
      }
      const tti = parseOptionalUint(form.kcpTti);
      if (hasError(tti)) {
        return { error: "mKCP tti 必须是不小于 0 的整数" };
      }
      const uplinkCapacity = parseOptionalUint(form.kcpUplinkCapacity);
      if (hasError(uplinkCapacity)) {
        return { error: "mKCP uplinkCapacity 必须是不小于 0 的整数" };
      }
      const downlinkCapacity = parseOptionalUint(form.kcpDownlinkCapacity);
      if (hasError(downlinkCapacity)) {
        return { error: "mKCP downlinkCapacity 必须是不小于 0 的整数" };
      }
      return {
        congestion: form.kcpCongestion,
        downlinkCapacity,
        headerType: form.kcpHeaderType === "" ? null : form.kcpHeaderType,
        mtu,
        seed: optional(form.kcpSeed),
        tti,
        type: "kcp",
        uplinkCapacity,
      };
    }
  }
}

/** Parses an optional non-negative integer field, or an error marker. */
function parseOptionalUint(value: string): number | null | { error: string } {
  if (value.trim() === "") {
    return null;
  }
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 0) {
    return { error: "invalid" };
  }
  return parsed;
}

function buildTls(form: ManualNodeForm): TlsDraft | null | { error: string } {
  // Shadowsocks and SOCKS outbounds have no TLS layer at all, and WireGuard
  // authenticates peers by key instead of certificate.
  if (
    form.protocol === "shadowsocks" ||
    form.protocol === "socks" ||
    form.protocol === "wireguard"
  ) {
    return null;
  }
  if (form.protocol === "naive") {
    if (form.realityEnabled) {
      return { error: "Naive 不支持 Reality" };
    }
    if (form.allowInsecure) {
      return { error: "Naive 不支持跳过证书校验" };
    }
    if (form.alpn.trim() !== "") {
      return { error: "Naive 不支持 ALPN" };
    }
    if (form.fingerprint.trim() !== "") {
      return { error: "Naive 不支持 TLS 指纹" };
    }
    if (form.pinnedSha256.trim() !== "") {
      return { error: "Naive 不支持证书固定" };
    }
    return {
      allowInsecure: false,
      alpn: [],
      fingerprint: null,
      pinnedSha256: null,
      serverName: optional(form.serverName),
      type: "tls",
    };
  }
  if (form.realityEnabled) {
    if (form.protocol === "http") {
      return { error: "HTTP 代理不支持 Reality" };
    }
    if (form.serverName.trim() === "") {
      return { error: "Reality 需要填写 SNI / serverName" };
    }
    if (form.publicKey.trim() === "") {
      return { error: "Reality 需要填写 publicKey" };
    }
    return {
      alpn: form.alpn
        .split(",")
        .map((entry) => entry.trim())
        .filter((entry) => entry !== ""),
      fingerprint: optional(form.fingerprint),
      publicKey: form.publicKey.trim(),
      serverName: form.serverName.trim(),
      shortId: optional(form.shortId),
      spiderX: optional(form.spiderX),
      type: "reality",
    };
  }
  // Hysteria2 has no plaintext mode, so the toggle does not apply to it.
  if (!form.tlsEnabled && usesStreamTransport(form.protocol)) {
    return null;
  }
  return {
    allowInsecure: form.allowInsecure,
    alpn: form.alpn
      .split(",")
      .map((entry) => entry.trim())
      .filter((entry) => entry !== ""),
    fingerprint: optional(form.fingerprint),
    pinnedSha256: optional(form.pinnedSha256),
    serverName: optional(form.serverName),
    type: "tls",
  };
}

function hasError(value: unknown): value is { error: string } {
  return typeof value === "object" && value !== null && "error" in value;
}

/**
 * Turns the raw form into the payload `session_create_node` expects, or an
 * error message ready to show. The Rust draft re-validates everything; this
 * pass exists so the common mistakes get a specific message.
 */
export function buildManualNodeDraft(
  form: ManualNodeForm,
): ManualNodeDraftResult {
  const name = form.name.trim();
  const server = form.server.trim();
  if (name === "" || server === "") {
    return { error: "请填写节点名称和服务器" };
  }
  const port = Number(form.port);
  if (!Number.isInteger(port) || port < 1 || port > 65_535) {
    return { error: "节点端口必须是 1 到 65535 的整数" };
  }

  const credential = buildCredential(form);
  if (hasError(credential)) {
    return credential;
  }
  const transport = buildTransport(form);
  if (hasError(transport)) {
    return transport;
  }
  const tls = buildTls(form);
  if (hasError(tls)) {
    return tls;
  }

  return {
    draft: {
      credential,
      name,
      port,
      server,
      tls,
      transport,
      udpEnabled: form.udpEnabled,
    },
  };
}

/** Fills the editable form from a draft returned by `session_node_draft`. */
export function formFromManualNodeDraft(draft: ManualNodeDraft): ManualNodeForm {
  const form: ManualNodeForm = {
    ...emptyManualNodeForm,
    name: draft.name,
    server: draft.server,
    port: String(draft.port),
    udpEnabled: draft.udpEnabled,
  };

  switch (draft.credential.protocol) {
    case "vless":
      form.protocol = "vless";
      form.userId = draft.credential.userId;
      form.flow = draft.credential.flow ?? "";
      break;
    case "vmess":
      form.protocol = "vmess";
      form.userId = draft.credential.userId;
      form.security = draft.credential.security;
      form.alterId = String(draft.credential.alterId);
      break;
    case "trojan":
      form.protocol = "trojan";
      form.password = draft.credential.password;
      break;
    case "shadowsocks":
      form.protocol = "shadowsocks";
      form.method = draft.credential.method;
      form.password = draft.credential.password;
      break;
    case "hysteria2":
      form.protocol = "hysteria2";
      form.authentication = draft.credential.authentication ?? "";
      form.obfsEnabled = draft.credential.obfuscation !== null;
      form.obfsMethod = draft.credential.obfuscation?.method ?? "Salamander";
      form.obfsPassword = draft.credential.obfuscation?.password ?? "";
      break;
    case "tuic":
      form.protocol = "tuic";
      form.userId = draft.credential.uuid;
      form.password = draft.credential.password ?? "";
      form.congestionControl = draft.credential.congestionControl ?? "";
      form.udpRelayMode = draft.credential.udpRelayMode ?? "";
      form.udpOverStream = draft.credential.udpOverStream;
      form.zeroRttHandshake = draft.credential.zeroRttHandshake;
      break;
    case "socks":
      form.protocol = "socks";
      form.username = draft.credential.username ?? "";
      form.password = draft.credential.password ?? "";
      break;
    case "http":
      form.protocol = "http";
      form.username = draft.credential.username ?? "";
      form.password = draft.credential.password ?? "";
      break;
    case "wireguard":
      form.protocol = "wireguard";
      form.privateKey = draft.credential.privateKey;
      form.peerPublicKey = draft.credential.peerPublicKey;
      form.preSharedKey = draft.credential.preSharedKey ?? "";
      form.localAddress = draft.credential.localAddress.join(",");
      form.mtu = draft.credential.mtu === null ? "" : String(draft.credential.mtu);
      form.reserved =
        draft.credential.reserved === null
          ? ""
          : draft.credential.reserved.join(",");
      break;
    case "anytls":
      form.protocol = "anytls";
      form.password = draft.credential.password;
      break;
    case "naive":
      form.protocol = "naive";
      form.username = draft.credential.username ?? "";
      form.password = draft.credential.password ?? "";
      form.quic = draft.credential.quic;
      form.quicCongestionControl = draft.credential.quicCongestionControl ?? "";
      break;
  }

  if (draft.transport?.type === "websocket") {
    form.transport = "websocket";
    form.wsPath = draft.transport.path;
    form.wsHost = draft.transport.host ?? "";
  } else if (draft.transport?.type === "httpupgrade") {
    form.transport = "httpupgrade";
    form.wsPath = draft.transport.path;
    form.wsHost = draft.transport.host ?? "";
  } else if (draft.transport?.type === "xhttp") {
    form.transport = "xhttp";
    form.wsPath = draft.transport.path;
    form.wsHost = draft.transport.host ?? "";
    form.xhttpMode = draft.transport.mode;
  } else if (draft.transport?.type === "grpc") {
    form.transport = "grpc";
    form.grpcServiceName = draft.transport.serviceName;
    form.grpcMode = draft.transport.mode;
    form.grpcAuthority = draft.transport.authority ?? "";
  } else if (draft.transport?.type === "kcp") {
    form.transport = "kcp";
    form.kcpMtu = draft.transport.mtu === null ? "" : String(draft.transport.mtu);
    form.kcpTti = draft.transport.tti === null ? "" : String(draft.transport.tti);
    form.kcpUplinkCapacity =
      draft.transport.uplinkCapacity === null
        ? ""
        : String(draft.transport.uplinkCapacity);
    form.kcpDownlinkCapacity =
      draft.transport.downlinkCapacity === null
        ? ""
        : String(draft.transport.downlinkCapacity);
    form.kcpCongestion = draft.transport.congestion;
    form.kcpHeaderType = (draft.transport.headerType ?? "") as KcpHeaderType;
    form.kcpSeed = draft.transport.seed ?? "";
  } else if (draft.transport?.type === "tcp") {
    form.transport = "tcp";
  }

  if (draft.tls?.type === "reality") {
    form.realityEnabled = true;
    form.tlsEnabled = true;
    form.serverName = draft.tls.serverName;
    form.publicKey = draft.tls.publicKey;
    form.shortId = draft.tls.shortId ?? "";
    form.spiderX = draft.tls.spiderX ?? "";
    form.fingerprint = draft.tls.fingerprint ?? "";
    form.alpn = draft.tls.alpn.join(",");
  } else if (draft.tls?.type === "tls") {
    form.tlsEnabled = true;
    form.allowInsecure = draft.tls.allowInsecure;
    form.serverName = draft.tls.serverName ?? "";
    form.fingerprint = draft.tls.fingerprint ?? "";
    form.pinnedSha256 = draft.tls.pinnedSha256 ?? "";
    form.alpn = draft.tls.alpn.join(",");
  }

  return form;
}
