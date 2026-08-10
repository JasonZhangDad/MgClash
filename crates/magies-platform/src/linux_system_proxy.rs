use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::system_proxy::{PacSetting, ProxyEndpoint, ProxySetting, SystemProxyState};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum GnomeProxyMode {
    None,
    Manual,
    Auto,
}

impl GnomeProxyMode {
    #[cfg(target_os = "linux")]
    fn parse(value: &str) -> Result<Self, LinuxSystemProxyError> {
        match value {
            "none" => Ok(Self::None),
            "manual" => Ok(Self::Manual),
            "auto" => Ok(Self::Auto),
            _ => Err(LinuxSystemProxyError::MalformedSnapshot),
        }
    }

    #[cfg(target_os = "linux")]
    const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Manual => "manual",
            Self::Auto => "auto",
        }
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
struct GnomeProxyEndpoint {
    host: String,
    port: u16,
}

impl GnomeProxyEndpoint {
    const fn disabled() -> Self {
        Self {
            host: String::new(),
            port: 0,
        }
    }

    fn to_proxy_setting(&self, enabled: bool) -> Result<ProxySetting, LinuxSystemProxyError> {
        let endpoint = match (self.host.trim().is_empty(), self.port) {
            (true, 0) => None,
            (false, port @ 1..=u16::MAX) => Some(
                ProxyEndpoint::new(&self.host, port)
                    .map_err(|_| LinuxSystemProxyError::MalformedSnapshot)?,
            ),
            (true, _) | (false, 0) => return Err(LinuxSystemProxyError::MalformedSnapshot),
        };
        ProxySetting::new(enabled && endpoint.is_some(), endpoint)
            .map_err(|_| LinuxSystemProxyError::MalformedSnapshot)
    }

    fn from_setting(setting: &ProxySetting) -> Self {
        setting
            .endpoint()
            .map_or_else(Self::disabled, |endpoint| Self {
                host: endpoint.host().to_owned(),
                port: endpoint.port(),
            })
    }
}

impl Debug for GnomeProxyEndpoint {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GnomeProxyEndpoint")
            .field("host", &(!self.host.is_empty()).then_some("configured"))
            .field("port", &self.port)
            .finish()
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
struct GnomeHttpProxy {
    enabled: bool,
    endpoint: GnomeProxyEndpoint,
    use_authentication: bool,
    authentication_user: String,
    authentication_password: String,
}

impl Debug for GnomeHttpProxy {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GnomeHttpProxy")
            .field("enabled", &self.enabled)
            .field("endpoint", &self.endpoint)
            .field("use_authentication", &self.use_authentication)
            .field(
                "authentication_user",
                &(!self.authentication_user.is_empty()).then_some("configured"),
            )
            .field(
                "authentication_password",
                &(!self.authentication_password.is_empty()).then_some("configured"),
            )
            .finish()
    }
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct LinuxSystemProxySnapshot {
    mode: GnomeProxyMode,
    autoconfig_url: String,
    ignore_hosts: Vec<String>,
    use_same_proxy: bool,
    http: GnomeHttpProxy,
    https: GnomeProxyEndpoint,
    ftp: GnomeProxyEndpoint,
    socks: GnomeProxyEndpoint,
}

impl LinuxSystemProxySnapshot {
    /// Converts a GNOME proxy snapshot into the shared proxy state model.
    ///
    /// # Errors
    ///
    /// Returns an error for a partial endpoint or enabled PAC without a URL.
    pub fn to_system_proxy_state(&self) -> Result<SystemProxyState, LinuxSystemProxyError> {
        let manual = self.mode == GnomeProxyMode::Manual;
        let http = self.http.endpoint.to_proxy_setting(manual)?;
        let https = self.https.to_proxy_setting(manual)?;
        let socks = self.socks.to_proxy_setting(manual)?;
        let pac = PacSetting::new(
            self.mode == GnomeProxyMode::Auto,
            (!self.autoconfig_url.trim().is_empty()).then(|| self.autoconfig_url.clone()),
        )
        .map_err(|_| LinuxSystemProxyError::MalformedSnapshot)?;
        Ok(SystemProxyState::new(http, https, socks, pac))
    }

    fn with_state(&self, state: &SystemProxyState) -> Result<Self, LinuxSystemProxyError> {
        let settings = [state.http(), state.https(), state.socks()];
        let manual = settings.iter().any(|setting| setting.enabled());
        if manual && state.pac().enabled() {
            return Err(LinuxSystemProxyError::UnrepresentableProxyState);
        }
        if manual
            && settings
                .iter()
                .any(|setting| !setting.enabled() && setting.endpoint().is_some())
        {
            return Err(LinuxSystemProxyError::UnrepresentableProxyState);
        }
        let mode = if manual {
            GnomeProxyMode::Manual
        } else if state.pac().enabled() {
            GnomeProxyMode::Auto
        } else {
            GnomeProxyMode::None
        };
        let http_endpoint = GnomeProxyEndpoint::from_setting(state.http());

        Ok(Self {
            mode,
            autoconfig_url: state.pac().url().unwrap_or_default().to_owned(),
            ignore_hosts: self.ignore_hosts.clone(),
            use_same_proxy: false,
            http: GnomeHttpProxy {
                enabled: state.http().enabled(),
                endpoint: http_endpoint,
                use_authentication: false,
                authentication_user: String::new(),
                authentication_password: String::new(),
            },
            https: GnomeProxyEndpoint::from_setting(state.https()),
            ftp: GnomeProxyEndpoint::disabled(),
            socks: GnomeProxyEndpoint::from_setting(state.socks()),
        })
    }

    #[must_use]
    pub const fn mode(&self) -> GnomeProxyMode {
        self.mode
    }

    #[must_use]
    pub fn ignore_hosts(&self) -> &[String] {
        &self.ignore_hosts
    }
}

impl Debug for LinuxSystemProxySnapshot {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LinuxSystemProxySnapshot")
            .field("mode", &self.mode)
            .field(
                "autoconfig_url",
                &(!self.autoconfig_url.is_empty()).then_some("configured"),
            )
            .field("ignore_hosts", &self.ignore_hosts)
            .field("use_same_proxy", &self.use_same_proxy)
            .field("http", &self.http)
            .field("https", &self.https)
            .field("ftp", &self.ftp)
            .field("socks", &self.socks)
            .finish()
    }
}

pub struct LinuxSystemProxyAdapter {
    backend: Arc<dyn SettingsBackend>,
}

impl LinuxSystemProxyAdapter {
    /// Creates a GNOME/GSettings System Proxy Adapter.
    ///
    /// # Errors
    ///
    /// Returns an error outside Linux or when the GNOME proxy schema is absent.
    pub fn new() -> Result<Self, LinuxSystemProxyError> {
        #[cfg(target_os = "linux")]
        {
            Ok(Self::with_backend(Arc::new(GnomeSettingsBackend::new()?)))
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(LinuxSystemProxyError::UnsupportedPlatform)
        }
    }

    #[cfg(any(test, target_os = "linux"))]
    fn with_backend(backend: Arc<dyn SettingsBackend>) -> Self {
        Self { backend }
    }

    /// Reads an exact GNOME System Proxy snapshot.
    ///
    /// # Errors
    ///
    /// Returns a typed backend or malformed snapshot error.
    pub fn read_snapshot(&self) -> Result<LinuxSystemProxySnapshot, LinuxSystemProxyError> {
        self.backend.read()
    }

    /// Restores an exact GNOME System Proxy snapshot.
    ///
    /// # Errors
    ///
    /// Returns a typed backend error when a key is locked or cannot be written.
    pub fn apply_snapshot(
        &self,
        snapshot: &LinuxSystemProxySnapshot,
    ) -> Result<(), LinuxSystemProxyError> {
        self.backend.write(snapshot)
    }

    /// Reads GNOME settings into the shared proxy state model.
    ///
    /// # Errors
    ///
    /// Returns a typed backend or state mapping error.
    pub fn read(&self) -> Result<SystemProxyState, LinuxSystemProxyError> {
        self.read_snapshot()?.to_system_proxy_state()
    }

    /// Applies shared state while preserving the current GNOME ignore list.
    ///
    /// # Errors
    ///
    /// Returns an error for backend failures or modes GNOME cannot represent.
    pub fn apply(&self, state: &SystemProxyState) -> Result<(), LinuxSystemProxyError> {
        let current = self.read_snapshot()?;
        let updated = current.with_state(state)?;
        self.apply_snapshot(&updated)
    }
}

impl crate::system_proxy_recovery::SystemProxyControl for LinuxSystemProxyAdapter {
    type Error = LinuxSystemProxyError;
    type Snapshot = LinuxSystemProxySnapshot;

    fn capture(&self) -> Result<Self::Snapshot, Self::Error> {
        self.read_snapshot()
    }

    fn current(&self) -> Result<SystemProxyState, Self::Error> {
        Self::read(self)
    }

    fn apply(&self, state: &SystemProxyState) -> Result<(), Self::Error> {
        Self::apply(self, state)
    }

    fn restore(&self, snapshot: &Self::Snapshot) -> Result<(), Self::Error> {
        self.apply_snapshot(snapshot)
    }
}

trait SettingsBackend: Send + Sync {
    fn read(&self) -> Result<LinuxSystemProxySnapshot, LinuxSystemProxyError>;
    fn write(&self, snapshot: &LinuxSystemProxySnapshot) -> Result<(), LinuxSystemProxyError>;
}

#[cfg(target_os = "linux")]
struct GnomeSettingsBackend;

#[cfg(target_os = "linux")]
impl GnomeSettingsBackend {
    fn new() -> Result<Self, LinuxSystemProxyError> {
        use gio::SettingsSchemaSource;

        let source =
            SettingsSchemaSource::default().ok_or(LinuxSystemProxyError::SchemaUnavailable {
                schema: ROOT_SCHEMA,
            })?;
        for schema in [
            ROOT_SCHEMA,
            HTTP_SCHEMA,
            HTTPS_SCHEMA,
            FTP_SCHEMA,
            SOCKS_SCHEMA,
        ] {
            if source.lookup(schema, true).is_none() {
                return Err(LinuxSystemProxyError::SchemaUnavailable { schema });
            }
        }
        Ok(Self)
    }
}

#[cfg(target_os = "linux")]
impl SettingsBackend for GnomeSettingsBackend {
    fn read(&self) -> Result<LinuxSystemProxySnapshot, LinuxSystemProxyError> {
        use gio::prelude::*;

        let root = gio::Settings::new(ROOT_SCHEMA);
        let http = root.child("http");
        let https = root.child("https");
        let ftp = root.child("ftp");
        let socks = root.child("socks");
        Ok(LinuxSystemProxySnapshot {
            mode: GnomeProxyMode::parse(root.string("mode").as_str())?,
            autoconfig_url: root.string("autoconfig-url").to_string(),
            ignore_hosts: root
                .strv("ignore-hosts")
                .iter()
                .map(ToString::to_string)
                .collect(),
            use_same_proxy: root.boolean("use-same-proxy"),
            http: GnomeHttpProxy {
                enabled: http.boolean("enabled"),
                endpoint: read_endpoint(&http)?,
                use_authentication: http.boolean("use-authentication"),
                authentication_user: http.string("authentication-user").to_string(),
                authentication_password: http.string("authentication-password").to_string(),
            },
            https: read_endpoint(&https)?,
            ftp: read_endpoint(&ftp)?,
            socks: read_endpoint(&socks)?,
        })
    }

    fn write(&self, snapshot: &LinuxSystemProxySnapshot) -> Result<(), LinuxSystemProxyError> {
        use gio::prelude::*;

        let root = gio::Settings::new(ROOT_SCHEMA);
        root.delay();
        let http = root.child("http");
        let https = root.child("https");
        let ftp = root.child("ftp");
        let socks = root.child("socks");
        ensure_writable(
            &root,
            ROOT_SCHEMA,
            &["mode", "autoconfig-url", "ignore-hosts", "use-same-proxy"],
        )?;
        ensure_writable(
            &http,
            HTTP_SCHEMA,
            &[
                "enabled",
                "host",
                "port",
                "use-authentication",
                "authentication-user",
                "authentication-password",
            ],
        )?;
        for (settings, schema) in [
            (&https, HTTPS_SCHEMA),
            (&ftp, FTP_SCHEMA),
            (&socks, SOCKS_SCHEMA),
        ] {
            ensure_writable(settings, schema, &["host", "port"])?;
        }

        let result = (|| {
            set_string(
                &root,
                ROOT_SCHEMA,
                "autoconfig-url",
                &snapshot.autoconfig_url,
            )?;
            let ignore_hosts: Vec<&str> =
                snapshot.ignore_hosts.iter().map(String::as_str).collect();
            root.set_strv("ignore-hosts", ignore_hosts)
                .map_err(|source| write_error(ROOT_SCHEMA, "ignore-hosts", source))?;
            set_boolean(
                &root,
                ROOT_SCHEMA,
                "use-same-proxy",
                snapshot.use_same_proxy,
            )?;
            set_boolean(&http, HTTP_SCHEMA, "enabled", snapshot.http.enabled)?;
            write_endpoint(&http, HTTP_SCHEMA, &snapshot.http.endpoint)?;
            set_boolean(
                &http,
                HTTP_SCHEMA,
                "use-authentication",
                snapshot.http.use_authentication,
            )?;
            set_string(
                &http,
                HTTP_SCHEMA,
                "authentication-user",
                &snapshot.http.authentication_user,
            )?;
            set_string(
                &http,
                HTTP_SCHEMA,
                "authentication-password",
                &snapshot.http.authentication_password,
            )?;
            write_endpoint(&https, HTTPS_SCHEMA, &snapshot.https)?;
            write_endpoint(&ftp, FTP_SCHEMA, &snapshot.ftp)?;
            write_endpoint(&socks, SOCKS_SCHEMA, &snapshot.socks)?;
            set_string(&root, ROOT_SCHEMA, "mode", snapshot.mode.as_str())
        })();

        if let Err(error) = result {
            root.revert();
            return Err(error);
        }
        root.apply();
        gio::Settings::sync();
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn read_endpoint(settings: &gio::Settings) -> Result<GnomeProxyEndpoint, LinuxSystemProxyError> {
    use gio::prelude::*;

    let port = settings.int("port");
    let port = u16::try_from(port).map_err(|_| LinuxSystemProxyError::MalformedSnapshot)?;
    Ok(GnomeProxyEndpoint {
        host: settings.string("host").to_string(),
        port,
    })
}

#[cfg(target_os = "linux")]
fn ensure_writable(
    settings: &gio::Settings,
    schema: &'static str,
    keys: &[&'static str],
) -> Result<(), LinuxSystemProxyError> {
    use gio::prelude::*;

    for key in keys {
        if !settings.is_writable(key) {
            return Err(LinuxSystemProxyError::KeyNotWritable { schema, key });
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn write_endpoint(
    settings: &gio::Settings,
    schema: &'static str,
    endpoint: &GnomeProxyEndpoint,
) -> Result<(), LinuxSystemProxyError> {
    set_string(settings, schema, "host", &endpoint.host)?;
    set_int(settings, schema, "port", i32::from(endpoint.port))
}

#[cfg(target_os = "linux")]
fn set_string(
    settings: &gio::Settings,
    schema: &'static str,
    key: &'static str,
    value: &str,
) -> Result<(), LinuxSystemProxyError> {
    use gio::prelude::*;

    settings
        .set_string(key, value)
        .map_err(|source| write_error(schema, key, source))
}

#[cfg(target_os = "linux")]
fn set_boolean(
    settings: &gio::Settings,
    schema: &'static str,
    key: &'static str,
    value: bool,
) -> Result<(), LinuxSystemProxyError> {
    use gio::prelude::*;

    settings
        .set_boolean(key, value)
        .map_err(|source| write_error(schema, key, source))
}

#[cfg(target_os = "linux")]
fn set_int(
    settings: &gio::Settings,
    schema: &'static str,
    key: &'static str,
    value: i32,
) -> Result<(), LinuxSystemProxyError> {
    use gio::prelude::*;

    settings
        .set_int(key, value)
        .map_err(|source| write_error(schema, key, source))
}

#[cfg(target_os = "linux")]
fn write_error(
    schema: &'static str,
    key: &'static str,
    source: gio::glib::BoolError,
) -> LinuxSystemProxyError {
    LinuxSystemProxyError::GSettingsWrite {
        schema,
        key,
        source,
    }
}

#[cfg(target_os = "linux")]
const ROOT_SCHEMA: &str = "org.gnome.system.proxy";
#[cfg(target_os = "linux")]
const HTTP_SCHEMA: &str = "org.gnome.system.proxy.http";
#[cfg(target_os = "linux")]
const HTTPS_SCHEMA: &str = "org.gnome.system.proxy.https";
#[cfg(target_os = "linux")]
const FTP_SCHEMA: &str = "org.gnome.system.proxy.ftp";
#[cfg(target_os = "linux")]
const SOCKS_SCHEMA: &str = "org.gnome.system.proxy.socks";

#[derive(Debug)]
pub enum LinuxSystemProxyError {
    UnsupportedPlatform,
    SchemaUnavailable {
        schema: &'static str,
    },
    KeyNotWritable {
        schema: &'static str,
        key: &'static str,
    },
    #[cfg(target_os = "linux")]
    GSettingsWrite {
        schema: &'static str,
        key: &'static str,
        source: gio::glib::BoolError,
    },
    MalformedSnapshot,
    UnrepresentableProxyState,
}

impl Display for LinuxSystemProxyError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedPlatform => {
                formatter.write_str("GNOME System Proxy is only available on Linux")
            }
            Self::SchemaUnavailable { schema } => {
                write!(
                    formatter,
                    "required GSettings schema is unavailable: {schema}"
                )
            }
            Self::KeyNotWritable { schema, key } => {
                write!(formatter, "GSettings key is not writable: {schema} {key}")
            }
            #[cfg(target_os = "linux")]
            Self::GSettingsWrite {
                schema,
                key,
                source,
            } => write!(
                formatter,
                "failed to write GSettings key {schema} {key}: {source}"
            ),
            Self::MalformedSnapshot => {
                formatter.write_str("GNOME System Proxy snapshot is malformed")
            }
            Self::UnrepresentableProxyState => {
                formatter.write_str("GNOME cannot represent the requested System Proxy state")
            }
        }
    }
}

impl Error for LinuxSystemProxyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            #[cfg(target_os = "linux")]
            Self::GSettingsWrite { source, .. } => Some(source),
            Self::UnsupportedPlatform
            | Self::SchemaUnavailable { .. }
            | Self::KeyNotWritable { .. }
            | Self::MalformedSnapshot
            | Self::UnrepresentableProxyState => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::system_proxy::{PacSetting, ProxyEndpoint, ProxySetting, SystemProxyState};
    use crate::system_proxy_recovery::SystemProxyControl;

    #[test]
    fn adapter_satisfies_recovery_control_contract_and_snapshot_persistence() {
        fn assert_control<T: SystemProxyControl<Snapshot = LinuxSystemProxySnapshot>>(
            _control: &T,
        ) {
        }

        let backend = RecordingBackend::new(snapshot(GnomeProxyMode::Auto));
        let adapter = LinuxSystemProxyAdapter::with_backend(backend);
        assert_control(&adapter);
        let captured = snapshot(GnomeProxyMode::Auto);
        let encoded = serde_json::to_string(&captured).unwrap();
        assert_eq!(
            serde_json::from_str::<LinuxSystemProxySnapshot>(&encoded).unwrap(),
            captured
        );
    }

    struct RecordingBackend {
        snapshot: LinuxSystemProxySnapshot,
        writes: Mutex<Vec<LinuxSystemProxySnapshot>>,
    }

    impl RecordingBackend {
        fn new(snapshot: LinuxSystemProxySnapshot) -> Arc<Self> {
            Arc::new(Self {
                snapshot,
                writes: Mutex::new(Vec::new()),
            })
        }

        fn writes(&self) -> Vec<LinuxSystemProxySnapshot> {
            self.writes.lock().unwrap().clone()
        }
    }

    impl SettingsBackend for RecordingBackend {
        fn read(&self) -> Result<LinuxSystemProxySnapshot, LinuxSystemProxyError> {
            Ok(self.snapshot.clone())
        }

        fn write(&self, snapshot: &LinuxSystemProxySnapshot) -> Result<(), LinuxSystemProxyError> {
            self.writes.lock().unwrap().push(snapshot.clone());
            Ok(())
        }
    }

    #[test]
    fn manual_mode_maps_configured_endpoints_to_enabled_proxies() {
        let snapshot = snapshot(GnomeProxyMode::Manual);

        let state = snapshot.to_system_proxy_state().unwrap();

        assert!(state.http().enabled());
        assert_eq!(state.http().endpoint().unwrap().port(), 8080);
        assert!(state.https().enabled());
        assert!(state.socks().enabled());
        assert!(!state.pac().enabled());
        assert!(state.pac().url().is_some());
    }

    #[test]
    fn automatic_mode_enables_only_pac_and_preserves_manual_values() {
        let snapshot = snapshot(GnomeProxyMode::Auto);

        let state = snapshot.to_system_proxy_state().unwrap();

        assert!(!state.http().enabled());
        assert!(state.http().endpoint().is_some());
        assert!(!state.https().enabled());
        assert!(!state.socks().enabled());
        assert!(state.pac().enabled());
    }

    #[test]
    fn apply_preserves_ignore_hosts_and_clears_temporary_authentication_and_ftp() {
        let backend = RecordingBackend::new(snapshot(GnomeProxyMode::Auto));
        let adapter = LinuxSystemProxyAdapter::with_backend(backend.clone());
        let state = SystemProxyState::new(
            enabled("127.0.0.1", 10809),
            enabled("127.0.0.1", 10809),
            enabled("127.0.0.1", 10808),
            PacSetting::disabled(),
        );

        adapter.apply(&state).unwrap();

        let writes = backend.writes();
        assert_eq!(writes.len(), 1);
        let written = &writes[0];
        assert_eq!(written.mode, GnomeProxyMode::Manual);
        assert_eq!(written.ignore_hosts, ["localhost", "127.0.0.0/8"]);
        assert!(!written.use_same_proxy);
        assert_eq!(written.http.endpoint, endpoint("127.0.0.1", 10809));
        assert!(written.http.enabled);
        assert!(!written.http.use_authentication);
        assert!(written.http.authentication_user.is_empty());
        assert!(written.http.authentication_password.is_empty());
        assert_eq!(written.https, endpoint("127.0.0.1", 10809));
        assert_eq!(written.socks, endpoint("127.0.0.1", 10808));
        assert_eq!(written.ftp, GnomeProxyEndpoint::disabled());
        assert!(written.autoconfig_url.is_empty());
    }

    #[test]
    fn apply_rejects_simultaneous_manual_and_pac_modes() {
        let backend = RecordingBackend::new(snapshot(GnomeProxyMode::None));
        let adapter = LinuxSystemProxyAdapter::with_backend(backend.clone());
        let state = SystemProxyState::new(
            enabled("127.0.0.1", 10809),
            ProxySetting::disabled(),
            ProxySetting::disabled(),
            PacSetting::new(true, Some("https://example.test/proxy.pac".to_owned())).unwrap(),
        );

        assert!(matches!(
            adapter.apply(&state),
            Err(LinuxSystemProxyError::UnrepresentableProxyState)
        ));
        assert!(backend.writes().is_empty());
    }

    #[test]
    fn partial_endpoint_is_rejected_as_a_malformed_snapshot() {
        let mut snapshot = snapshot(GnomeProxyMode::Manual);
        snapshot.http.endpoint.port = 0;

        assert!(matches!(
            snapshot.to_system_proxy_state(),
            Err(LinuxSystemProxyError::MalformedSnapshot)
        ));
    }

    #[test]
    fn snapshot_debug_redacts_pac_proxy_and_authentication_values() {
        let snapshot = snapshot(GnomeProxyMode::Auto);

        let debug = format!("{snapshot:?}");

        assert!(debug.contains("configured"));
        assert!(debug.contains("localhost"));
        assert!(!debug.contains("pac-secret"));
        assert!(!debug.contains("proxy-secret"));
        assert!(!debug.contains("auth-secret"));
    }

    fn snapshot(mode: GnomeProxyMode) -> LinuxSystemProxySnapshot {
        LinuxSystemProxySnapshot {
            mode,
            autoconfig_url: "https://pac-secret@example.test/proxy.pac".to_owned(),
            ignore_hosts: vec!["localhost".to_owned(), "127.0.0.0/8".to_owned()],
            use_same_proxy: true,
            http: GnomeHttpProxy {
                enabled: true,
                endpoint: endpoint("proxy-secret.example", 8080),
                use_authentication: true,
                authentication_user: "saved-user".to_owned(),
                authentication_password: "auth-secret".to_owned(),
            },
            https: endpoint("secure.example", 8443),
            ftp: endpoint("ftp.example", 2121),
            socks: endpoint("socks.example", 1080),
        }
    }

    fn endpoint(host: &str, port: u16) -> GnomeProxyEndpoint {
        GnomeProxyEndpoint {
            host: host.to_owned(),
            port,
        }
    }

    fn enabled(host: &str, port: u16) -> ProxySetting {
        ProxySetting::new(true, Some(ProxyEndpoint::new(host, port).unwrap())).unwrap()
    }
}
