import { useCallback, useEffect, useState } from "react";

import { loadPlatformSummary, type PlatformSummary } from "./platform";
import {
  connectSession,
  deleteNode,
  dismissSystemProxyRecovery,
  disconnectSession,
  exportDiagnostics,
  importNode,
  isCommandError,
  loadNodes,
  loadSessionStatus,
  loadSystemProxyStartupStatus,
  recoverSystemProxy,
  selectNode,
  type NodeSummary,
  type SessionStatus,
  type SystemProxyStartupStatus,
} from "./session";
import {
  createSubscription,
  deleteSubscription,
  loadSubscriptions,
  refreshSubscription,
  updateSubscription,
  type SubscriptionSummary,
} from "./subscriptions";

/**
 * Automatic network recovery can reconnect the session without the user acting,
 * so the dashboard re-reads the status instead of trusting its last command.
 */
const REFRESH_INTERVAL_MS = 3_000;

const TUN_LABEL: Record<PlatformSummary["tunAvailability"], string> = {
  requiresElevation: "需要管理员权限",
  unavailableInUnsignedBuild: "未签名版本不可用",
};

/**
 * PRD V1.1 DoD 6 requires the unsigned-build risk to be visible on first
 * launch, not only on the download page.
 */
const UNSIGNED_NOTICE_KEY = "mgclash.unsignedNoticeDismissed";

const TUN_NOTICE: Record<PlatformSummary["tunAvailability"], string> = {
  requiresElevation: "TUN 需要管理员权限才能启用。",
  unavailableInUnsignedBuild:
    "TUN 在未签名版本中不可用：它需要 Apple 签发的 Network Extension entitlement。本机可用的是本地 HTTP/SOCKS 代理与系统代理。",
};

function noticeWasDismissed(): boolean {
  try {
    return localStorage.getItem(UNSIGNED_NOTICE_KEY) === "1";
  } catch {
    // A webview with storage disabled should still start; showing the notice
    // again is the safe direction to fail.
    return false;
  }
}

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
  const [nodes, setNodes] = useState<NodeSummary[]>([]);
  const [subscriptions, setSubscriptions] = useState<SubscriptionSummary[]>([]);
  const [uri, setUri] = useState("");
  const [subscriptionName, setSubscriptionName] = useState("");
  const [subscriptionUrl, setSubscriptionUrl] = useState("");
  const [subscriptionInterval, setSubscriptionInterval] = useState("60");
  const [subscriptionAutoUpdate, setSubscriptionAutoUpdate] = useState(true);
  const [subscriptionEnabled, setSubscriptionEnabled] = useState(true);
  const [editingSubscriptionId, setEditingSubscriptionId] = useState<
    string | null
  >(null);
  const [error, setError] = useState<string | null>(null);
  const [exportedTo, setExportedTo] = useState<string | null>(null);
  const [systemProxyStartup, setSystemProxyStartup] =
    useState<SystemProxyStartupStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [noticeDismissed, setNoticeDismissed] = useState(noticeWasDismissed);

  useEffect(() => {
    loadPlatformSummary().then(setPlatform, (failure: unknown) =>
      setPlatformError(describeFailure(failure)),
    );
    loadSessionStatus().then(setStatus, (failure: unknown) =>
      setError(describeFailure(failure)),
    );
    loadNodes().then(setNodes, (failure: unknown) =>
      setError(describeFailure(failure)),
    );
    loadSubscriptions().then(setSubscriptions, (failure: unknown) =>
      setError(describeFailure(failure)),
    );
    loadSystemProxyStartupStatus().then(
      setSystemProxyStartup,
      (failure: unknown) => setError(describeFailure(failure)),
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
    setExportedTo(null);
    try {
      setStatus(await command());
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  const onExport = useCallback(async () => {
    setBusy(true);
    setError(null);
    setExportedTo(null);
    try {
      setExportedTo(await exportDiagnostics());
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  const resolveSystemProxyStartup = useCallback(
    async (command: () => Promise<SystemProxyStartupStatus>) => {
      setBusy(true);
      setError(null);
      try {
        setSystemProxyStartup(await command());
      } catch (failure: unknown) {
        setError(describeFailure(failure));
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  const dismissNotice = useCallback(() => {
    try {
      localStorage.setItem(UNSIGNED_NOTICE_KEY, "1");
    } catch {
      // Dismissal still applies to this launch even if it cannot be stored.
    }
    setNoticeDismissed(true);
  }, []);

  const onImport = useCallback(async () => {
    const sharingUri = uri.trim();
    if (sharingUri === "") {
      setError("请先粘贴分享链接");
      return;
    }
    setBusy(true);
    setError(null);
    setExportedTo(null);
    try {
      setStatus(await importNode(sharingUri));
      setNodes(await loadNodes());
      setUri("");
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [uri]);

  const onDeleteNode = useCallback(async (id: string) => {
    setBusy(true);
    setError(null);
    setExportedTo(null);
    try {
      setStatus(await deleteNode(id));
      setNodes(await loadNodes());
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, []);

  const resetSubscriptionForm = useCallback(() => {
    setEditingSubscriptionId(null);
    setSubscriptionName("");
    setSubscriptionUrl("");
    setSubscriptionInterval("60");
    setSubscriptionAutoUpdate(true);
    setSubscriptionEnabled(true);
  }, []);

  const syncNodes = useCallback(async () => {
    const [nextNodes, nextStatus] = await Promise.all([
      loadNodes(),
      loadSessionStatus(),
    ]);
    setNodes(nextNodes);
    setStatus(nextStatus);
  }, []);

  const onSaveSubscription = useCallback(async () => {
    const name = subscriptionName.trim();
    const url = subscriptionUrl.trim();
    const updateIntervalMinutes = Number(subscriptionInterval);
    if (name === "" || (editingSubscriptionId === null && url === "")) {
      setError("请填写订阅名称和地址");
      return;
    }
    if (!Number.isInteger(updateIntervalMinutes) || updateIntervalMinutes < 1) {
      setError("更新间隔必须是正整数");
      return;
    }

    setBusy(true);
    setError(null);
    try {
      const saved =
        editingSubscriptionId === null
          ? await createSubscription({
              autoUpdate: subscriptionAutoUpdate,
              name,
              updateIntervalMinutes,
              url,
            })
          : await updateSubscription({
              autoUpdate: subscriptionAutoUpdate,
              enabled: subscriptionEnabled,
              id: editingSubscriptionId,
              name,
              updateIntervalMinutes,
              url: url === "" ? null : url,
            });
      setSubscriptions((current) => {
        const existing = current.findIndex((item) => item.id === saved.id);
        if (existing === -1) {
          return [...current, saved];
        }
        return current.map((item) => (item.id === saved.id ? saved : item));
      });
      await syncNodes();
      resetSubscriptionForm();
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [
    editingSubscriptionId,
    resetSubscriptionForm,
    subscriptionAutoUpdate,
    subscriptionEnabled,
    subscriptionInterval,
    subscriptionName,
    subscriptionUrl,
    syncNodes,
  ]);

  const onEditSubscription = useCallback((item: SubscriptionSummary) => {
    setEditingSubscriptionId(item.id);
    setSubscriptionName(item.name);
    setSubscriptionUrl("");
    setSubscriptionInterval(String(item.updateIntervalMinutes));
    setSubscriptionAutoUpdate(item.autoUpdate);
    setSubscriptionEnabled(item.enabled);
  }, []);

  const onRefreshSubscription = useCallback(async (id: string) => {
    setBusy(true);
    setError(null);
    try {
      const refreshed = await refreshSubscription(id);
      setSubscriptions((current) =>
        current.map((item) => (item.id === id ? refreshed : item)),
      );
      await syncNodes();
    } catch (failure: unknown) {
      setError(describeFailure(failure));
    } finally {
      setBusy(false);
    }
  }, [syncNodes]);

  const onDeleteSubscription = useCallback(
    async (id: string) => {
      setBusy(true);
      setError(null);
      try {
        await deleteSubscription(id);
        setSubscriptions((current) =>
          current.filter((item) => item.id !== id),
        );
        await syncNodes();
        if (editingSubscriptionId === id) {
          resetSubscriptionForm();
        }
      } catch (failure: unknown) {
        setError(describeFailure(failure));
      } finally {
        setBusy(false);
      }
    },
    [editingSubscriptionId, resetSubscriptionForm, syncNodes],
  );

  const connected = status?.connected ?? false;
  const node = status?.node ?? null;

  return (
    <main className="app-shell">
      <header className="toolbar">
        <h1>MgClash</h1>
      </header>

      <section className="content">
        {systemProxyStartup === "restoreRequired" && (
          <section className="notice" aria-label="系统代理恢复">
            <p>
              检测到上次异常退出留下的系统代理设置。恢复原设置可以避免系统继续指向已经停止的本地代理。
            </p>
            <div className="actions">
              <button
                type="button"
                disabled={busy}
                onClick={() =>
                  void resolveSystemProxyStartup(recoverSystemProxy)
                }
              >
                恢复原设置
              </button>
              <button
                type="button"
                disabled={busy}
                onClick={() =>
                  void resolveSystemProxyStartup(dismissSystemProxyRecovery)
                }
              >
                保留当前设置
              </button>
            </div>
          </section>
        )}

        {!noticeDismissed && (
          <div className="notice" role="note">
            <p>
              这是<strong>未签名</strong>版本：macOS Gatekeeper 与 Windows
              SmartScreen 会在首次打开时提示，需要你手动确认后才能运行。
            </p>
            <p>{platform ? TUN_NOTICE[platform.tunAvailability] : ""}</p>
            <div className="actions">
              <button type="button" onClick={dismissNotice}>
                我知道了
              </button>
            </div>
          </div>
        )}

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
            disabled={
              busy || node === null || systemProxyStartup !== "clean"
            }
            onClick={() =>
              void run(connected ? disconnectSession : connectSession)
            }
          >
            {connected ? "断开" : "连接"}
          </button>
        </div>

        <h2>节点</h2>

        {nodes.length === 0 ? (
          <p className="hint">尚未导入节点</p>
        ) : (
          <table className="node-list" aria-label="节点列表">
            <thead>
              <tr>
                <th>名称</th>
                <th>协议</th>
                <th>服务器</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {nodes.map((candidate) => {
                const selected = candidate.id === node?.id;
                return (
                  <tr key={candidate.id}>
                    <td>{candidate.name}</td>
                    <td>{candidate.protocol}</td>
                    <td>{`${candidate.server}:${candidate.port}`}</td>
                    <td className="node-actions">
                      <button
                        type="button"
                        aria-label={`选择 ${candidate.name}`}
                        disabled={busy || connected || selected}
                        onClick={() =>
                          void run(() => selectNode(candidate.id))
                        }
                      >
                        {selected ? "当前" : "选择"}
                      </button>
                      <button
                        type="button"
                        aria-label={`删除 ${candidate.name}`}
                        disabled={busy || connected || !candidate.deletable}
                        onClick={() => void onDeleteNode(candidate.id)}
                      >
                        {candidate.deletable ? "删除" : "订阅管理"}
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}

        <h2>订阅</h2>

        {subscriptions.length === 0 ? (
          <p className="hint">尚未添加订阅</p>
        ) : (
          <table className="node-list" aria-label="订阅列表">
            <thead>
              <tr>
                <th>名称</th>
                <th>节点</th>
                <th>更新</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {subscriptions.map((item) => (
                <tr key={item.id}>
                  <td>{item.name}</td>
                  <td>{item.nodeCount}</td>
                  <td>{item.lastUpdatedAt === null ? "从未" : "已更新"}</td>
                  <td className="node-actions">
                    <button
                      type="button"
                      aria-label={`编辑 ${item.name}`}
                      disabled={busy || connected}
                      onClick={() => onEditSubscription(item)}
                    >
                      编辑
                    </button>
                    <button
                      type="button"
                      aria-label={`刷新 ${item.name}`}
                      disabled={busy || connected || !item.enabled}
                      onClick={() => void onRefreshSubscription(item.id)}
                    >
                      刷新
                    </button>
                    <button
                      type="button"
                      aria-label={`删除订阅 ${item.name}`}
                      disabled={busy || connected}
                      onClick={() => void onDeleteSubscription(item.id)}
                    >
                      删除
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}

        <div className="subscription-form">
          <label>
            名称
            <input
              aria-label="订阅名称"
              value={subscriptionName}
              disabled={busy}
              onChange={(event) => setSubscriptionName(event.target.value)}
            />
          </label>
          <label>
            地址
            <input
              aria-label="订阅地址"
              type="password"
              value={subscriptionUrl}
              disabled={busy}
              placeholder={
                editingSubscriptionId === null ? "https://" : "留空则不修改"
              }
              onChange={(event) => setSubscriptionUrl(event.target.value)}
            />
          </label>
          <label>
            更新间隔（分钟）
            <input
              aria-label="更新间隔"
              type="number"
              min="1"
              value={subscriptionInterval}
              disabled={busy}
              onChange={(event) => setSubscriptionInterval(event.target.value)}
            />
          </label>
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={subscriptionAutoUpdate}
              disabled={busy}
              onChange={(event) =>
                setSubscriptionAutoUpdate(event.target.checked)
              }
            />
            自动更新
          </label>
          {editingSubscriptionId !== null && (
            <label className="checkbox-label">
              <input
                type="checkbox"
                checked={subscriptionEnabled}
                disabled={busy}
                onChange={(event) => setSubscriptionEnabled(event.target.checked)}
              />
              启用订阅
            </label>
          )}
        </div>

        <div className="actions">
          <button
            type="button"
            disabled={busy}
            onClick={() => void onSaveSubscription()}
          >
            {editingSubscriptionId === null ? "添加订阅" : "保存修改"}
          </button>
          {editingSubscriptionId !== null && (
            <button
              type="button"
              disabled={busy}
              onClick={resetSubscriptionForm}
            >
              取消
            </button>
          )}
        </div>

        <h2>导入节点</h2>

        <textarea
          aria-label="分享链接"
          rows={3}
          value={uri}
          disabled={busy || connected}
          placeholder="vless:// vmess:// trojan:// ss:// hysteria2://"
          onChange={(event) => setUri(event.target.value)}
        />

        <div className="actions">
          <button
            type="button"
            disabled={busy || connected}
            onClick={() => void onImport()}
          >
            导入
          </button>
        </div>

        <h2>诊断</h2>

        <p className="hint">
          导出的诊断包已按 PRD 25.3 脱敏：凭据字段一律替换为 [REDACTED]。
        </p>

        <div className="actions">
          <button type="button" disabled={busy} onClick={() => void onExport()}>
            导出诊断
          </button>
        </div>

        {exportedTo !== null && (
          <p className="success" role="status">
            已导出到 {exportedTo}
          </p>
        )}

        {error !== null && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
      </section>
    </main>
  );
}
