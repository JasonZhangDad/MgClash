import { invoke } from "@tauri-apps/api/core";

export type ProxyProtocol =
  | "hysteria2"
  | "shadowsocks"
  | "trojan"
  | "vless"
  | "vmess";

export interface NodeSummary {
  deletable: boolean;
  id: string;
  lastTestedAt: number | null;
  latencyMs: number | null;
  name: string;
  port: number;
  protocol: ProxyProtocol;
  server: string;
}

export type NodeTestStatus = "failed" | "success" | "timeout";

export interface NodeTestResult {
  id: string;
  latencyMs: number | null;
  status: NodeTestStatus;
}

export interface TrafficSnapshot {
  downloadBytesPerSecond: number;
  monthBytes: number;
  todayBytes: number;
  totalBytes: number;
  uploadBytesPerSecond: number;
}

export interface SessionStatus {
  connected: boolean;
  core: string;
  dns: DnsSettings;
  httpPort: number;
  mode: RoutingMode;
  node: NodeSummary | null;
  socksPort: number;
  systemProxy: boolean;
}

export type RoutingMode = "direct" | "global" | "rule";

export type DnsMode = "system" | "plainUdp" | "plainTcp" | "doh" | "dot";

export type DnsStrategy =
  | "preferIpv4"
  | "preferIpv6"
  | "ipv4Only"
  | "ipv6Only";

export interface DnsSettings {
  dohPath: string;
  fakeIpEnabled: boolean;
  ipv6Enabled: boolean;
  mode: DnsMode;
  port: number;
  server: string;
  strategy: DnsStrategy;
  systemDomains: string[];
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

export function setRoutingMode(mode: RoutingMode): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_set_routing_mode", { mode });
}

export function setDnsSettings(settings: DnsSettings): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_set_dns_settings", { settings });
}

export function importNode(uri: string): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_import_node", { uri });
}

export function loadNodes(): Promise<NodeSummary[]> {
  return invoke<NodeSummary[]>("session_nodes");
}

export function testNode(id: string): Promise<NodeTestResult> {
  return invoke<NodeTestResult>("session_test_node", { id });
}

export function testUrl(url: string): Promise<NodeTestResult> {
  return invoke<NodeTestResult>("session_url_test", { url });
}

export function loadTraffic(): Promise<TrafficSnapshot> {
  return invoke<TrafficSnapshot>("session_traffic");
}

export async function testAllNodes(
  ids: string[],
  onResult: (result: NodeTestResult) => void,
  isCancelled: () => boolean,
): Promise<void> {
  let nextIndex = 0;
  let firstFailure: unknown;
  let hasFailure = false;
  const workers = Array.from(
    { length: Math.min(8, ids.length) },
    async () => {
      while (!isCancelled() && !hasFailure) {
        const index = nextIndex;
        nextIndex += 1;
        if (index >= ids.length) {
          return;
        }
        try {
          onResult(await testNode(ids[index]));
        } catch (failure: unknown) {
          if (!hasFailure) {
            firstFailure = failure;
            hasFailure = true;
          }
        }
      }
    },
  );
  await Promise.all(workers);
  if (hasFailure) {
    throw firstFailure;
  }
}

export function selectNode(id: string): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_select_node", { id });
}

export function deleteNode(id: string): Promise<SessionStatus> {
  return invoke<SessionStatus>("session_delete_node", { id });
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
