use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::fmt::{Debug, Formatter};

use magies_domain::{ProxyNode, ProxyProtocol, TlsConfig, TransportConfig};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct CredentialIdentity([u8; 32]);

impl CredentialIdentity {
    /// Derives an opaque stable identity without retaining credential material.
    #[must_use]
    pub fn from_bytes(value: impl AsRef<[u8]>) -> Self {
        Self(Sha256::digest(value.as_ref()).into())
    }
}

impl Debug for CredentialIdentity {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CredentialIdentity([REDACTED])")
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct NodeFingerprint {
    protocol: ProxyProtocol,
    server: String,
    port: u16,
    credential_identity: CredentialIdentity,
    transport: Option<TransportConfig>,
    tls: Option<TlsConfig>,
}

#[derive(Clone, Debug)]
pub struct NodeDedupCandidate {
    node: ProxyNode,
    credential_identity: CredentialIdentity,
}

impl NodeDedupCandidate {
    #[must_use]
    pub const fn new(node: ProxyNode, credential_identity: CredentialIdentity) -> Self {
        Self {
            node,
            credential_identity,
        }
    }

    #[must_use]
    pub fn fingerprint(&self) -> NodeFingerprint {
        NodeFingerprint {
            protocol: self.node.protocol_type,
            server: self.node.server.as_str().to_owned(),
            port: self.node.port.get(),
            credential_identity: self.credential_identity,
            transport: self.node.transport.clone(),
            tls: self.node.tls.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NodeDedupSummary {
    pub added_nodes: usize,
    pub updated_nodes: usize,
    pub removed_nodes: usize,
    pub deduplicated_nodes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeDedupResult {
    pub nodes: Vec<ProxyNode>,
    pub summary: NodeDedupSummary,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NodeDeduplicator;

impl NodeDeduplicator {
    /// Reconciles a subscription snapshot by stable node fingerprint.
    ///
    /// Existing matches retain their UUID, name, group, enabled state, and
    /// latency data. The first incoming duplicate wins, while duplicate
    /// existing rows resolve to the lowest UUID.
    #[must_use]
    pub fn reconcile(
        existing: &[NodeDedupCandidate],
        incoming: Vec<NodeDedupCandidate>,
    ) -> NodeDedupResult {
        let mut existing_by_fingerprint = HashMap::with_capacity(existing.len());
        for candidate in existing {
            match existing_by_fingerprint.entry(candidate.fingerprint()) {
                Entry::Vacant(entry) => {
                    entry.insert(&candidate.node);
                }
                Entry::Occupied(mut entry) => {
                    if candidate.node.id < entry.get().id {
                        entry.insert(&candidate.node);
                    }
                }
            }
        }

        let mut seen_incoming = HashSet::with_capacity(incoming.len());
        let mut nodes = Vec::with_capacity(incoming.len());
        let mut summary = NodeDedupSummary::default();
        for candidate in incoming {
            let node_fingerprint = candidate.fingerprint();
            if !seen_incoming.insert(node_fingerprint.clone()) {
                summary.deduplicated_nodes += 1;
                continue;
            }

            let mut node = candidate.node;
            if let Some(existing_node) = existing_by_fingerprint.get(&node_fingerprint) {
                preserve_local_state(&mut node, existing_node);
                summary.updated_nodes += 1;
            } else {
                summary.added_nodes += 1;
            }
            nodes.push(node);
        }
        summary.removed_nodes = existing.len() - summary.updated_nodes;

        NodeDedupResult { nodes, summary }
    }
}

fn preserve_local_state(node: &mut ProxyNode, existing: &ProxyNode) {
    node.id = existing.id;
    node.name = existing.name.clone();
    node.subscription_id = existing.subscription_id;
    node.group_id = existing.group_id;
    node.latency_ms = existing.latency_ms;
    node.last_tested_at = existing.last_tested_at;
    node.enabled = existing.enabled;
}
