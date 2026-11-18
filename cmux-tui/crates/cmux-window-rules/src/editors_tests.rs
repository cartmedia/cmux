use super::*;

#[test]
fn the_table_lists_the_editors_in_menu_order() {
    let ids: Vec<_> = EDITORS.iter().map(|editor| editor.id).collect();
    assert_eq!(
        ids,
        [
            "vscode", "cursor", "zed", "xcode", "sublime", "intellij", "webstorm", "pycharm",
            "goland", "rustrover", "clion", "phpstorm", "rubymine", "rider", "fleet", "nova",
        ]
    );
}

#[test]
fn every_editor_has_an_id_the_app_accepts_a_name_bundle_ids_and_a_symbol() {
    let mut bundles = std::collections::HashSet::new();
    for editor in EDITORS {
        assert!(
            (1..=32).contains(&editor.id.len())
                && editor.id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
            "{}: an id is ^[a-z0-9-]{{1,32}}$",
            editor.id
        );
        assert!(!editor.name.is_empty() && !editor.symbol.is_empty(), "{}", editor.id);
        assert!(!editor.bundle_ids.is_empty(), "{}", editor.id);
        for bundle in editor.bundle_ids {
            assert!(bundles.insert(*bundle), "{bundle} is two editors'");
        }
    }
    assert_eq!(find("zed").map(|editor| editor.name), Some("Zed"));
    assert_eq!(find("emacs"), None);
}

#[test]
fn only_vscode_and_cursor_have_a_remote_scheme() {
    let remote: Vec<_> =
        EDITORS.iter().filter_map(|editor| editor.remote_scheme.map(|s| (editor.id, s))).collect();
    assert_eq!(remote, [("vscode", "vscode"), ("cursor", "cursor")]);
}

#[test]
fn the_table_serializes_as_the_app_reads_it() {
    let value = serde_json::to_value(table()).unwrap();
    assert_eq!(
        value["editors"][0],
        serde_json::json!({
            "id": "vscode",
            "name": "Visual Studio Code",
            "bundleIds": ["com.microsoft.VSCode", "com.microsoft.VSCodeInsiders"],
            "symbol": DEFAULT_SYMBOL,
            "remoteScheme": "vscode",
        })
    );
    assert_eq!(value["editors"][2]["remoteScheme"], serde_json::Value::Null);
}

#[test]
fn the_configured_editor_is_the_default_when_it_is_installed() {
    assert_eq!(
        pick(&["zed", "vscode", "cursor"], Some("cursor")),
        Pick { default: Some("cursor"), ordered: vec!["cursor", "vscode", "zed"] }
    );
}

#[test]
fn otherwise_the_first_installed_in_table_order_is_the_default() {
    let first = Pick { default: Some("vscode"), ordered: vec!["vscode", "zed"] };
    assert_eq!(pick(&["zed", "vscode"], None), first);
    assert_eq!(pick(&["zed", "vscode"], Some("cursor")), first, "cursor is not installed");
    assert_eq!(pick(&["zed", "vscode"], Some("emacs")), first, "not in the table");
}

#[test]
fn unknown_and_repeated_ids_count_once_or_not_at_all() {
    assert_eq!(
        pick(&["emacs", "nova", "nova", "xcode"], Some("nova")),
        Pick { default: Some("nova"), ordered: vec!["nova", "xcode"] }
    );
}

#[test]
fn no_editor_installed_is_no_default_and_no_rows() {
    assert_eq!(pick::<&str>(&[], Some("vscode")), Pick::default());
    assert_eq!(pick(&["emacs"], None), Pick::default());
    assert_eq!(
        serde_json::to_value(pick::<&str>(&[], None)).unwrap(),
        serde_json::json!({"default": null, "ordered": []})
    );
}
