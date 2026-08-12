import type {
  ManualCredentialDraft,
  ManualNodeDraft,
  ObfuscationMethod,
  ProxyProtocol,
  TlsDraft,
  TransportDraft,
  VmessSecurity,
} from "./session";

export type TransportKind = "tcp" | "websocket" | "grpc";

export type GrpcMode = "gun" | "multi" | "guna";

/** Every field the manual creation form can collect, as raw strings. */
export interface ManualNodeForm {
  alterId: string;
  allowInsecure: boolean;
  alpn: string;
  authentication: string;
  fingerprint: string;
  flow: string;
  grpcAuthority: string;
  grpcMode: GrpcMode;
  grpcServiceName: string;
  method: string;
  name: string;
  obfsEnabled: boolean;
  obfsMethod: ObfuscationMethod;
  obfsPassword: string;
  password: string;
  port: string;
  protocol: ProxyProtocol;
  security: VmessSecurity;
  server: string;
  serverName: string;
  tlsEnabled: boolean;
  transport: TransportKind;
  udpEnabled: boolean;
  userId: string;
  wsHost: string;
  wsPath: string;
}

export const emptyManualNodeForm: ManualNodeForm = {
  alterId: "0",
  allowInsecure: false,
  alpn: "",
  authentication: "",
  fingerprint: "",
  flow: "",
  grpcAuthority: "",
  grpcMode: "gun",
  grpcServiceName: "",
  method: "aes-256-gcm",
  name: "",
  obfsEnabled: false,
  obfsMethod: "Salamander",
  obfsPassword: "",
  password: "",
  port: "",
  protocol: "vless",
  security: "Auto",
  server: "",
  serverName: "",
  tlsEnabled: false,
  transport: "tcp",
  udpEnabled: true,
  userId: "",
  wsHost: "",
  wsPath: "",
};

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

/** Hysteria2 carries its own QUIC transport and always runs over TLS. */
export function usesStreamTransport(protocol: ProxyProtocol): boolean {
  return protocol !== "hysteria2";
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
  }
}

function buildTransport(
  form: ManualNodeForm,
): TransportDraft | null | { error: string } {
  if (!usesStreamTransport(form.protocol)) {
    return null;
  }
  // Shadowsocks accepts nothing but plain TCP, so a transport left over from a
  // previously selected protocol must not leak into the payload.
  if (form.protocol === "shadowsocks") {
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
  }
}

function buildTls(form: ManualNodeForm): TlsDraft | null {
  // The Shadowsocks outbound has no TLS layer at all.
  if (form.protocol === "shadowsocks") {
    return null;
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

  return {
    draft: {
      credential,
      name,
      port,
      server,
      tls: buildTls(form),
      transport,
      udpEnabled: form.udpEnabled,
    },
  };
}
