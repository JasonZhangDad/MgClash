import { useCallback, useEffect, useState } from "react";

import { loadPlatformSummary, type PlatformSummary } from "./platform";
import {
  connectSession,
  disconnectSession,
  importNode,
  isCommandError,
  loadSessionStatus,
  type SessionStatus,
} from "./session";

/**
 * Automatic network recovery can reconnect the session without the user acting,
 * so the dashboard re-reads the status instead of trusting its last command.
 */
const REFRESH_INTERVAL_MS = 3_000;

const TUN_LABEL: Record<PlatformSummary["tunAvailability"], string> = {
  requiresElevation: "需要管理员权限",
  unavailableInUnsignedBuild: "未签名版本不可用",
};

/** Command rejections are typed values from Rust, not `Error` instances. */
function describeFailure(error: unknown): string {
  if (isCommandError(error)) {
    return error.message;
  }
  return error instanceof Error ? error.message : String(error);
}

export default function App() {
  const [platform, setPlatform] = useState<PlatformSummary | null>(null);
  const [platformError, setPlatformError] = useState<string | null>(null);
  const [status, setStatus] = useState<SessionStatus | null>(null);
  const [uri, setUri] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    loadPlatformSummary().then(setPlatform, (failure: unknown) =>
      setPlatformError(describeFailure(failure)),
    );
    loadSessionStatus().then(setStatus, (failure: unknown) =>
      setError(describeFailure(failure)),
    );
  }, []);

  useEffect(() => {
    // Skip while a command is in flight: it owns the status it is about to set.
    if (busy) {
      return undefined;
    }
    const timer = setInterval(() => {
      loadSessionStatus().then(setStatus, (failure: unknown) => {
        // A background refresh must not replace an error the user is reading.
        console.warn("session status refresh failed", failure);
      });
    }, REFRESH_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [busy]);

  const run = useCallback(async (command: () => Promise<SessionStatus>) => {
    setBusy(true);
    setError(null);
    try {
      setStatus(await command());
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  const onImport = useCallback(() => {
    const sharingUri = uri.trim();
    if (sharingUri === "") {
      setError("请先粘贴分享链接");
      return;
    }
    void run(() => importNode(sharingUri));
  }, [run, uri]);

  const connected = status?.connected ?? false;
  const node = status?.node ?? null;

  return (
    <main className="app-shell">
      <header className="toolbar">
        <h1>MgClash</h1>
      </header>

      <section className="content">
        <h2>连接</h2>

        <dl>
          <div>
            <dt>状态</dt>
            <dd className={connected ? "connected" : undefined}>
              {connected ? "已连接" : "未连接"}
            </dd>
          </div>
          <div>
            <dt>节点</dt>
            <dd>{node ? node.name : "尚未导入"}</dd>
          </div>
          <div>
            <dt>协议</dt>
            <dd>{node ? node.protocol : "—"}</dd>
          </div>
          <div>
            <dt>地址</dt>
            <dd>{node ? `${node.server}:${node.port}` : "—"}</dd>
          </div>
          <div>
            <dt>Core</dt>
            <dd>{status ? status.core : "—"}</dd>
          </div>
          <div>
            <dt>模式</dt>
            <dd>{status ? status.mode : "—"}</dd>
          </div>
          <div>
            <dt>本地代理</dt>
            <dd>
              {status
                ? `SOCKS ${status.socksPort} · HTTP ${status.httpPort}`
                : "—"}
            </dd>
          </div>
          <div>
            <dt>系统代理</dt>
            <dd>
              {connected && status?.systemProxy
                ? "已接管系统代理"
                : "未接管系统代理"}
            </dd>
          </div>
          <div>
            <dt>TUN</dt>
            <dd>{platform ? TUN_LABEL[platform.tunAvailability] : "—"}</dd>
          </div>
          <div>
            <dt>构建目标</dt>
            <dd>{platform ? platform.artifactIdentifier : platformError}</dd>
          </div>
        </dl>

        <div className="actions">
          <button
            type="button"
            disabled={busy || node === null}
            onClick={() =>
              void run(connected ? disconnectSession : connectSession)
            }
          >
            {connected ? "断开" : "连接"}
          </button>
        </div>

        <h2>导入节点</h2>

        <textarea
          aria-label="分享链接"
          rows={3}
          value={uri}
          disabled={busy}
          placeholder="vless:// vmess:// trojan:// ss:// hysteria2://"
          onChange={(event) => setUri(event.target.value)}
        />

        <div className="actions">
          <button type="button" disabled={busy} onClick={onImport}>
            导入
          </button>
        </div>

        {error !== null && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
      </section>
    </main>
  );
}
