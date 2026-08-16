import { groupTraffic } from "../appHelpers";
import { ProxiesView } from "../components/ProxiesView";
import type { AppModel } from "../hooks/useAppController";
import { setNodeGroupStrategy } from "../session";

export function ProxiesPage({ app }: { app: AppModel }) {
  const {
    t,
    busy,
    nodeGroups,
    nodes,
    node,
    nodeTraffic,
    proxyGroupId,
    setProxyGroupId,
    nodeTestInProgress,
    onTestNodes,
    onActivateNode,
    run,
    setNodeGroups,
  } = app;

  return (
    <div className="page-fill">
      <ProxiesView
        busy={busy}
        groups={nodeGroups}
        nodes={nodes}
        activeNodeId={node?.id ?? null}
        traffic={groupTraffic(
          nodes,
          nodeTraffic,
          proxyGroupId ?? nodeGroups[0]?.id ?? null,
        )}
        selectedGroupId={proxyGroupId}
        testing={nodeTestInProgress}
        t={t}
        onSelectGroup={setProxyGroupId}
        onStrategy={(groupId, strategy) =>
          void run(async () => {
            setNodeGroups(await setNodeGroupStrategy(groupId, strategy));
          })
        }
        onTestGroup={(ids) => void onTestNodes(ids)}
        onActivate={(id) => void onActivateNode(id)}
      />
    </div>
  );
}