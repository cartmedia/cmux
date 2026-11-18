//! Open in Editor: a chat's or workspace's folder opens in the person's editor, Finder or a
//! terminal, or is copied, from its right-click menus (Leo: "what do we think ab this thread?").
//! The rows, their menus and arguments, and no default key (people bind one in Settings).

use cmux_action_catalog::model::{Action, ArgKind, Placement};

fn row(id: &str) -> &'static Action {
    cmux_action_catalog::find(id).unwrap_or_else(|| panic!("{id} is not an action"))
}

/// `id`'s placements as (menu, group, rank, style, folder, label).
fn placements(id: &str) -> Vec<(&'static str, &'static str, i64, &'static str, Option<&'static str>, Option<&'static str>)> {
    row(id)
        .surface_plan
        .context_menus
        .iter()
        .map(|p: &Placement| (p.context, p.group, p.rank, p.style, p.folder, p.label.map(|l| l.en)))
        .collect()
}

const CHAT: &[&str] = &[
    "agentPane.openCwdInEditor",
    "agentPane.openCwdInApp",
    "agentPane.revealCwdInFinder",
    "agentPane.openCwdInTerminal",
    "agentPane.copyCwdPath",
];
const WORKSPACE: &[&str] = &[
    "workspace.openInEditor",
    "workspace.openInApp",
    "revealWorkspaceInFinder",
    "workspace.openTerminalHere",
    "workspace.copyPath",
];

#[test]
fn the_folder_rows_have_no_default_key_and_stay_in_the_palette() {
    for id in CHAT.iter().chain(WORKSPACE).chain(&["palette.terminalOpenDirectory"]) {
        let action = row(id);
        assert!(action.default_shortcut.is_none() && action.default_chord.is_none(), "{id}");
        assert_eq!(action.surface_plan.palette.wire(), "offered", "{id}");
        assert_eq!(action.surface_plan.context_menu_exemption, None, "{id}");
    }
}

#[test]
fn the_chat_rows_act_on_a_pane_or_a_priority_row_chat() {
    for id in CHAT {
        assert_eq!(row(id).targets, &["pane", "agent-chat"], "{id}");
    }
    assert_eq!(row("agentPane.openCwdInEditor").cli_name, "agent open-in-editor");
    assert_eq!(row("workspace.openInEditor").cli_name, "workspace open-in-editor");
    assert_eq!(row("workspace.openTerminalHere").cli_name, "workspace open-terminal-here");
}

#[test]
fn open_in_editor_may_name_an_editor_and_open_in_must() {
    for (id, required) in [
        ("agentPane.openCwdInEditor", false),
        ("agentPane.openCwdInApp", true),
        ("workspace.openInEditor", false),
        ("workspace.openInApp", true),
    ] {
        let [app] = row(id).arguments else { panic!("{id}: one argument") };
        assert_eq!((app.name, app.required, app.kind), ("app", required, ArgKind::String), "{id}");
        assert_eq!(app.suggestions.map(|s| s.source), Some("editors"), "{id}");
    }
    let [app] = row("palette.terminalOpenDirectory").arguments else { panic!("one argument") };
    assert!(!app.required, "no app opens the default editor");
}

#[test]
fn the_chat_menu_has_the_folder_group_and_copy_path_in_copy() {
    assert_eq!(
        CHAT.iter().flat_map(|id| placements(id)).filter(|p| p.0 == "agentChat").collect::<Vec<_>>(),
        [
            ("agentChat", "reopen", 0, "item", None, Some("Open in Editor")),
            ("agentChat", "reopen", 1, "choices", None, Some("Open In")),
            ("agentChat", "reopen", 2, "item", None, Some("Reveal in Finder")),
            ("agentChat", "reopen", 3, "item", None, Some("Open Terminal Here")),
            ("agentChat", "inspect", 2, "item", Some("copy"), Some("Copy Path")),
        ]
    );
    assert_eq!(placements("agentChat.copyIdentifier")[1].4, Some("copy"));
    for id in ["agentPaneZoomIn", "agentPaneZoomOut", "agentPaneZoomReset", "appearance.changeBackground", "workspace.setTheme"] {
        let chat: Vec<_> = placements(id).into_iter().filter(|p| p.0 == "agentChat").collect();
        assert_eq!(chat.len(), 1, "{id}");
        assert_eq!(chat[0].4, Some("appearance"), "{id} folds into Appearance");
    }
}

#[test]
fn the_priority_row_has_the_five_folder_rows_in_one_group() {
    assert_eq!(
        CHAT.iter().flat_map(|id| placements(id)).filter(|p| p.0 == "priorityRow").collect::<Vec<_>>(),
        [
            ("priorityRow", "reopen", 0, "item", None, Some("Open in Editor")),
            ("priorityRow", "reopen", 1, "choices", None, Some("Open In")),
            ("priorityRow", "reopen", 2, "item", None, Some("Reveal in Finder")),
            ("priorityRow", "reopen", 3, "item", None, Some("Open Terminal Here")),
            ("priorityRow", "reopen", 4, "item", None, Some("Copy Path")),
        ]
    );
}

#[test]
fn the_workspace_row_has_the_folder_rows_and_copy_path_in_copy() {
    assert_eq!(
        WORKSPACE.iter().flat_map(|id| placements(id)).collect::<Vec<_>>(),
        [
            ("workspaceRow", "reopen", 0, "item", None, Some("Open in Editor")),
            ("workspaceRow", "reopen", 1, "choices", None, Some("Open In")),
            ("workspaceRow", "reopen", 2, "item", None, Some("Reveal in Finder")),
            ("workspaceRow", "reopen", 3, "item", None, Some("Open Terminal Here")),
            ("workspaceRow", "inspect", 407, "item", Some("copy"), Some("Copy Path")),
        ]
    );
}

/// The chat menu's top level as the app makes it from the placements (`ContextMenuCatalog
/// .topLevel`: one row per unfoldered placement, one per folder of two or more), within the
/// 13 rows `ChatMenuLengthTests` allows; every folder it has holds at least two rows.
#[test]
fn the_chat_menu_keeps_a_short_top_level() {
    let mut rows = 0;
    let mut folders: std::collections::BTreeMap<&str, usize> = Default::default();
    for action in cmux_action_catalog::actions() {
        for p in action.surface_plan.context_menus.iter().filter(|p| p.context == "agentChat" && p.parent.is_none()) {
            match p.folder {
                Some(folder) => *folders.entry(folder).or_default() += 1,
                None => rows += 1,
            }
        }
    }
    let top = rows + folders.values().map(|_| 1).sum::<usize>();
    assert!(top <= 12, "the chat menu has {top} top-level rows ({rows} rows, folders {folders:?})");
    assert!(folders.values().all(|n| *n >= 2), "a folder of one row: {folders:?}");
}
