use magies_domain::{CredentialRef, Subscription, SubscriptionModelError, TimestampMillis};
use uuid::Uuid;

#[test]
fn constructs_and_round_trips_a_valid_subscription() {
    let id = uuid("018f78b5-2cd0-7000-a9a6-3bccf60951e8");
    let secret_ref = CredentialRef::new("keychain://subscription/url-secret").unwrap();
    let mut subscription = Subscription::new(id, "  Daily nodes  ", secret_ref, 60).unwrap();
    subscription.auto_update = true;
    subscription.last_updated_at = Some(TimestampMillis::new(1_786_291_200_000));
    subscription.etag = Some("\"revision-1\"".to_owned());
    subscription.last_modified = Some("Sun, 09 Aug 2026 12:00:00 GMT".to_owned());

    assert_eq!(subscription.id, id);
    assert_eq!(subscription.name.as_str(), "Daily nodes");
    assert_eq!(subscription.update_interval_minutes.get(), 60);
    assert!(subscription.auto_update);
    assert!(subscription.enabled);

    let json = serde_json::to_string(&subscription).unwrap();
    let decoded: Subscription = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, subscription);
    assert!(!format!("{subscription:?}").contains("url-secret"));
}

#[test]
fn rejects_invalid_subscription_fields_and_deserialization() {
    let id = uuid("018f78b5-2cd0-7000-a9a6-3bccf60951e8");
    let empty_name = Subscription::new(
        id,
        "   ",
        CredentialRef::new("keychain://subscription/url").unwrap(),
        60,
    )
    .unwrap_err();
    let zero_interval = Subscription::new(
        id,
        "Nodes",
        CredentialRef::new("keychain://subscription/url").unwrap(),
        0,
    )
    .unwrap_err();
    let invalid_json = serde_json::json!({
        "id": id,
        "name": "",
        "urlSecretRef": "keychain://subscription/url",
        "updateIntervalMinutes": 60,
        "autoUpdate": false,
        "lastUpdatedAt": null,
        "etag": null,
        "lastModified": null,
        "enabled": true
    });

    assert_eq!(empty_name, SubscriptionModelError::EmptyName);
    assert_eq!(
        zero_interval,
        SubscriptionModelError::InvalidUpdateInterval { minutes: 0 }
    );
    assert!(serde_json::from_value::<Subscription>(invalid_json).is_err());
}

fn uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).unwrap()
}
