//! The code editors a chat's or workspace's folder opens in (Open in Editor, Open In ▸): the
//! table of editors cmux knows, in menu order, and which one is the default. The app only finds
//! which bundle ids resolve to an installed app and opens the folder with the one picked here.

use serde::Serialize;

/// The symbol of an editor row with no symbol of its own.
pub const DEFAULT_SYMBOL: &str = "chevron.left.forwardslash.chevron.right";

/// One editor: its id (`cmux.json` `editor.default`, the actions' `app` argument), the name the
/// menus show, the bundle ids that are this editor (installed = any of them resolves), its menu
/// symbol, and the URL scheme its Remote-SSH open uses (none for an editor without one).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Editor {
    pub id: &'static str,
    pub name: &'static str,
    pub bundle_ids: &'static [&'static str],
    pub symbol: &'static str,
    pub remote_scheme: Option<&'static str>,
}

const fn editor(
    id: &'static str,
    name: &'static str,
    bundle_ids: &'static [&'static str],
) -> Editor {
    Editor { id, name, bundle_ids, symbol: DEFAULT_SYMBOL, remote_scheme: None }
}

/// Every editor, in the order the menus list them.
#[rustfmt::skip]
pub static EDITORS: &[Editor] = &[
    Editor {
        remote_scheme: Some("vscode"),
        ..editor("vscode", "Visual Studio Code", &["com.microsoft.VSCode", "com.microsoft.VSCodeInsiders"])
    },
    Editor { remote_scheme: Some("cursor"), ..editor("cursor", "Cursor", &["com.todesktop.230313mzl4w4u92"]) },
    editor("zed", "Zed", &["dev.zed.Zed", "dev.zed.Zed-Preview"]),
    Editor { symbol: "hammer", ..editor("xcode", "Xcode", &["com.apple.dt.Xcode"]) },
    editor("sublime", "Sublime Text", &["com.sublimetext.4", "com.sublimetext.3"]),
    editor("intellij", "IntelliJ IDEA", &["com.jetbrains.intellij", "com.jetbrains.intellij.ce"]),
    editor("webstorm", "WebStorm", &["com.jetbrains.WebStorm"]),
    editor("pycharm", "PyCharm", &["com.jetbrains.pycharm", "com.jetbrains.pycharm.ce"]),
    editor("goland", "GoLand", &["com.jetbrains.goland"]),
    editor("rustrover", "RustRover", &["com.jetbrains.rustrover"]),
    editor("clion", "CLion", &["com.jetbrains.CLion"]),
    editor("phpstorm", "PhpStorm", &["com.jetbrains.PhpStorm"]),
    editor("rubymine", "RubyMine", &["com.jetbrains.rubymine"]),
    editor("rider", "Rider", &["com.jetbrains.rider"]),
    editor("fleet", "Fleet", &["com.jetbrains.fleet"]),
    editor("nova", "Nova", &["com.panic.Nova"]),
];

/// The editor `id` names, if the table has it.
pub fn find(id: &str) -> Option<&'static Editor> {
    EDITORS.iter().find(|editor| editor.id == id)
}

/// The editor table as the app reads it (`window_editors_table`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Table {
    pub editors: &'static [Editor],
}

pub fn table() -> Table {
    Table { editors: EDITORS }
}

/// The installed editors as the menus list them: the default first, then the others in table
/// order. No default and no rows when none is installed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Pick {
    pub default: Option<&'static str>,
    pub ordered: Vec<&'static str>,
}

/// The default editor and the menu order, from the ids of the editors the app found installed
/// and the one `editor.default` names: that one when it is installed, else the first installed
/// in table order. An id the table does not have, or one listed twice, counts once or not at all.
pub fn pick<S: AsRef<str>>(installed: &[S], configured: Option<&str>) -> Pick {
    let is_installed = |id: &str| installed.iter().any(|item| item.as_ref() == id);
    let found: Vec<&'static str> =
        EDITORS.iter().map(|editor| editor.id).filter(|id| is_installed(id)).collect();
    let default = configured
        .and_then(|id| found.iter().copied().find(|found| *found == id))
        .or_else(|| found.first().copied());
    let ordered = default
        .into_iter()
        .chain(found.iter().copied().filter(|id| Some(*id) != default))
        .collect();
    Pick { default, ordered }
}

#[cfg(test)]
#[path = "editors_tests.rs"]
mod tests;
