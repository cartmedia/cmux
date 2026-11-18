//! All chats search per keystroke at 200 and 5,000 chats: the work one keystroke does (row values
//! built, chat texts searched) must not grow with the number of chats (Leo, 2026-11-04: "never
//! ... acceptable ... linear time growth degradation"). Counted, not timed, so it is the same on
//! every machine. `cargo test -p acpmux --lib chats::window::scale_tests -- --nocapture` prints
//! the counts.

use std::path::{Path, PathBuf};

use cmux_chat_index::transcript::Turn;
use cmux_chat_index::{AdapterKind, ChatRoot, RootSource};

use super::*;

const LIMIT: usize = 50;
/// The chats whose text holds the word typed, by position.
fn holds_word(i: usize, chats: usize) -> bool {
    i == 3 || i == chats / 2 || i == chats - 10
}

fn session_id(i: usize) -> String {
    format!("00000000-0000-4000-8000-{i:012}")
}

fn line(i: usize) -> String {
    let (h, m, s) = (i / 3600, i / 60 % 60, i % 60);
    format!(
        r#"{{"type":"user","isSidechain":false,"cwd":"/work/p{}","sessionId":"{}","timestamp":"2026-10-01T{h:02}:{m:02}:{s:02}.000Z","message":{{"role":"user","content":"chat {i} about the build"}}}}"#,
        i % 50,
        session_id(i)
    )
}

fn index_of(home: &Path, chats: usize) -> ChatIndex {
    let projects = home.join("projects");
    for i in 0..chats {
        let file: PathBuf =
            projects.join(format!("-work-p{}", i % 50)).join(format!("{}.jsonl", session_id(i)));
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, format!("{}\n", line(i))).unwrap();
    }
    let mut index = ChatIndex::new(vec![ChatRoot {
        harness: AdapterKind::ClaudeCode,
        path: projects.clone(),
        real_path: projects.canonicalize().unwrap(),
        source: RootSource::Default,
        aliases: Vec::new(),
        accounts: Vec::new(),
    }]);
    index.rescan_all();
    assert_eq!(index.chats().len(), chats);
    index
}

fn text_of(home: &Path, index: &ChatIndex, chats: usize) -> ChatText {
    let text = ChatText::new(home.join("text.json"));
    let held: Vec<IndexedChat> = index.chats().into_iter().cloned().collect();
    text.update(&held, &|entry| {
        let i: usize = entry.session_id.rsplit('-').next()?.parse().ok()?;
        let answer = if holds_word(i, chats) {
            "the zephyrine migration failed on two columns".to_owned()
        } else {
            format!("we talked about the build in chat {i}")
        };
        Some(vec![Turn {
            prompt: "what happened".into(),
            answer,
            prompt_at: None,
            answer_at: None,
        }])
    });
    text
}

type Costs = Vec<(usize, usize, Vec<String>)>;

/// What each keystroke of `typed` cost (row values built, chat texts searched, its rows), and the
/// keys of the chats whose text holds the word, in the index's order.
fn keystrokes(chats: usize, typed: &str) -> (Costs, Vec<String>) {
    let home = tempfile::tempdir().unwrap();
    let index = index_of(home.path(), chats);
    let text = text_of(home.path(), &index, chats);
    let holders = index
        .chats()
        .into_iter()
        .map(|chat| key_text(&chat.entry.key()))
        .filter(|key| {
            key.rsplit('-')
                .next()
                .and_then(|n| n.parse().ok())
                .is_some_and(|i| holds_word(i, chats))
        })
        .collect();
    let activity = ChatActivity::default();
    let costs = (1..=typed.len())
        .map(|end| {
            let spec = WindowSpec::from_params(&json!({"window": "all", "query": &typed[..end],
                "limit": LIMIT, "content": true, "untitled": "New chat"}))
            .unwrap();
            let (values, scanned) = (values_built(), text.scanned());
            let window = rows(&spec, &index, &activity, Some(&text));
            let keys = window.rows.into_iter().map(|(key, _)| key).collect();
            (values_built() - values, text.scanned() - scanned, keys)
        })
        .collect();
    (costs, holders)
}

/// A title query that every chat passes builds values for the rows it shows, not for every chat.
#[test]
fn a_title_query_builds_values_for_the_page_only_at_200_and_5000_chats() {
    let runs: Vec<(usize, Costs)> =
        [200, 5_000].into_iter().map(|chats| (chats, keystrokes(chats, "build").0)).collect();
    for (chats, costs) in &runs {
        let values: Vec<usize> = costs.iter().map(|(v, _, _)| *v).collect();
        println!(
            "all chats search, title query, {chats} chats: values built per keystroke {values:?}"
        );
    }
    for (chats, costs) in &runs {
        let (_, _, keys) = costs.last().unwrap();
        assert_eq!(keys.len(), LIMIT, "a full page");
        for (n, (values, _, _)) in costs.iter().enumerate() {
            assert!(
                *values <= LIMIT,
                "keystroke {} over {chats} chats built {values} row values; a page shows {LIMIT}",
                n + 1
            );
        }
    }
}

/// Typing a word said in three chats: each keystroke after the first searches only the chats the
/// one before found, and builds values only for the rows it shows.
#[test]
fn typing_a_said_word_searches_only_the_last_hits_at_200_and_5000_chats() {
    let runs: Vec<(usize, (Costs, Vec<String>))> =
        [200, 5_000].into_iter().map(|chats| (chats, keystrokes(chats, "zephyrine"))).collect();
    for (chats, (costs, _)) in &runs {
        let values: Vec<usize> = costs.iter().map(|(v, _, _)| *v).collect();
        let scanned: Vec<usize> = costs.iter().map(|(_, s, _)| *s).collect();
        let shown: Vec<usize> = costs.iter().map(|(_, _, keys)| keys.len()).collect();
        println!(
            "all chats search, said word, {chats} chats: values built per keystroke {values:?}, \
             chat texts searched per keystroke {scanned:?}, rows {shown:?}"
        );
    }
    for (chats, (costs, holders)) in &runs {
        let (_, _, keys) = costs.last().unwrap();
        assert_eq!(holders.len(), 3);
        assert_eq!(keys, holders, "the three chats, in the index's order (newest first)");
        for (n, (values, scanned, _)) in costs.iter().enumerate() {
            assert!(*values <= LIMIT, "keystroke {} built {values} row values", n + 1);
            if n > 0 {
                let before = costs[n - 1].2.len();
                assert!(
                    *scanned <= before,
                    "keystroke {} over {chats} chats searched {scanned} chat texts; the one \
                     before found {before}",
                    n + 1
                );
            }
        }
    }
}

/// A word said in a chat past the text held of it (its newest `CHAT_BYTES`) comes from the
/// transcript index: the window answers at once with what the held text found and stays filling,
/// then shows that chat too, its match carrying the message's time.
#[test]
fn a_word_past_the_held_text_comes_from_the_transcript_index_while_filling() {
    let chats = 200;
    let home = tempfile::tempdir().unwrap();
    let index = index_of(home.path(), chats);
    let text = text_of(home.path(), &index, chats);
    let old = 7;
    assert!(!holds_word(old, chats));
    text.deep().set_search(Box::new(move |ask| {
        assert_eq!(ask.words, ["zephyrine".to_owned()]);
        let hit = acpmux_transcript_search::Hit {
            session: session_id(old),
            harness: acpmux_transcript_search::Harness::Claude,
            path: "/p.jsonl".into(),
            subagent: false,
            turn: 1,
            line: 2,
            at_ms: Some(1_794_000_000_000),
            role: acpmux_transcript_search::Role::User,
            snippet: "long ago:\nthe zephyrine   plan".into(),
            match_start: 14,
            match_len: 9,
            cut_before: true,
            cut_after: false,
            message_id: None,
            matches: 1,
            in_window: false,
        };
        Some(vec![(hit, 2)])
    }));
    let mut moved = text.subscribe();
    let activity = ChatActivity::default();
    let spec = WindowSpec::from_params(&json!({"window": "all", "query": "zephyrine",
        "limit": LIMIT, "content": true, "untitled": "New chat"}))
    .unwrap();
    let first = rows(&spec, &index, &activity, Some(&text));
    assert!(first.filling, "the index has not answered yet");
    assert_eq!(first.rows.len(), 3, "what the held text found, at once");
    let runtime = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
    runtime.block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(10), moved.changed())
            .await
            .unwrap()
            .unwrap();
    });
    let next = rows(&spec, &index, &activity, Some(&text));
    assert!(!next.filling);
    assert_eq!(next.total, Some(4));
    let (_, row) = next
        .rows
        .iter()
        .find(|(key, _)| key.ends_with(&session_id(old)))
        .expect("the chat found by the index");
    let said = &row["match"];
    assert_eq!(said["at"], 1_794_000_000_000_i64);
    assert_eq!((said["turn"].as_u64(), said["role"].as_str()), (Some(0), Some("prompt")));
    assert_eq!(said["matches"], 2);
    assert_eq!(said["snippet"], "…long ago: the zephyrine plan", "cut where the index cut it");
    // The held text's own matches carry no time: the page reveals them as before.
    let held = next.rows.iter().find(|(key, _)| !key.ends_with(&session_id(old))).unwrap();
    assert!(held.1["match"].get("at").is_none());
}

/// The search syntax in the chats window: operators filter what the ranker and the content search
/// found; a plain query is the window it always was.
#[test]
fn operators_filter_the_window_by_role_exclusion_phrase_folder_harness_and_time() {
    let chats = 200;
    let home = tempfile::tempdir().unwrap();
    let index = index_of(home.path(), chats);
    let text = text_of(home.path(), &index, chats);
    let activity = ChatActivity::default();
    let window = |query: &str, extra: Value| -> WindowRows {
        let mut params = json!({"window": "all", "query": query, "limit": LIMIT, "content": true,
            "untitled": "New chat"});
        if let (Some(p), Some(e)) = (params.as_object_mut(), extra.as_object()) {
            p.extend(e.clone());
        }
        rows(&WindowSpec::from_params(&params).unwrap(), &index, &activity, Some(&text))
    };
    let count = |query: &str| window(query, json!({})).rows.len();
    assert_eq!(count("zephyrine"), 3);
    assert_eq!(count("zephyrine from:agent"), 3, "the word is in the answers");
    assert_eq!(count("zephyrine from:me"), 0, "and in no prompt");
    assert_eq!(count("zephyrine from:tool"), 0, "the held text has no tool calls");
    assert_eq!(count("zephyrine -columns"), 0);
    assert_eq!(count("zephyrine -\"two rows\""), 3);
    assert_eq!(count("\"migration failed\""), 3, "a phrase as written");
    assert_eq!(count("\"failed migration\""), 0, "not its words in another order");
    assert_eq!(count("zephyrine folder:P3"), 1, "/work/p3, any case");
    assert_eq!(count("zephyrine repo:p3"), 1);
    assert_eq!(count("zephyrine agent:codex"), 0);
    assert_eq!(count("zephyrine agent:claude"), 3);
    assert_eq!(count("zephyrine before:2000-01-01"), 0);
    assert_eq!(count("zephyrine since:2000-01-01"), 3);
    assert_eq!(window("zephyrine", json!({"sinceMs": i64::MAX / 2})).rows.len(), 0);
    assert_eq!(window("zephyrine", json!({"sinceMs": 0})).rows.len(), 3);
    // Operators alone list the newest chats they pass: /work/p3 and /work/p30..p39.
    let listed = window("folder:p3", json!({}));
    assert_eq!(listed.rows.len(), chats / 50 * 11);
    assert!(listed.rows.iter().all(|(_, v)| v["cwd"].as_str().unwrap().contains("/work/p3")));
    // A title match an exclusion names is dropped.
    assert_eq!(count("build"), LIMIT);
    assert_eq!(count("build -build"), 0);

    // The answer says how the query read when it holds an operator or a problem.
    let spec = WindowSpec::from_params(&json!({"window": "w", "query": "zephyrine from:agent",
        "content": true}))
    .unwrap();
    let mut state = WindowState::new(spec.clone(), 0);
    let message = state.next(rows(&spec, &index, &activity, Some(&text))).unwrap();
    assert_eq!(message["parsed"]["role"], "assistant");
    assert_eq!(message["parsed"]["chips"][0]["kind"], "from");
    let plain = WindowSpec::from_params(&json!({"window": "w", "query": "zephyrine"})).unwrap();
    let mut state = WindowState::new(plain.clone(), 0);
    let message = state.next(rows(&plain, &index, &activity, Some(&text))).unwrap();
    assert!(message.get("parsed").is_none(), "plain words: the message it always was");
    assert!(WindowSpec::from_params(&json!({"window": "w", "sinceMs": "soon"})).is_err());
    assert!(WindowSpec::from_params(&json!({"window": "w", "tzOffsetMinutes": 9999})).is_err());
}

/// The index is asked with the query's filters, and its hits outside the times are dropped.
#[test]
fn the_transcript_index_is_asked_with_the_operators() {
    let chats = 200;
    let home = tempfile::tempdir().unwrap();
    let index = index_of(home.path(), chats);
    let text = text_of(home.path(), &index, chats);
    let old = 7;
    let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = asked.clone();
    text.deep().set_search(Box::new(move |ask| {
        seen.lock().unwrap().push(ask.clone());
        let hit = acpmux_transcript_search::Hit {
            session: session_id(old),
            harness: acpmux_transcript_search::Harness::Claude,
            path: "/p.jsonl".into(),
            subagent: false,
            turn: 1,
            line: 2,
            at_ms: Some(1_794_000_000_000),
            role: acpmux_transcript_search::Role::Tool,
            snippet: "grep zephyrine".into(),
            match_start: 5,
            match_len: 9,
            cut_before: false,
            cut_after: false,
            message_id: None,
            matches: 1,
            in_window: false,
        };
        Some(vec![(hit, 1)])
    }));
    let mut moved = text.subscribe();
    let activity = ChatActivity::default();
    let spec = WindowSpec::from_params(&json!({"window": "all", "content": true, "limit": LIMIT,
        "query": "zephyrine -flaky from:tool agent:claude folder:work before:2026-12-01",
        "sinceMs": 5}))
    .unwrap();
    let first = rows(&spec, &index, &activity, Some(&text));
    assert!(first.filling && first.rows.is_empty(), "from:tool: the index alone answers");
    let runtime = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
    runtime.block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(10), moved.changed())
            .await
            .unwrap()
            .unwrap();
    });
    let next = rows(&spec, &index, &activity, Some(&text));
    assert_eq!(next.rows.len(), 1);
    assert!(next.rows[0].0.ends_with(&session_id(old)));
    let ask = asked.lock().unwrap()[0].clone();
    assert_eq!(ask.words, ["zephyrine"]);
    assert_eq!(ask.exclude, ["flaky"]);
    assert_eq!(ask.role, Some(acpmux_transcript_search::Role::Tool));
    assert_eq!(ask.harness, Some(acpmux_transcript_search::Harness::Claude));
    assert_eq!(ask.folder.as_deref(), Some("work"));
    assert_eq!(ask.since_ms, Some(5));
    assert!(ask.before_ms.is_some());
    // A hit the index gives past the window's times is not shown.
    let late = WindowSpec::from_params(&json!({"window": "all", "content": true,
        "query": "zephyrine from:tool before:2000-01-01"}))
    .unwrap();
    let _ = rows(&late, &index, &activity, Some(&text));
    runtime.block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(10), moved.changed())
            .await
            .unwrap()
            .unwrap();
    });
    assert!(rows(&late, &index, &activity, Some(&text)).rows.is_empty());
}
