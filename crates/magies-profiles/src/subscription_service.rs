use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::str;

use magies_domain::{CredentialRef, ProxyNode, Subscription, TimestampMillis};
use magies_storage::{SecretStore, SecretStoreError, SecretValue};
use thiserror::Error;
use uuid::Uuid;

use crate::subscription_url::{
    SubscriptionUrlError, effective_fetch_urls, split_subscription_urls,
};
use crate::{
    CredentialCodec, CredentialCodecError, CredentialIdentity, NodeDedupCandidate,
    NodeDeduplicator, ParsedSubscriptionNode, SqliteSubscriptionStore, SubscriptionContentError,
    SubscriptionContentParser, SubscriptionFetchError, SubscriptionFetchResult,
    SubscriptionFetcher, SubscriptionTransactionError, SubscriptionUpdate, SubscriptionValidators,
};

type PendingCredential = (CredentialRef, SecretValue);
type PendingCredentials = HashMap<String, PendingCredential>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriptionRefreshOutcome {
    NotModified,
    Updated {
        added_nodes: usize,
        updated_nodes: usize,
        removed_nodes: usize,
        deduplicated_nodes: usize,
    },
}

pub struct SubscriptionRefreshService<'a, S: SecretStore + ?Sized> {
    store: &'a mut SqliteSubscriptionStore,
    secret_store: &'a S,
    fetcher: SubscriptionFetcher,
}

impl<'a, S: SecretStore + ?Sized> SubscriptionRefreshService<'a, S> {
    #[must_use]
    pub const fn new(
        store: &'a mut SqliteSubscriptionStore,
        secret_store: &'a S,
        fetcher: SubscriptionFetcher,
    ) -> Self {
        Self {
            store,
            secret_store,
            fetcher,
        }
    }

    /// Fetches and atomically replaces one subscription snapshot.
    ///
    /// # Errors
    ///
    /// Returns a typed fetch, parsing, credential, or transaction error. The
    /// previous snapshot remains committed unless old-credential cleanup fails
    /// after a successful database commit.
    pub async fn refresh(
        &mut self,
        subscription_id: Uuid,
        updated_at: TimestampMillis,
    ) -> Result<SubscriptionRefreshOutcome, SubscriptionRefreshError> {
        let subscription = self.store.subscription(subscription_id)?.ok_or(
            SubscriptionTransactionError::SubscriptionNotFound {
                id: subscription_id,
            },
        )?;
        let url_secret = self
            .secret_store
            .get(&subscription.url_secret_ref)
            .map_err(|source| SubscriptionRefreshError::SecretStore {
                operation: SubscriptionSecretOperation::ReadUrl,
                source,
            })?;
        let url = str::from_utf8(url_secret.expose_secret())
            .map_err(|source| SubscriptionRefreshError::InvalidUrlSecret { source })?;
        let sources = split_subscription_urls(url);
        let fetch_urls = effective_fetch_urls(&sources, subscription.subconverter_url.as_deref())
            .map_err(SubscriptionRefreshError::Url)?;
        let validators = SubscriptionValidators::new(
            subscription.etag.clone(),
            subscription.last_modified.clone(),
        );
        let user_agent = subscription.user_agent.as_deref();

        if fetch_urls.len() == 1 {
            return match self
                .fetcher
                .fetch(&fetch_urls[0], Some(&validators), user_agent)
                .await?
            {
                SubscriptionFetchResult::NotModified { validators } => {
                    self.store
                        .touch_subscription(subscription_id, &validators, updated_at)?;
                    Ok(SubscriptionRefreshOutcome::NotModified)
                }
                SubscriptionFetchResult::Updated {
                    content,
                    validators,
                } => self.commit_updated(&subscription, &content, validators, updated_at),
            };
        }

        let mut parts = Vec::with_capacity(fetch_urls.len());
        let mut last_validators = validators;
        for fetch_url in fetch_urls {
            match self.fetcher.fetch(&fetch_url, None, user_agent).await? {
                SubscriptionFetchResult::NotModified { .. } => {}
                SubscriptionFetchResult::Updated {
                    content,
                    validators,
                } => {
                    last_validators = validators;
                    parts.push(content);
                }
            }
        }
        if parts.is_empty() {
            self.store
                .touch_subscription(subscription_id, &last_validators, updated_at)?;
            return Ok(SubscriptionRefreshOutcome::NotModified);
        }
        let merged = merge_subscription_contents(&parts);
        self.commit_updated(&subscription, &merged, last_validators, updated_at)
    }

    fn commit_updated(
        &mut self,
        subscription: &Subscription,
        content: &[u8],
        validators: SubscriptionValidators,
        updated_at: TimestampMillis,
    ) -> Result<SubscriptionRefreshOutcome, SubscriptionRefreshError> {
        let subscription_id = subscription.id;
        let parsed = SubscriptionContentParser
            .parse(content, subscription_id)?
            .into_iter()
            .filter(|parsed| subscription.accepts_node_name(parsed.node().name.as_str()))
            .collect::<Vec<_>>();
        let existing_nodes = self.store.subscription_nodes(subscription_id)?;
        let existing = self.existing_candidates(&existing_nodes)?;
        let (incoming, mut pending_credentials) = incoming_candidates(parsed)?;
        let reconciled = NodeDeduplicator::reconcile(&existing, incoming);
        let credentials = take_reconciled_credentials(&reconciled.nodes, &mut pending_credentials);
        let saved_credentials = self.save_credentials(&credentials)?;
        let update = match SubscriptionUpdate::new(
            subscription_id,
            reconciled.nodes,
            validators,
            updated_at,
        ) {
            Ok(update) => update,
            Err(error) => {
                self.rollback_credentials(&saved_credentials)?;
                return Err(error.into());
            }
        };
        if let Err(error) = self.store.apply_update(&update) {
            self.rollback_credentials(&saved_credentials)?;
            return Err(error.into());
        }

        self.delete_old_credentials(&existing_nodes, &saved_credentials)?;
        Ok(SubscriptionRefreshOutcome::Updated {
            added_nodes: reconciled.summary.added_nodes,
            updated_nodes: reconciled.summary.updated_nodes,
            removed_nodes: reconciled.summary.removed_nodes,
            deduplicated_nodes: reconciled.summary.deduplicated_nodes,
        })
    }

    fn existing_candidates(
        &self,
        nodes: &[ProxyNode],
    ) -> Result<Vec<NodeDedupCandidate>, SubscriptionRefreshError> {
        nodes
            .iter()
            .map(|node| {
                let secret = self
                    .secret_store
                    .get(&node.credential_ref)
                    .map_err(|source| SubscriptionRefreshError::SecretStore {
                        operation: SubscriptionSecretOperation::ReadExistingNode,
                        source,
                    })?;
                Ok(NodeDedupCandidate::new(
                    node.clone(),
                    CredentialIdentity::from_bytes(secret.expose_secret()),
                ))
            })
            .collect()
    }

    fn save_credentials(
        &self,
        credentials: &[(CredentialRef, SecretValue)],
    ) -> Result<Vec<CredentialRef>, SubscriptionRefreshError> {
        let mut saved = Vec::with_capacity(credentials.len());
        for (credential_ref, secret) in credentials {
            if let Err(source) = self.secret_store.put(credential_ref, secret) {
                self.rollback_credentials(&saved)?;
                return Err(SubscriptionRefreshError::SecretStore {
                    operation: SubscriptionSecretOperation::SaveNewNode,
                    source,
                });
            }
            saved.push(credential_ref.clone());
        }
        Ok(saved)
    }

    fn rollback_credentials(
        &self,
        credential_refs: &[CredentialRef],
    ) -> Result<(), SubscriptionRefreshError> {
        for credential_ref in credential_refs.iter().rev() {
            self.secret_store.delete(credential_ref).map_err(|source| {
                SubscriptionRefreshError::SecretStore {
                    operation: SubscriptionSecretOperation::RollbackNewNode,
                    source,
                }
            })?;
        }
        Ok(())
    }

    fn delete_old_credentials(
        &self,
        old_nodes: &[ProxyNode],
        saved_credentials: &[CredentialRef],
    ) -> Result<(), SubscriptionRefreshError> {
        let saved: HashSet<&str> = saved_credentials
            .iter()
            .map(CredentialRef::as_str)
            .collect();
        let mut deleted = HashSet::new();
        for node in old_nodes {
            if saved.contains(node.credential_ref.as_str())
                || !deleted.insert(node.credential_ref.as_str())
            {
                continue;
            }
            self.secret_store
                .delete(&node.credential_ref)
                .map_err(|source| SubscriptionRefreshError::SecretStore {
                    operation: SubscriptionSecretOperation::DeleteOldNode,
                    source,
                })?;
        }
        Ok(())
    }
}

fn incoming_candidates(
    parsed: Vec<ParsedSubscriptionNode>,
) -> Result<(Vec<NodeDedupCandidate>, PendingCredentials), SubscriptionRefreshError> {
    let mut candidates = Vec::with_capacity(parsed.len());
    let mut credentials = HashMap::with_capacity(parsed.len());
    for parsed_node in parsed {
        let (node, credential) = parsed_node.into_parts();
        let secret = CredentialCodec::encode(&credential)?;
        candidates.push(NodeDedupCandidate::new(
            node.clone(),
            CredentialIdentity::from_bytes(secret.expose_secret()),
        ));
        credentials.insert(
            node.credential_ref.as_str().to_owned(),
            (node.credential_ref, secret),
        );
    }
    Ok((candidates, credentials))
}

fn take_reconciled_credentials(
    nodes: &[ProxyNode],
    credentials: &mut PendingCredentials,
) -> Vec<PendingCredential> {
    nodes
        .iter()
        .map(|node| {
            credentials
                .remove(node.credential_ref.as_str())
                .expect("every reconciled node originated from a parsed credential")
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriptionSecretOperation {
    ReadUrl,
    ReadExistingNode,
    SaveNewNode,
    RollbackNewNode,
    DeleteOldNode,
}

impl Display for SubscriptionSecretOperation {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadUrl => formatter.write_str("read subscription URL"),
            Self::ReadExistingNode => formatter.write_str("read existing node credential"),
            Self::SaveNewNode => formatter.write_str("save new node credential"),
            Self::RollbackNewNode => formatter.write_str("roll back new node credential"),
            Self::DeleteOldNode => formatter.write_str("delete old node credential"),
        }
    }
}

#[derive(Debug, Error)]
pub enum SubscriptionRefreshError {
    #[error(transparent)]
    Transaction(#[from] SubscriptionTransactionError),
    #[error("subscription URL secret is not valid UTF-8")]
    InvalidUrlSecret { source: str::Utf8Error },
    #[error(transparent)]
    Url(#[from] SubscriptionUrlError),
    #[error("subscription credential store failed while attempting to {operation}")]
    SecretStore {
        operation: SubscriptionSecretOperation,
        source: SecretStoreError,
    },
    #[error(transparent)]
    Fetch(#[from] SubscriptionFetchError),
    #[error("subscription content is invalid")]
    Content {
        #[from]
        source: SubscriptionContentError,
    },
    #[error("subscription node credential is invalid")]
    Credential {
        #[from]
        source: CredentialCodecError,
    },
}

fn merge_subscription_contents(parts: &[Vec<u8>]) -> Vec<u8> {
    let separator_count = parts.len().saturating_sub(1);
    let total = parts.iter().map(Vec::len).sum::<usize>() + separator_count;
    let mut merged = Vec::with_capacity(total);
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            merged.push(b'\n');
        }
        merged.extend_from_slice(part);
    }
    merged
}
