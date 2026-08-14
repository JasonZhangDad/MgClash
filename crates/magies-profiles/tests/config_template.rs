//! The user-supplied Core config template.
//!
//! A template is a JSON Merge Patch (RFC 7386) applied to what the generators
//! produced, which gives three things a config format has to have: adding a key
//! the app does not model, changing one it does, and removing one it emitted.

use magies_profiles::apply_config_template;
use serde_json::json;

#[test]
fn adds_a_section_the_generators_do_not_model() {
    let mut document = json!({ "log": { "level": "warn" }, "outbounds": [] });

    apply_config_template(
        &mut document,
        &json!({ "experimental": { "cache_file": { "enabled": true } } }),
    );

    assert_eq!(
        document,
        json!({
            "log": { "level": "warn" },
            "outbounds": [],
            "experimental": { "cache_file": { "enabled": true } },
        })
    );
}

#[test]
fn changes_one_field_without_restating_the_object_it_lives_in() {
    let mut document = json!({ "log": { "level": "warn", "timestamp": true } });

    apply_config_template(&mut document, &json!({ "log": { "level": "debug" } }));

    // The sibling survives: a patch that replaced whole objects would make the
    // template a copy of the generator's output, kept in sync by hand.
    assert_eq!(
        document,
        json!({ "log": { "level": "debug", "timestamp": true } })
    );
}

#[test]
fn removes_a_generated_key_with_null() {
    let mut document = json!({ "log": { "level": "warn" }, "experimental": { "clash_api": {} } });

    apply_config_template(&mut document, &json!({ "experimental": null }));

    assert_eq!(document, json!({ "log": { "level": "warn" } }));
}

#[test]
fn replaces_an_array_rather_than_appending_to_it() {
    let mut document = json!({ "inbounds": [{ "type": "socks" }, { "type": "http" }] });

    apply_config_template(&mut document, &json!({ "inbounds": [{ "type": "mixed" }] }));

    // There is no honest way to merge two arrays element-wise: position is not
    // identity, and the caller means "these inbounds", not "these as well".
    assert_eq!(document, json!({ "inbounds": [{ "type": "mixed" }] }));
}

#[test]
fn an_empty_template_changes_nothing() {
    let generated = json!({ "log": { "level": "warn" }, "outbounds": [{ "tag": "proxy" }] });
    let mut document = generated.clone();

    apply_config_template(&mut document, &json!({}));

    assert_eq!(document, generated);
}

#[test]
fn a_template_that_is_not_an_object_replaces_nothing_it_cannot_describe() {
    let mut document = json!({ "log": { "level": "warn" } });

    // A scalar patch at the root would mean "the config is now the string
    // 'oops'", which no Core would accept. It is refused earlier, by the
    // command that parses it; here the merge simply does what RFC 7386 says.
    apply_config_template(&mut document, &json!("oops"));

    assert_eq!(document, json!("oops"));
}
