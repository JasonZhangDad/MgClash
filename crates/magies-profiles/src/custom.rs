//! Custom full Core JSON credentials (v2rayN `AddServer2` style).
//!
//! The user supplies a complete sing-box or Xray runtime document; the session
//! writes it verbatim and skips the usual outbound / DNS / route generators.

use std::fmt::{Debug, Formatter};

use magies_domain::CoreType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomCredential {
    pub core: CoreType,
    pub document: String,
}

impl CustomCredential {
    #[must_use]
    pub const fn core(&self) -> CoreType {
        self.core
    }

    #[must_use]
    pub fn document(&self) -> &str {
        &self.document
    }
}

impl Debug for CustomCredential {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CustomCredential([REDACTED])")
    }
}
