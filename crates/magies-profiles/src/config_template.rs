//! Applying a user-supplied Core config template.
//!
//! The template is a JSON Merge Patch (RFC 7386) over what the generators
//! produced, rather than a document that replaces them. That is what lets a
//! user carry a setting the app does not model without also taking over
//! everything it does model — and what keeps a template from silently going
//! stale when a generator gains a field.
//!
//! See ADR 0005 for why raw config access exists at all.

use serde_json::Value;

/// Applies `template` to `document` in place, per RFC 7386.
///
/// Objects merge key by key; anything else replaces what it lands on, and a
/// `null` removes the key it names.
pub fn apply_config_template(document: &mut Value, template: &Value) {
    let Some(patch) = template.as_object() else {
        *document = template.clone();
        return;
    };
    // Taken rather than borrowed: a patch onto something that is not an object
    // replaces it, and taking makes that the same code path as merging.
    let mut target = match document.take() {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    for (key, value) in patch {
        if value.is_null() {
            target.remove(key);
        } else {
            apply_config_template(target.entry(key.clone()).or_insert(Value::Null), value);
        }
    }
    *document = Value::Object(target);
}
