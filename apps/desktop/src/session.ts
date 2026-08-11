import { invoke } from "@tauri-apps/api/core";

export type ProxyProtocol =
  | "hysteria2"
  | "shadowsocks"
  | "trojan"
  | "vless"
  | "vmess";

export interface NodeSummary {
  name: string;
  port: number;
  protocol: ProxyProtocol;
  server: string;
}

export interface SessionStatus {
  connected: boolean;
  core: string;
  httpPort: number;
  mode: string;
  node: NodeSummary | null;
  socksPort: number;
  systemProxy: boolean;
}

/** The shape every failed Tauri command returns. */
export interface CommandError {
  code: string;
  message: string;
}

export type SystemProxyStartupStatus = "clean" | "restoreRequired";

export function isCommandError(value: unknown): value is CommandError {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as CommandError).code === "string" &&
    typeof (value as CommandError).message === "string"
  );
}

export function loadSessionStatus(): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_status");
}

export function importNode(uri: string): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_import_node", { uri });
}

export function connectSession(): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_connect");
}

export function disconnectSession(): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_disconnect");
}

export function loadSystemProxyStartupStatus(): Promise<SystemProxyStartupStatus> {
  return invoke<SystemProxyStartupStatus>("system_proxy_startup_status");
}

export function recoverSystemProxy(): Promise<SystemProxyStartupStatus> {
  return invoke<SystemProxyStartupStatus>("system_proxy_recover");
}

export function dismissSystemProxyRecovery(): Promise<SystemProxyStartupStatus> {
  return invoke<SystemProxyStartupStatus>("system_proxy_dismiss");
}

/** Writes a redacted diagnostic bundle and resolves with its path. */
export function exportDiagnostics(): Promise<string> {
  return invoke<string>("export_diagnostics");
}
