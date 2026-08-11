use std::sync::{Mutex, PoisonError};

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{App, AppHandle, Wry};
use uuid::Uuid;

use crate::session::{NodeSummary, SessionStatus};

pub const OPEN_MENU_ID: &str = "tray:open";
pub const TOGGLE_MENU_ID: &str = "tray:toggle";
pub const QUIT_MENU_ID: &str = "tray:quit";
const NODE_MENU_PREFIX: &str = "tray:node:";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrayMenuModel {
    pub status_text: String,
    pub toggle_text: &'static str,
    pub toggle_enabled: bool,
    pub nodes: Vec<TrayNodeItem>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrayNodeItem {
    pub id: Uuid,
    pub name: String,
    pub selected: bool,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrayAction {
    Open,
    Toggle,
    SelectNode(Uuid),
    Quit,
}

pub struct TrayUi {
    status: MenuItem<Wry>,
    toggle: MenuItem<Wry>,
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
        let toggle = MenuItem::with_id(
            app,
            TOGGLE_MENU_ID,
            initial.toggle_text,
            initial.toggle_enabled,
            None::<&str>,
        )?;
        let nodes = Submenu::new(app, "节点", true)?;
        replace_node_items(app.handle(), &nodes, &initial.nodes)?;
        let separator = PredefinedMenuItem::separator(app)?;
        let quit = MenuItem::with_id(app, QUIT_MENU_ID, "退出", true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&open, &status, &toggle, &nodes, &separator, &quit])?;
        let mut builder = TrayIconBuilder::with_id("main")
            .menu(&menu)
            .tooltip("MgClash")
            .on_menu_event(move |app, event| {
                if let Some(action) = action_for_menu_id(event.id().as_ref()) {
                    on_action(app, action);
                }
            });
        if let Some(icon) = app.default_window_icon().cloned() {
            builder = builder.icon(icon);
        }
        builder.build(app)?;

        Ok(Self {
            status,
            toggle,
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
        self.toggle.set_text(model.toggle_text)?;
        self.toggle.set_enabled(model.toggle_enabled)?;
        replace_node_items(app, &self.nodes, &model.nodes)?;
        *last_model = Some(model);
        Ok(())
    }

    pub fn show_action_failure(&self) {
        if let Err(error) = self.status.set_text("操作失败 · 请打开 MgClash 查看") {
            eprintln!("tray failure status update failed: {error}");
        }
    }
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
                &node.name,
                node.enabled,
                node.selected,
                None::<&str>,
            )?)?;
        }
    }
    Ok(())
}

#[must_use]
pub fn menu_model(status: &SessionStatus, nodes: &[NodeSummary]) -> TrayMenuModel {
    let selected_id = status.node.as_ref().map(|node| node.id);
    let state = if status.connected {
        "已连接"
    } else {
        "未连接"
    };
    let status_text = status.node.as_ref().map_or_else(
        || state.to_owned(),
        |node| format!("{state} · {}", node.name),
    );

    TrayMenuModel {
        status_text,
        toggle_text: if status.connected { "断开" } else { "连接" },
        toggle_enabled: status.connected || selected_id.is_some(),
        nodes: nodes
            .iter()
            .map(|node| {
                let selected = selected_id == Some(node.id);
                TrayNodeItem {
                    id: node.id,
                    name: node.name.clone(),
                    selected,
                    enabled: !status.connected && !selected,
                }
            })
            .collect(),
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
        _ => id
            .strip_prefix(NODE_MENU_PREFIX)
            .and_then(|id| Uuid::parse_str(id).ok())
            .map(TrayAction::SelectNode),
    }
}

#[cfg(test)]
mod tests {
    use magies_domain::ProxyProtocol;
    use uuid::Uuid;

    use super::{TrayAction, action_for_menu_id, menu_model, node_menu_id};
    use crate::session::{NodeSummary, SessionStatus};

    #[test]
    fn disconnected_menu_can_connect_and_switch_nodes() {
        let selected = node(1, "Tokyo");
        let other = node(2, "Seattle");
        let model = menu_model(
            &status(false, Some(selected.clone())),
            &[selected.clone(), other.clone()],
        );

        assert_eq!(model.status_text, "未连接 · Tokyo");
        assert_eq!(model.toggle_text, "连接");
        assert!(model.toggle_enabled);
        assert_eq!(model.nodes.len(), 2);
        assert!(model.nodes[0].selected);
        assert!(!model.nodes[0].enabled);
        assert!(model.nodes[1].enabled);
        assert_eq!(
            action_for_menu_id(&node_menu_id(other.id)),
            Some(TrayAction::SelectNode(other.id))
        );
    }

    #[test]
    fn connected_menu_disables_node_switching_and_can_disconnect() {
        let selected = node(1, "Tokyo");
        let model = menu_model(
            &status(true, Some(selected.clone())),
            std::slice::from_ref(&selected),
        );

        assert_eq!(model.status_text, "已连接 · Tokyo");
        assert_eq!(model.toggle_text, "断开");
        assert!(model.toggle_enabled);
        assert!(!model.nodes[0].enabled);
        assert_eq!(action_for_menu_id("tray:toggle"), Some(TrayAction::Toggle));
        assert_eq!(action_for_menu_id("tray:open"), Some(TrayAction::Open));
        assert_eq!(action_for_menu_id("tray:quit"), Some(TrayAction::Quit));
        assert_eq!(action_for_menu_id("tray:unknown"), None);
    }

    #[test]
    fn menu_without_a_selected_node_cannot_connect() {
        let model = menu_model(&status(false, None), &[]);

        assert_eq!(model.status_text, "未连接");
        assert_eq!(model.toggle_text, "连接");
        assert!(!model.toggle_enabled);
        assert!(model.nodes.is_empty());
    }

    fn node(value: u128, name: &str) -> NodeSummary {
        NodeSummary {
            id: Uuid::from_u128(value),
            name: name.to_owned(),
            protocol: ProxyProtocol::Vless,
            server: "example.com".to_owned(),
            port: 443,
            deletable: true,
        }
    }

    const fn status(connected: bool, node: Option<NodeSummary>) -> SessionStatus {
        SessionStatus {
            connected,
            node,
            core: "sing-box",
            mode: "global",
            system_proxy: connected,
            socks_port: 1080,
            http_port: 1081,
        }
    }
}
