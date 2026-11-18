// cmux NEXT action catalog data. Edit by hand: this is the source of truth.
//! The `AgentChatMenuActionCatalog` actions.

use super::args;
use crate::model::*;
use crate::text::t;

const GROUP: &str = "AgentChatMenuActionCatalog";

// The folder rows' menu titles, shared by the chat, Priority row and workspace row menus.
pub(super) const OPEN_IN_EDITOR: Text = t("ActionCatalog", "menu.openInEditor", "Open in Editor");
pub(super) const OPEN_IN: Text = t("Localizable", "menu.folder.openIn", "Open In");
pub(super) const REVEAL_IN_FINDER: Text =
    t("ActionCatalog", "action.fileExplorerReveal", "Reveal in Finder");
pub(super) const OPEN_TERMINAL_HERE: Text =
    t("ActionCatalog", "menu.openTerminalHere", "Open Terminal Here");
pub(super) const COPY_PATH: Text = t("ActionCatalog", "action.fileExplorerCopyPath", "Copy Path");

#[rustfmt::skip]
pub static ACTIONS: &[Action] = &[
    Action {
        id: "agentPane.toggleDisplaySettings",
        group: GROUP,
        title: Text::raw("Toggle Text & Width"),
        keywords: &["agent", "chat", "display", "text", "width", "appearance", "font", "layout"],
        category: "agents",
        symbol: "textformat.size",
        surface_plan: SurfacePlan { cli: Some(Decision::Offered), context_menu_exemption: Some("guiOnly"), ..SurfacePlan::DEFAULT },
        requires: &["agentPaneFocused"],
        targets: &["pane"],
        cli_name: "agent toggle-display-settings",
        focuses: true,
        ..Action::DEFAULT
    },
    Action {
        id: "agentPane.replayConversation",
        group: GROUP,
        title: Text::raw("Replay Conversation"),
        keywords: &["agent", "chat", "conversation", "replay", "history", "recording", "playback"],
        category: "agents",
        symbol: "arrow.counterclockwise",
        surface_plan: SurfacePlan { cli: Some(Decision::Offered), context_menu_exemption: Some("guiOnly"), ..SurfacePlan::DEFAULT },
        requires: &["agentPaneFocused"],
        targets: &["pane"],
        cli_name: "agent replay-conversation",
        focuses: true,
        ..Action::DEFAULT
    },
    Action {
        id: "agentPane.editInstructions",
        group: GROUP,
        title: Text::raw("Edit Project Instructions…"),
        keywords: &["agent", "chat", "project", "instructions", "claude.md", "agents.md", "rules", "edit"],
        category: "agents",
        symbol: "doc.text",
        surface_plan: SurfacePlan { cli: Some(Decision::Offered), context_menu_exemption: Some("guiOnly"), ..SurfacePlan::DEFAULT },
        requires: &["agentPaneFocused"],
        targets: &["pane"],
        cli_name: "agent edit-instructions",
        focuses: true,
        ..Action::DEFAULT
    },
    Action {
        id: "agentPane.addProjectFiles",
        group: GROUP,
        title: Text::raw("Add Project Files…"),
        keywords: &["agent", "chat", "project", "files", "context", "knowledge", "add", "attach"],
        category: "agents",
        symbol: "folder.badge.plus",
        surface_plan: SurfacePlan { cli: Some(Decision::Offered), context_menu_exemption: Some("guiOnly"), ..SurfacePlan::DEFAULT },
        requires: &["agentPaneFocused"],
        targets: &["pane"],
        cli_name: "agent add-project-files",
        focuses: true,
        ..Action::DEFAULT
    },
    Action {
        id: "agentPane.markUnread",
        group: GROUP,
        title: Text::raw("Mark Chat Unread"),
        keywords: &["agent", "chat", "unread", "notification", "mark", "later"],
        category: "agents",
        symbol: "circle.fill",
        surface_plan: SurfacePlan { cli: Some(Decision::Offered), context_menu_exemption: Some("guiOnly"), ..SurfacePlan::DEFAULT },
        requires: &["agentPaneFocused"],
        targets: &["pane"],
        cli_name: "agent mark-unread",
        focuses: true,
        ..Action::DEFAULT
    },
    Action {
        id: "agentPane.moveToFloatingChat",
        group: GROUP,
        title: Text::raw("Move Chat to Floating Chat"),
        keywords: &["agent", "chat", "floating", "float", "detach", "panel", "window"],
        category: "agents",
        symbol: "rectangle.inset.filled.and.person.filled",
        surface_plan: SurfacePlan { cli: Some(Decision::Offered), context_menu_exemption: Some("guiOnly"), ..SurfacePlan::DEFAULT },
        requires: &["agentPaneFocused"],
        targets: &["pane"],
        cli_name: "agent move-to-floating-chat",
        focuses: true,
        ..Action::DEFAULT
    },
    Action {
        id: "agentPane.continueAfterError",
        group: GROUP,
        title: t("ActionCatalog", "action.agentPane.continueAfterError", "Continue After Error"),
        keywords: &["agent", "chat", "continue", "retry", "try again", "resume", "error", "failed", "rate limit", "capacity"],
        category: "agents",
        symbol: "arrow.clockwise",
        surface_plan: SurfacePlan { cli: Some(Decision::Offered), context_menu_exemption: Some("guiOnly"), ..SurfacePlan::DEFAULT },
        requires: &["agentPaneFocused"],
        targets: &["pane"],
        cli_name: "agent continue-after-error",
        focuses: true,
        ..Action::DEFAULT
    },
    Action {
        id: "agentPane.toggleMinimap",
        group: GROUP,
        title: Text::raw("Toggle Thread Minimap"),
        keywords: &["agent", "chat", "thread", "minimap", "rail", "prompts", "navigation", "map"],
        category: "agents",
        symbol: "sidebar.left",
        surface_plan: SurfacePlan { cli: Some(Decision::Offered), context_menu_exemption: Some("guiOnly"), ..SurfacePlan::DEFAULT },
        requires: &["agentPaneFocused"],
        targets: &["pane"],
        cli_name: "agent toggle-minimap",
        focuses: true,
        ..Action::DEFAULT
    },
    // The chat's folder, one click away (Open in Editor, Open In ▸, Reveal in Finder, Open Terminal
    // Here, Copy Path) on the chat's right-click menu and its Priority row in the sidebar. A remote
    // or Cloud chat's folder refuses the Finder, editor and terminal rows with the registry's remote
    // refusal; Copy Path copies the remote path.
    Action {
        id: "agentPane.openCwdInEditor",
        group: GROUP,
        title: Text::raw("Open Chat Folder in Editor"),
        keywords: &["agent", "chat", "folder", "cwd", "directory", "editor", "ide", "code", "vscode", "cursor", "zed", "xcode", "open"],
        category: "agents",
        symbol: "chevron.left.forwardslash.chevron.right",
        surface_plan: SurfacePlan { cli: Some(Decision::Offered), context_menus: &[Placement::at("agentChat", "reopen", 0).label(OPEN_IN_EDITOR), Placement::at("priorityRow", "reopen", 0).label(OPEN_IN_EDITOR)], ..SurfacePlan::DEFAULT },
        requires: &["agentPaneFocused"],
        arguments: &[args::EDITOR.optional()],
        targets: &["pane", "agent-chat"],
        cli_name: "agent open-in-editor",
        focuses: true,
        ..Action::DEFAULT
    },
    Action {
        id: "agentPane.openCwdInApp",
        group: GROUP,
        title: Text::raw("Open Chat Folder In…"),
        keywords: &["agent", "chat", "folder", "cwd", "directory", "editor", "ide", "open in", "open with", "app"],
        category: "agents",
        symbol: "arrow.up.forward.app",
        surface_plan: SurfacePlan { cli: Some(Decision::Offered), context_menus: &[Placement::at("agentChat", "reopen", 1).style("choices").label(OPEN_IN), Placement::at("priorityRow", "reopen", 1).style("choices").label(OPEN_IN)], ..SurfacePlan::DEFAULT },
        requires: &["agentPaneFocused"],
        arguments: &[args::EDITOR],
        targets: &["pane", "agent-chat"],
        cli_name: "agent open-in",
        focuses: true,
        ..Action::DEFAULT
    },
    Action {
        id: "agentPane.revealCwdInFinder",
        group: GROUP,
        title: Text::raw("Reveal Chat Folder in Finder"),
        keywords: &["agent", "chat", "folder", "cwd", "directory", "finder", "reveal", "show"],
        category: "agents",
        symbol: "folder",
        surface_plan: SurfacePlan { cli: Some(Decision::Offered), context_menus: &[Placement::at("agentChat", "reopen", 2).label(REVEAL_IN_FINDER), Placement::at("priorityRow", "reopen", 2).label(REVEAL_IN_FINDER)], ..SurfacePlan::DEFAULT },
        requires: &["agentPaneFocused"],
        targets: &["pane", "agent-chat"],
        cli_name: "agent reveal-cwd-in-finder",
        focuses: true,
        ..Action::DEFAULT
    },
    Action {
        id: "agentPane.openCwdInTerminal",
        group: GROUP,
        title: Text::raw("Open Chat Folder in Terminal"),
        keywords: &["agent", "chat", "folder", "cwd", "directory", "terminal", "shell", "open", "here"],
        category: "agents",
        symbol: "terminal",
        surface_plan: SurfacePlan { cli: Some(Decision::Offered), context_menus: &[Placement::at("agentChat", "reopen", 3).label(OPEN_TERMINAL_HERE), Placement::at("priorityRow", "reopen", 3).label(OPEN_TERMINAL_HERE)], ..SurfacePlan::DEFAULT },
        requires: &["agentPaneFocused"],
        targets: &["pane", "agent-chat"],
        cli_name: "agent open-cwd-in-terminal",
        focuses: true,
        ..Action::DEFAULT
    },
    Action {
        id: "agentPane.copyCwdPath",
        group: GROUP,
        title: Text::raw("Copy Chat Folder Path"),
        keywords: &["agent", "chat", "folder", "cwd", "directory", "path", "copy", "clipboard"],
        category: "agents",
        symbol: "doc.on.clipboard",
        surface_plan: SurfacePlan { cli: Some(Decision::Exempt("clipboard")), context_menus: &[Placement::at("agentChat", "inspect", 2).folder("copy").label(COPY_PATH), Placement::at("priorityRow", "reopen", 4).label(COPY_PATH)], ..SurfacePlan::DEFAULT },
        requires: &["agentPaneFocused"],
        targets: &["pane", "agent-chat"],
        cli_name: "agent copy-cwd-path",
        ..Action::DEFAULT
    },
];
