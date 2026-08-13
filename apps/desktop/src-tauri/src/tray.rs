use std::sync::{Mutex, PoisonError};

use magies_routing::RoutingMode;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{App, AppHandle, Wry};
use uuid::Uuid;

use crate::session::{NodeSummary, SessionStatus};
use crate::traffic::TrafficSnapshot;

pub const OPEN_MENU_ID: &str = "tray:open";
pub const TOGGLE_MENU_ID: &str = "tray:toggle";
pub const QUIT_MENU_ID: &str = "tray:quit";
const NODE_MENU_PREFIX: &str = "tray:node:";
const MODE_GLOBAL_MENU_ID: &str = "tray:mode:global";
const MODE_RULE_MENU_ID: &str = "tray:mode:rule";
const MODE_DIRECT_MENU_ID: &str = "tray:mode:direct";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrayMenuModel {
    pub status_text: String,
    pub node_text: String,
    pub traffic_text: String,
    pub toggle_text: &'static str,
    pub toggle_enabled: bool,
    pub mode: RoutingMode,
    pub mode_enabled: bool,
    pub nodes: Vec<TrayNodeItem>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrayNodeItem {
    pub id: Uuid,
    pub name: String,
    pub latency_ms: Option<u32>,
    pub selected: bool,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrayAction {
    Open,
    Toggle,
    SetRoutingMode(RoutingMode),
    SelectNode(Uuid),
    Quit,
}

const TRAY_ICON_ID: &str = "main";

pub struct TrayUi {
    icon: TrayIcon<Wry>,
    status: MenuItem<Wry>,
    node: MenuItem<Wry>,
    traffic: MenuItem<Wry>,
    toggle: MenuItem<Wry>,
    modes: Submenu<Wry>,
    nodes: Submenu<Wry>,
    last_model: Mutex<Option<TrayMenuModel>>,
}

impl TrayUi {
    /// Installs the single compact desktop tray menu.
    ///
    /// # Errors
    ///
    /// Returns a Tauri menu or tray error when the host desktop cannot create
    /// one of the native items.
    pub fn install(
        app: &App,
        initial: TrayMenuModel,
        on_action: impl Fn(&AppHandle, TrayAction) + Send + Sync + 'static,
    ) -> tauri::Result<Self> {
        let open = MenuItem::with_id(app, OPEN_MENU_ID, "打开 MgClash", true, None::<&str>)?;
        let status = MenuItem::new(app, &initial.status_text, false, None::<&str>)?;
        let node = MenuItem::new(app, &initial.node_text, false, None::<&str>)?;
        let traffic = MenuItem::new(app, &initial.traffic_text, false, None::<&str>)?;
        let toggle = MenuItem::with_id(
            app,
            TOGGLE_MENU_ID,
            initial.toggle_text,
            initial.toggle_enabled,
            None::<&str>,
        )?;
        let modes = Submenu::new(app, "模式", true)?;
        replace_mode_items(app.handle(), &modes, initial.mode, initial.mode_enabled)?;
        let nodes = Submenu::new(app, "节点", true)?;
        replace_node_items(app.handle(), &nodes, &initial.nodes)?;
        let info_separator = PredefinedMenuItem::separator(app)?;
        let action_separator = PredefinedMenuItem::separator(app)?;
        let quit = MenuItem::with_id(app, QUIT_MENU_ID, "退出", true, None::<&str>)?;
        let menu = Menu::with_items(
            app,
            &[
                &status,
                &node,
                &traffic,
                &info_separator,
                &modes,
                &nodes,
                &action_separator,
                &open,
                &toggle,
                &quit,
            ],
        )?;
        let mut builder = TrayIconBuilder::with_id(TRAY_ICON_ID)
            .menu(&menu)
            .tooltip(tray_tooltip(&initial))
            .on_menu_event(move |app, event| {
                if let Some(action) = action_for_menu_id(event.id().as_ref()) {
                    on_action(app, action);
                }
            });
        if let Some(icon) = app.default_window_icon().cloned() {
            builder = builder.icon(icon);
        }
        let icon = builder.build(app)?;

        Ok(Self {
            icon,
            status,
            node,
            traffic,
            toggle,
            modes,
            nodes,
            last_model: Mutex::new(Some(initial)),
        })
    }

    /// Applies a changed session snapshot without replacing the native menu.
    /// Keeping the same menu is required by the Linux tray implementation.
    ///
    /// # Errors
    ///
    /// Returns a Tauri menu error when a native item cannot be changed.
    pub fn refresh(&self, app: &AppHandle, model: TrayMenuModel) -> tauri::Result<()> {
        let mut last_model = self
            .last_model
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if last_model.as_ref() == Some(&model) {
            return Ok(());
        }

        self.status.set_text(&model.status_text)?;
        self.node.set_text(&model.node_text)?;
        self.traffic.set_text(&model.traffic_text)?;
        self.toggle.set_text(model.toggle_text)?;
        self.toggle.set_enabled(model.toggle_enabled)?;
        replace_mode_items(app, &self.modes, model.mode, model.mode_enabled)?;
        replace_node_items(app, &self.nodes, &model.nodes)?;
        self.icon.set_tooltip(Some(tray_tooltip(&model)))?;
        *last_model = Some(model);
        Ok(())
    }

    pub fn show_action_failure(&self) {
        if let Err(error) = self.status.set_text("操作失败 · 请打开 MgClash 查看") {
            tracing::warn!("tray failure status update failed: {error}");
        }
    }
}

fn replace_mode_items(
    app: &AppHandle,
    submenu: &Submenu<Wry>,
    selected: RoutingMode,
    enabled: bool,
) -> tauri::Result<()> {
    let item_count = submenu.items()?.len();
    for _ in 0..item_count {
        let _ = submenu.remove_at(0)?;
    }
    for (id, label, mode) in [
        (MODE_GLOBAL_MENU_ID, "全局", RoutingMode::Global),
        (MODE_RULE_MENU_ID, "规则", RoutingMode::Rule),
        (MODE_DIRECT_MENU_ID, "直连", RoutingMode::Direct),
    ] {
        submenu.append(&CheckMenuItem::with_id(
            app,
            id,
            label,
            enabled,
            selected == mode,
            None::<&str>,
        )?)?;
    }
    Ok(())
}

fn replace_node_items(
    app: &AppHandle,
    submenu: &Submenu<Wry>,
    nodes: &[TrayNodeItem],
) -> tauri::Result<()> {
    let item_count = submenu.items()?.len();
    for _ in 0..item_count {
        let _ = submenu.remove_at(0)?;
    }

    if nodes.is_empty() {
        submenu.append(&MenuItem::new(app, "暂无节点", false, None::<&str>)?)?;
    } else {
        for node in nodes {
            submenu.append(&CheckMenuItem::with_id(
                app,
                node_menu_id(node.id),
                &format_tray_node_label(&node.name, node.latency_ms),
                node.enabled,
                node.selected,
                None::<&str>,
            )?)?;
        }
    }
    Ok(())
}

#[must_use]
pub fn menu_model(
    status: &SessionStatus,
    nodes: &[NodeSummary],
    traffic: TrafficSnapshot,
) -> TrayMenuModel {
    let selected_id = status.node.as_ref().map(|node| node.id);
    let state = if status.connected {
        "已连接"
    } else {
        "未连接"
    };
    let (mode, mode_text) = match status.mode {
        "rule" => (RoutingMode::Rule, "规则"),
        "direct" => (RoutingMode::Direct, "直连"),
        _ => (RoutingMode::Global, "全局"),
    };
    let status_text = format!("{state} · {mode_text}");
    let node_text = status.node.as_ref().map_or_else(
        || "未选择节点".to_owned(),
        |node| match node.latency_ms {
            Some(latency_ms) => format!("{} · {latency_ms} ms", node.name),
            None => format!("{} · 未测速", node.name),
        },
    );
    let traffic = if status.connected {
        traffic
    } else {
        TrafficSnapshot::default()
    };
    let traffic_text = format!(
        "↓ {}    ↑ {}",
        format_rate(traffic.download_bytes_per_second),
        format_rate(traffic.upload_bytes_per_second)
    );
    let ordered_nodes = sort_nodes_for_tray(nodes);

    TrayMenuModel {
        status_text,
        node_text,
        traffic_text,
        toggle_text: if status.connected { "断开" } else { "连接" },
        toggle_enabled: status.connected || selected_id.is_some(),
        mode,
        mode_enabled: !status.connected,
        nodes: ordered_nodes
            .iter()
            .map(|node| {
                let selected = selected_id == Some(node.id);
                TrayNodeItem {
                    id: node.id,
                    name: node.name.clone(),
                    latency_ms: node.latency_ms,
                    selected,
                    enabled: !selected && node.enabled,
                }
            })
            .collect(),
    }
}

fn tray_tooltip(model: &TrayMenuModel) -> String {
    format!(
        "MgClash\n{}\n{}\n{}",
        model.status_text, model.node_text, model.traffic_text
    )
}

fn sort_nodes_for_tray(nodes: &[NodeSummary]) -> Vec<NodeSummary> {
    let mut ordered = nodes.to_vec();
    ordered.sort_by(|left, right| {
        match (left.latency_ms, right.latency_ms) {
            (Some(left_ms), Some(right_ms)) => left_ms.cmp(&right_ms),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left.name.cmp(&right.name),
        }
    });
    ordered
}

fn format_tray_node_label(name: &str, latency_ms: Option<u32>) -> String {
    match latency_ms {
        Some(latency_ms) => format!("{name} · {latency_ms} ms"),
        None => format!("{name} · --"),
    }
}

fn format_rate(bytes_per_second: u64) -> String {
    let (divisor, unit) = if bytes_per_second >= 1_073_741_824 {
        (1_073_741_824, "GB/s")
    } else if bytes_per_second >= 1_048_576 {
        (1_048_576, "MB/s")
    } else if bytes_per_second >= 1_024 {
        (1_024, "KB/s")
    } else {
        return format!("{bytes_per_second} B/s");
    };
    let whole = bytes_per_second / divisor;
    let tenth = ((bytes_per_second % divisor) * 10 + divisor / 2) / divisor;
    if tenth == 10 {
        format!("{}.0 {unit}", whole + 1)
    } else {
        format!("{whole}.{tenth} {unit}")
    }
}

#[must_use]
pub fn node_menu_id(id: Uuid) -> String {
    format!("{NODE_MENU_PREFIX}{id}")
}

#[must_use]
pub fn action_for_menu_id(id: &str) -> Option<TrayAction> {
    match id {
        OPEN_MENU_ID => Some(TrayAction::Open),
        TOGGLE_MENU_ID => Some(TrayAction::Toggle),
        QUIT_MENU_ID => Some(TrayAction::Quit),
        MODE_GLOBAL_MENU_ID => Some(TrayAction::SetRoutingMode(RoutingMode::Global)),
        MODE_RULE_MENU_ID => Some(TrayAction::SetRoutingMode(RoutingMode::Rule)),
        MODE_DIRECT_MENU_ID => Some(TrayAction::SetRoutingMode(RoutingMode::Direct)),
        _ => id
            .strip_prefix(NODE_MENU_PREFIX)
            .and_then(|id| Uuid::parse_str(id).ok())
            .map(TrayAction::SelectNode),
    }
}

#[cfg(test)]
mod tests {
    use crate::dns_settings::DnsSettings;
    use crate::route_settings::RouteSettings;
    use magies_domain::ProxyProtocol;
    use magies_routing::RoutingMode;
    use uuid::Uuid;

    use super::{TrayAction, action_for_menu_id, format_tray_node_label, menu_model, node_menu_id, sort_nodes_for_tray, tray_tooltip};
    use crate::session::{NodeSummary, RouteSchemeSummary, SessionStatus};
    use crate::traffic::TrafficSnapshot;

    #[test]
    fn disconnected_menu_can_connect_and_switch_nodes() {
        let selected = node(1, "Tokyo");
        let other = node(2, "Seattle");
        let model = menu_model(
            &status(false, Some(selected.clone())),
            &[selected.clone(), other.clone()],
            TrafficSnapshot::default(),
        );

        assert_eq!(model.status_text, "未连接 · 全局");
        assert_eq!(model.node_text, "Tokyo · 未测速");
        assert_eq!(model.traffic_text, "↓ 0 B/s    ↑ 0 B/s");
        assert_eq!(model.toggle_text, "连接");
        assert!(model.toggle_enabled);
        assert_eq!(model.mode, RoutingMode::Global);
        assert!(model.mode_enabled);
        assert_eq!(model.nodes.len(), 2);
        let selected_entry = model
            .nodes
            .iter()
            .find(|node| node.id == selected.id)
            .expect("selected node should appear in the tray list");
        assert!(selected_entry.selected);
        assert!(!selected_entry.enabled);
        assert!(model.nodes.iter().any(|node| node.id == other.id && node.enabled));
        assert_eq!(
            action_for_menu_id(&node_menu_id(other.id)),
            Some(TrayAction::SelectNode(other.id))
        );
    }

    #[test]
    fn connected_menu_disables_node_switching_and_can_disconnect() {
        let mut selected = node(1, "Tokyo");
        selected.latency_ms = Some(32);
        let model = menu_model(
            &status(true, Some(selected.clone())),
            std::slice::from_ref(&selected),
            traffic(1_048_576, 2_048),
        );

        assert_eq!(model.status_text, "已连接 · 全局");
        assert_eq!(model.node_text, "Tokyo · 32 ms");
        assert_eq!(model.traffic_text, "↓ 2.0 KB/s    ↑ 1.0 MB/s");
        assert_eq!(model.toggle_text, "断开");
        assert!(model.toggle_enabled);
        assert!(!model.mode_enabled);
        assert!(!model.nodes[0].enabled);
        assert_eq!(
            tray_tooltip(&model),
            "MgClash\n已连接 · 全局\nTokyo · 32 ms\n↓ 2.0 KB/s    ↑ 1.0 MB/s"
        );
        assert_eq!(action_for_menu_id("tray:toggle"), Some(TrayAction::Toggle));
        assert_eq!(action_for_menu_id("tray:open"), Some(TrayAction::Open));
        assert_eq!(action_for_menu_id("tray:quit"), Some(TrayAction::Quit));
        assert_eq!(
            action_for_menu_id("tray:mode:direct"),
            Some(TrayAction::SetRoutingMode(RoutingMode::Direct))
        );
        assert_eq!(action_for_menu_id("tray:unknown"), None);
    }

    #[test]
    fn menu_without_a_selected_node_cannot_connect() {
        let model = menu_model(&status(false, None), &[], traffic(1_024, 2_048));

        assert_eq!(model.status_text, "未连接 · 全局");
        assert_eq!(model.node_text, "未选择节点");
        assert_eq!(model.traffic_text, "↓ 0 B/s    ↑ 0 B/s");
        assert_eq!(model.toggle_text, "连接");
        assert!(!model.toggle_enabled);
        assert!(model.nodes.is_empty());
    }

    #[test]
    fn menu_displays_the_current_routing_mode() {
        let mut current = status(false, None);
        current.mode = "rule";

        assert_eq!(
            menu_model(&current, &[], TrafficSnapshot::default()).status_text,
            "未连接 · 规则"
        );
    }

    #[test]
    fn connected_tray_lets_the_user_switch_to_another_node() {
        let mut selected = node(1, "Tokyo");
        selected.latency_ms = Some(32);
        let mut other = node(2, "Osaka");
        other.latency_ms = Some(48);
        let model = menu_model(
            &status(true, Some(selected.clone())),
            &[selected, other.clone()],
            TrafficSnapshot::default(),
        );
        assert!(model.nodes.iter().any(|entry| entry.id == other.id && entry.enabled));
    }

    #[test]
    fn tray_node_labels_include_latency_when_measured() {
        assert_eq!(
            format_tray_node_label("Tokyo", Some(32)),
            "Tokyo · 32 ms"
        );
        assert_eq!(format_tray_node_label("Tokyo", None), "Tokyo · --");
    }

    #[test]
    fn tray_nodes_are_sorted_by_latency_with_untested_last() {
        let mut fast = node(1, "Fast");
        fast.latency_ms = Some(20);
        let mut slow = node(2, "Slow");
        slow.latency_ms = Some(80);
        let untested = node(3, "Untested");
        let ordered = sort_nodes_for_tray(&[untested.clone(), slow.clone(), fast.clone()]);
        assert_eq!(ordered[0].id, fast.id);
        assert_eq!(ordered[1].id, slow.id);
        assert_eq!(ordered[2].id, untested.id);
    }

    fn node(value: u128, name: &str) -> NodeSummary {
        NodeSummary {
            id: Uuid::from_u128(value),
            name: name.to_owned(),
            protocol: ProxyProtocol::Vless,
            server: "example.com".to_owned(),
            port: 443,
            group_id: None,
            transport: "tcp",
            tls: None,
            deletable: true,
            enabled: true,
            latency_ms: None,
            last_tested_at: None,
        }
    }

    const fn traffic(upload: u64, download: u64) -> TrafficSnapshot {
        TrafficSnapshot {
            upload_bytes_per_second: upload,
            download_bytes_per_second: download,
            today_bytes: 0,
            month_bytes: 0,
            total_bytes: 0,
        }
    }

    fn status(connected: bool, node: Option<NodeSummary>) -> SessionStatus {
        SessionStatus {
            connected,
            node,
            core: "sing-box",
            dns: DnsSettings::default(),
            mode: "global",
            route: RouteSettings::default(),
            route_scheme_id: "default".to_owned(),
            route_schemes: vec![RouteSchemeSummary {
                id: "default".to_owned(),
                name: "默认".to_owned(),
            }],
            system_proxy: connected,
            system_proxy_mode: "managed",
            socks_port: 1080,
            http_port: 1081,
            clash_api_port: 9090,
        }
    }
}
