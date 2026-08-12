use magies_domain::{CredentialRef, ProxyNode, ProxyProtocol};
use magies_profiles::{NodeOrderStoreError, SqliteNodeOrderStore};
use uuid::Uuid;

#[test]
fn persists_one_order_for_manual_and_subscription_nodes() {
    let database_path =
        std::env::temp_dir().join(format!("mgclash-node-order-{}.sqlite", Uuid::new_v4()));
    let manual = node(1, "Manual", None);
    let subscription_id = Uuid::from_u128(10);
    let managed = node(2, "Managed", Some(subscription_id));
    let new_node = node(3, "New", None);

    {
        let mut store = SqliteNodeOrderStore::open(&database_path).unwrap();
        assert_eq!(
            store
                .order_nodes(vec![manual.clone(), managed.clone()])
                .unwrap(),
            vec![manual.clone(), managed.clone()]
        );
        store.save(&[managed.id, manual.id]).unwrap();
    }

    let store = SqliteNodeOrderStore::open(&database_path).unwrap();
    assert_eq!(
        store
            .order_nodes(vec![manual.clone(), new_node.clone(), managed.clone()])
            .unwrap(),
        vec![managed, manual, new_node]
    );
    drop(store);
    std::fs::remove_file(database_path).unwrap();
}

#[test]
fn rejects_duplicate_ids_without_replacing_the_saved_order() {
    let mut store = SqliteNodeOrderStore::open_in_memory().unwrap();
    let first = node(1, "First", None);
    let second = node(2, "Second", None);
    store.save(&[second.id, first.id]).unwrap();

    assert!(matches!(
        store.save(&[first.id, first.id]),
        Err(NodeOrderStoreError::DuplicateNode { id }) if id == first.id
    ));
    assert_eq!(
        store
            .order_nodes(vec![first.clone(), second.clone()])
            .unwrap(),
        vec![second, first]
    );
}

fn node(value: u128, name: &str, subscription_id: Option<Uuid>) -> ProxyNode {
    let id = Uuid::from_u128(value);
    let mut node = ProxyNode::new(
        id,
        name,
        ProxyProtocol::Shadowsocks,
        "edge.example.com",
        8_388,
        Some(CredentialRef::new(format!("node/{id}")).unwrap()),
    )
    .unwrap();
    node.subscription_id = subscription_id;
    node
}
