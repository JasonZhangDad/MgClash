use magies_domain::{CredentialRef, ProxyNode, ProxyProtocol};
use magies_profiles::{NodeGroupStoreError, SqliteNodeGroupStore};
use uuid::Uuid;

#[test]
fn persists_named_groups_and_assigns_them_across_node_sources() {
    let database_path =
        std::env::temp_dir().join(format!("mgclash-node-groups-{}.sqlite", Uuid::new_v4()));
    let manual = node(1, "Manual", None);
    let managed = node(2, "Managed", Some(Uuid::from_u128(10)));

    let group_id = {
        let mut store = SqliteNodeGroupStore::open(&database_path).unwrap();
        let manual_group = store.assign(manual.id, Some(" Work ")).unwrap().unwrap();
        let managed_group = store.assign(managed.id, Some("Work")).unwrap().unwrap();
        assert_eq!(manual_group, managed_group);
        assert_eq!(manual_group.name, "Work");
        manual_group.id
    };

    let mut nodes = vec![manual, managed];
    let mut store = SqliteNodeGroupStore::open(&database_path).unwrap();
    store.apply(&mut nodes).unwrap();
    assert_eq!(nodes[0].group_id, Some(group_id));
    assert_eq!(nodes[1].group_id, Some(group_id));
    assert_eq!(store.groups().unwrap()[0].name, "Work");

    assert_eq!(store.assign(nodes[0].id, None).unwrap(), None);
    store.apply(&mut nodes).unwrap();
    assert_eq!(nodes[0].group_id, None);
    assert_eq!(nodes[1].group_id, Some(group_id));
    drop(store);
    std::fs::remove_file(database_path).unwrap();
}

#[test]
fn rejects_blank_group_names_without_changing_the_assignment() {
    let mut store = SqliteNodeGroupStore::open_in_memory().unwrap();
    let node_id = Uuid::from_u128(1);
    let group = store.assign(node_id, Some("Home")).unwrap().unwrap();

    assert!(matches!(
        store.assign(node_id, Some("  ")),
        Err(NodeGroupStoreError::EmptyName)
    ));
    let mut nodes = vec![node(1, "Node", None)];
    store.apply(&mut nodes).unwrap();
    assert_eq!(nodes[0].group_id, Some(group.id));
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
