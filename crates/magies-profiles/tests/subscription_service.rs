use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use magies_domain::{CredentialRef, NodeName, Subscription, TimestampMillis};
use magies_profiles::{
    CredentialCodec, ShareLinkParser, SqliteSubscriptionStore, SubscriptionFetchOptions,
    SubscriptionFetcher, SubscriptionRefreshError, SubscriptionRefreshOutcome,
    SubscriptionRefreshService, SubscriptionTransactionError, SubscriptionUpdate,
    SubscriptionValidators,
};
use magies_storage::{MemorySecretStore, SecretStore, SecretStoreError, SecretValue};
use uuid::Uuid;

const SUBSCRIPTION_ID: &str = "018f78b5-2cd0-7000-a9a6-3bccf60951e8";
const EXISTING_NODE_ID: &str = "018f78b5-2cd0-7000-a9a6-3bccf60951e9";
const URL_REF: &str = "subscription/primary/url";
const OLD_CREDENTIAL_REF: &str = "subscription/primary/node/old";
const LAST_MODIFIED: &str = "Sun, 09 Aug 2026 12:00:00 GMT";
const MATCHING_LINK: &str =
    "trojan://same-password@same.example.com:443?security=tls#Remote%20name";

#[test]
fn refreshes_and_reconciles_nodes_before_replacing_the_snapshot() {
    let body = format!(
        "{MATCHING_LINK}\n{MATCHING_LINK}\n{}",
        "trojan://new-password@new.example.com:443?security=tls#New"
    );
    let server = TestServer::spawn(response(
        "200 OK",
        &[("ETag", "\"revision-2\""), ("Last-Modified", LAST_MODIFIED)],
        body.as_bytes(),
    ));
    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let secrets = MemorySecretStore::default();
    let subscription_id = subscription_id();
    insert_subscription(&store, &secrets, &server.url("/list?token=url-secret"));
    let old_credential_ref = credential_ref(OLD_CREDENTIAL_REF);
    let (mut existing_node, existing_secret) = parsed_node(
        MATCHING_LINK,
        uuid(EXISTING_NODE_ID),
        old_credential_ref.clone(),
    );
    existing_node.subscription_id = Some(subscription_id);
    existing_node.name = NodeName::new("Pinned local name").unwrap();
    existing_node.enabled = false;
    existing_node.latency_ms = Some(42);
    secrets.put(&old_credential_ref, &existing_secret).unwrap();
    store
        .apply_update(
            &SubscriptionUpdate::new(
                subscription_id,
                vec![existing_node],
                SubscriptionValidators::new(
                    Some("\"revision-1\"".to_owned()),
                    Some(LAST_MODIFIED.to_owned()),
                ),
                TimestampMillis::new(100),
            )
            .unwrap(),
        )
        .unwrap();

    let outcome = refresh(&mut store, &secrets, TimestampMillis::new(200)).unwrap();

    assert_eq!(
        outcome,
        SubscriptionRefreshOutcome::Updated {
            added_nodes: 1,
            updated_nodes: 1,
            removed_nodes: 0,
            deduplicated_nodes: 1,
        }
    );
    let nodes = store.subscription_nodes(subscription_id).unwrap();
    assert_eq!(nodes.len(), 2);
    let retained = nodes
        .iter()
        .find(|node| node.id == uuid(EXISTING_NODE_ID))
        .unwrap();
    assert_eq!(retained.name.as_str(), "Pinned local name");
    assert!(!retained.enabled);
    assert_eq!(retained.latency_ms, Some(42));
    assert_ne!(retained.credential_ref, old_credential_ref);
    for node in &nodes {
        secrets.get(&node.credential_ref).unwrap();
    }
    assert!(matches!(
        secrets.get(&credential_ref(OLD_CREDENTIAL_REF)),
        Err(SecretStoreError::NotFound)
    ));
    assert_eq!(
        store
            .subscription_state(subscription_id)
            .unwrap()
            .unwrap()
            .last_updated_at,
        Some(TimestampMillis::new(200))
    );
    let request = server.finish().to_ascii_lowercase();
    assert!(request.contains("if-none-match: \"revision-1\"\r\n"));
    assert!(request.contains(&format!(
        "if-modified-since: {}\r\n",
        LAST_MODIFIED.to_ascii_lowercase()
    )));
}

#[test]
fn not_modified_refresh_only_touches_fetch_state() {
    let server = TestServer::spawn(response("304 Not Modified", &[], b""));
    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let secrets = MemorySecretStore::default();
    insert_subscription(&store, &secrets, &server.url("/list"));
    store
        .touch_subscription(
            subscription_id(),
            &SubscriptionValidators::new(
                Some("\"revision-1\"".to_owned()),
                Some(LAST_MODIFIED.to_owned()),
            ),
            TimestampMillis::new(100),
        )
        .unwrap();

    let outcome = refresh(&mut store, &secrets, TimestampMillis::new(200)).unwrap();

    assert_eq!(outcome, SubscriptionRefreshOutcome::NotModified);
    let state = store
        .subscription_state(subscription_id())
        .unwrap()
        .unwrap();
    assert_eq!(state.validators.etag(), Some("\"revision-1\""));
    assert_eq!(state.validators.last_modified(), Some(LAST_MODIFIED));
    assert_eq!(state.last_updated_at, Some(TimestampMillis::new(200)));
    assert!(
        store
            .subscription_nodes(subscription_id())
            .unwrap()
            .is_empty()
    );
    server.finish();
}

#[test]
fn invalid_content_keeps_the_previous_snapshot_and_fetch_state() {
    let server = TestServer::spawn(response("200 OK", &[("ETag", "\"bad\"")], b"not a link"));
    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let secrets = MemorySecretStore::default();
    insert_subscription(&store, &secrets, &server.url("/list?token=url-secret"));
    let old_credential_ref = credential_ref(OLD_CREDENTIAL_REF);
    let (mut existing_node, existing_secret) = parsed_node(
        MATCHING_LINK,
        uuid(EXISTING_NODE_ID),
        old_credential_ref.clone(),
    );
    existing_node.subscription_id = Some(subscription_id());
    secrets.put(&old_credential_ref, &existing_secret).unwrap();
    store
        .apply_update(
            &SubscriptionUpdate::new(
                subscription_id(),
                vec![existing_node.clone()],
                SubscriptionValidators::new(Some("\"revision-1\"".to_owned()), None),
                TimestampMillis::new(100),
            )
            .unwrap(),
        )
        .unwrap();

    let error = refresh(&mut store, &secrets, TimestampMillis::new(200)).unwrap_err();

    assert!(matches!(error, SubscriptionRefreshError::Content { .. }));
    assert_eq!(
        store.subscription_nodes(subscription_id()).unwrap(),
        vec![existing_node]
    );
    let state = store
        .subscription_state(subscription_id())
        .unwrap()
        .unwrap();
    assert_eq!(state.validators.etag(), Some("\"revision-1\""));
    assert_eq!(state.last_updated_at, Some(TimestampMillis::new(100)));
    secrets.get(&old_credential_ref).unwrap();
    let debug_output = format!("{error:?}");
    assert!(!debug_output.contains("url-secret"));
    server.finish();
}

#[test]
fn missing_subscription_fails_before_reading_any_secret() {
    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let secrets = MemorySecretStore::default();

    let error = refresh(&mut store, &secrets, TimestampMillis::new(200)).unwrap_err();

    assert!(matches!(
        error,
        SubscriptionRefreshError::Transaction(
            SubscriptionTransactionError::SubscriptionNotFound { .. }
        )
    ));
}

#[test]
fn credential_write_failure_rolls_back_new_secrets_and_preserves_the_database() {
    let body = b"trojan://first@first.example.com:443?security=tls#First\n\
                 trojan://second@second.example.com:443?security=tls#Second";
    let server = TestServer::spawn(response("200 OK", &[], body));
    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let secrets = FailingSecretStore::default();
    insert_subscription_with(&store, &secrets, &server.url("/list"));
    secrets.fail_put_number(2);

    let error = refresh_with(&mut store, &secrets, TimestampMillis::new(200)).unwrap_err();

    assert!(matches!(
        error,
        SubscriptionRefreshError::SecretStore {
            operation: magies_profiles::SubscriptionSecretOperation::SaveNewNode,
            source: SecretStoreError::EmptySecret,
        }
    ));
    assert_eq!(secrets.entry_count(), 1);
    secrets.get(&credential_ref(URL_REF)).unwrap();
    assert!(
        store
            .subscription_nodes(subscription_id())
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store
            .subscription_state(subscription_id())
            .unwrap()
            .unwrap()
            .last_updated_at,
        None
    );
    server.finish();
}

#[test]
fn invalid_url_secret_fails_without_contacting_the_network() {
    let mut store = SqliteSubscriptionStore::open_in_memory().unwrap();
    let secrets = MemorySecretStore::default();
    let url_ref = credential_ref(URL_REF);
    secrets
        .put(&url_ref, &SecretValue::new(vec![0xff]).unwrap())
        .unwrap();
    store
        .insert_subscription(&Subscription::new(subscription_id(), "Primary", url_ref, 60).unwrap())
        .unwrap();

    let error = refresh(&mut store, &secrets, TimestampMillis::new(200)).unwrap_err();

    assert!(matches!(
        error,
        SubscriptionRefreshError::InvalidUrlSecret { .. }
    ));
}

fn refresh(
    store: &mut SqliteSubscriptionStore,
    secrets: &MemorySecretStore,
    updated_at: TimestampMillis,
) -> Result<SubscriptionRefreshOutcome, SubscriptionRefreshError> {
    refresh_with(store, secrets, updated_at)
}

fn refresh_with<S: SecretStore>(
    store: &mut SqliteSubscriptionStore,
    secrets: &S,
    updated_at: TimestampMillis,
) -> Result<SubscriptionRefreshOutcome, SubscriptionRefreshError> {
    let fetcher = SubscriptionFetcher::new(SubscriptionFetchOptions::default()).unwrap();
    let mut service = SubscriptionRefreshService::new(store, secrets, fetcher);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(service.refresh(subscription_id(), updated_at))
}

fn insert_subscription(store: &SqliteSubscriptionStore, secrets: &MemorySecretStore, url: &str) {
    insert_subscription_with(store, secrets, url);
}

fn insert_subscription_with<S: SecretStore>(
    store: &SqliteSubscriptionStore,
    secrets: &S,
    url: &str,
) {
    let url_ref = credential_ref(URL_REF);
    secrets
        .put(
            &url_ref,
            &SecretValue::new(url.as_bytes().to_vec()).unwrap(),
        )
        .unwrap();
    store
        .insert_subscription(&Subscription::new(subscription_id(), "Primary", url_ref, 60).unwrap())
        .unwrap();
}

#[derive(Default)]
struct FailingSecretStore {
    entries: Mutex<HashMap<String, SecretValue>>,
    put_count: AtomicUsize,
    failed_put: AtomicUsize,
}

impl FailingSecretStore {
    fn fail_put_number(&self, number: usize) {
        self.put_count.store(0, Ordering::Relaxed);
        self.failed_put.store(number, Ordering::Relaxed);
    }

    fn entry_count(&self) -> usize {
        self.entries.lock().unwrap().len()
    }
}

impl SecretStore for FailingSecretStore {
    fn put(
        &self,
        credential_ref: &CredentialRef,
        secret: &SecretValue,
    ) -> Result<(), SecretStoreError> {
        let number = self.put_count.fetch_add(1, Ordering::Relaxed) + 1;
        if number == self.failed_put.load(Ordering::Relaxed) {
            return Err(SecretStoreError::EmptySecret);
        }
        self.entries
            .lock()
            .unwrap()
            .insert(credential_ref.as_str().to_owned(), secret.clone());
        Ok(())
    }

    fn get(&self, credential_ref: &CredentialRef) -> Result<SecretValue, SecretStoreError> {
        self.entries
            .lock()
            .unwrap()
            .get(credential_ref.as_str())
            .cloned()
            .ok_or(SecretStoreError::NotFound)
    }

    fn delete(&self, credential_ref: &CredentialRef) -> Result<(), SecretStoreError> {
        self.entries
            .lock()
            .unwrap()
            .remove(credential_ref.as_str())
            .map_or(Err(SecretStoreError::NotFound), |_| Ok(()))
    }
}

fn parsed_node(
    link: &str,
    id: Uuid,
    credential_ref: CredentialRef,
) -> (magies_domain::ProxyNode, SecretValue) {
    let parsed = ShareLinkParser.parse(link, id, credential_ref).unwrap();
    let (node, credential) = parsed.into_parts();
    let secret = CredentialCodec::encode(&credential).unwrap();
    (node, secret)
}

fn subscription_id() -> Uuid {
    uuid(SUBSCRIPTION_ID)
}

fn uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).unwrap()
}

fn credential_ref(value: &str) -> CredentialRef {
    CredentialRef::new(value).unwrap()
}

struct TestServer {
    address: std::net::SocketAddr,
    request: Receiver<String>,
    handle: JoinHandle<()>,
}

impl TestServer {
    fn spawn(response: Vec<u8>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, request) = mpsc::channel();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            sender.send(read_request(&mut stream)).unwrap();
            stream.write_all(&response).unwrap();
        });
        Self {
            address,
            request,
            handle,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }

    fn finish(self) -> String {
        self.handle.join().unwrap();
        self.request.recv().unwrap()
    }
}

fn response(status: &str, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    )
    .into_bytes();
    for (name, value) in headers {
        response.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
    }
    response.extend_from_slice(b"\r\n");
    response.extend_from_slice(body);
    response
}

fn read_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        let count = stream.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..count]);
    }
    String::from_utf8(request).unwrap()
}
