use serde_json::json;

use super::*;
use crate::store::{EventRecord, MemoryStore, Store};

#[test]
fn params_read_the_wire_and_bound_the_limit_and_context() {
    let p = Params::from_value(&json!({
        "query": "four agent pages", "sessionId": "coord", "role": "assistant",
        "sinceMs": 5, "limit": 5000, "context": 99_999, "host": "all",
        "order": "transcript", "window": {"fromMs": 10, "toMs": 20}, "offset": 40,
    }))
    .unwrap();
    assert_eq!(p.query, "four agent pages");
    assert_eq!(p.session.as_deref(), Some("coord"));
    assert_eq!(p.limit, MAX_LIMIT);
    assert_eq!(p.context, MAX_CONTEXT);
    assert_eq!(p.host.as_deref(), Some("all"));
    assert_eq!((p.order, p.window, p.offset), (ts::Order::Transcript, Some((10, 20)), 40));
    assert_eq!(Params::from_value(&p.to_value()).unwrap(), p);

    let bare = Params::from_value(&json!({"query": "x", "role": null, "host": ""})).unwrap();
    assert_eq!(
        (bare.limit, bare.context, bare.host.clone()),
        (DEFAULT_LIMIT, DEFAULT_CONTEXT, None)
    );
    assert_eq!((bare.order, bare.window, bare.offset), (ts::Order::Newest, None, 0));
    assert_eq!(Params::from_value(&bare.to_value()).unwrap(), bare);
    assert!(Params::from_value(&json!({"query": "  "})).is_err());
    assert!(Params::from_value(&json!({"query": "x", "role": "system"})).is_err());
    assert!(Params::from_value(&json!({"query": "x", "order": "oldest"})).is_err());
    assert!(
        Params::from_value(&json!({"query": "x", "window": {"fromMs": 9, "toMs": 1}})).is_err()
    );
    assert!(Params::from_value(&json!({"query": "x", "window": {"fromMs": 9}})).is_err());
    let far = Params::from_value(&json!({"query": "x", "offset": u64::MAX})).unwrap();
    assert_eq!(far.offset, MAX_OFFSET);
}

/// Several machines each answer from their first hit, deep enough for the page; the merge skips.
#[test]
fn a_search_of_several_machines_asks_each_from_its_first_hit() {
    let p = Params { query: "x".into(), limit: 20, offset: 30, ..Params::default() };
    let each = p.for_merge();
    assert_eq!((each.offset, each.limit), (0, 50));
    let deep = Params { offset: 500, ..p }.for_merge();
    assert_eq!(deep.limit, MAX_LIMIT);
}

#[test]
fn merge_puts_every_hosts_hits_newest_first_up_to_the_limit() {
    let here = json!({"hits": [{"host": "a", "at": 30}, {"host": "a", "at": 10}],
        "files": 2, "caughtUpBytes": 100, "catchUpMs": 4, "queryMs": 2, "unreached": [], "total": 2});
    let peer = json!({"hits": [{"host": "b", "at": 20}, {"host": "b", "at": null}],
        "files": 3, "caughtUpBytes": 0, "catchUpMs": 9, "queryMs": 1, "unreached": [], "total": 7});
    let p = Params { limit: 3, ..Params::default() };
    let merged = merge(
        vec![here.clone(), peer.clone()],
        vec![json!({"host": "c", "error": "down"})],
        &p,
        0,
        12,
    );
    let order: Vec<_> =
        merged["hits"].as_array().unwrap().iter().map(|h| h["at"].clone()).collect();
    assert_eq!(order, [json!(30), json!(20), json!(10)]);
    assert_eq!(merged["files"], 5);
    assert_eq!(merged["caughtUpBytes"], 100);
    assert_eq!(merged["catchUpMs"], 9);
    assert_eq!(merged["tookMs"], 12);
    assert_eq!(merged["unreached"][0]["host"], "c");
    assert_eq!(merged["total"], 9);

    let skipped = merge(vec![here.clone(), peer], vec![], &p, 1, 0);
    let order: Vec<_> =
        skipped["hits"].as_array().unwrap().iter().map(|h| h["at"].clone()).collect();
    assert_eq!(order, [json!(20), json!(10), json!(null)], "the page past the first hit");
    let uncounted = json!({"hits": [], "unreached": []});
    assert!(merge(vec![here, uncounted], vec![], &p, 0, 0).get("total").is_none());
    let unreached = merge(vec![], vec![json!({"host": "c", "error": "down"})], &p, 0, 0);
    assert!(unreached.get("total").is_none(), "no machine answered: nothing was counted");
}

#[test]
fn merge_keeps_transcript_order_and_the_window_first() {
    let hit = |path: &str, line: u64, inside: bool| {
        let mut h = json!({"path": path, "line": line, "matchStart": 0, "at": line});
        if inside {
            h["inWindow"] = json!(true);
        }
        h
    };
    let here = json!({"hits": [hit("/b", 1, false), hit("/a", 9, true)]});
    let peer = json!({"hits": [hit("/a", 2, false), hit("/a", 5, true)]});
    let p = Params { limit: 10, order: ts::Order::Transcript, ..Params::default() };
    let merged = merge(vec![here, peer], vec![], &p, 0, 0);
    let got: Vec<_> = merged["hits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| (h["path"].as_str().unwrap().to_owned(), h["line"].as_u64().unwrap()))
        .collect();
    let want: Vec<(String, u64)> =
        [("/a", 5), ("/a", 9), ("/a", 2), ("/b", 1)].map(|(p, l)| (p.to_owned(), l)).to_vec();
    assert_eq!(got, want);
}

fn rec(seq: u64, at: u64, dir: &str, kind: &str, msg: Value) -> EventRecord {
    EventRecord { seq, at, dir: dir.into(), kind: kind.into(), msg, host_seq: None }
}

fn logged(records: &[EventRecord]) -> MemoryStore {
    let store = MemoryStore::default();
    let meta = serde_json::from_value(json!({
        "schema": crate::store::META_SCHEMA, "id": "c", "name": "c", "harness": "claude",
        "family": "claude", "cwd": "/tmp", "status": "idle", "createdAt": 1, "updatedAt": 1,
    }))
    .unwrap();
    store.save(&meta).unwrap();
    for r in records {
        store.append("c", r).unwrap();
    }
    store
}

/// A hit of the chat's own transcript takes the seq of the logged message holding it nearest its
/// time; one the log never held, or another machine's, takes none.
#[test]
fn hits_are_placed_at_the_logged_message_that_holds_them() {
    let min = 60_000;
    let chunk = |seq: u64, at: u64, id: &str, text: &str| {
        rec(
            seq,
            at,
            "in",
            "agent_message_chunk",
            json!({"params": {"update": {
            "sessionUpdate": "agent_message_chunk", "messageId": id,
            "content": {"type": "text", "text": text}}}}),
        )
    };
    let store = logged(&[
        rec(1, 10 * min, "mux", "user_message", json!({"text": "fix the Parser bug"})),
        chunk(2, 10 * min + 500, "m1", "The parser "),
        chunk(3, 10 * min + 600, "m1", "bug is fixed."),
        rec(4, 50 * min, "mux", "user_message", json!({"text": "again: the parser bug"})),
        rec(5, 200 * min, "mux", "user_message", json!({"text": "something else entirely"})),
    ]);
    let hit =
        |role: &str, at: Option<u64>, host: &str| json!({"host": host, "role": role, "at": at});
    let mut hits = vec![
        hit("user", Some(10 * min + 2_000), "here"),
        hit("assistant", Some(12 * min), "here"),
        hit("user", Some(50 * min + 900), "here"),
        hit("user", Some(300 * min), "here"),
        hit("user", None, "here"),
        hit("user", Some(10 * min), "peer"),
        // Its text is past what the log keeps of a message: the nearest prompt in two minutes.
        hit("user", Some(200 * min + 30_000), "here"),
    ];
    let ours = |h: &Value| h["host"] == "here";
    place(&store, "c", "parser BUG", &mut hits, &ours).unwrap();
    let seqs: Vec<Option<u64>> =
        hits.iter().map(|h| h.get("seq").and_then(Value::as_u64)).collect();
    assert_eq!(seqs, [Some(1), Some(2), Some(4), None, None, None, Some(5)]);
}

/// The search syntax's params (`caseSensitive`, `beforeMs`, `tzOffsetMinutes`) and how a query
/// with operators becomes the index query.
#[test]
fn params_read_the_search_syntax_and_its_toggles() {
    let p = Params::from_value(&json!({
        "query": "deploy \"the fix\" -flaky from:agent agent:codex folder:cmux since:7d",
        "caseSensitive": true, "beforeMs": 99, "tzOffsetMinutes": -480, "sinceMs": 5,
        "role": "user",
    }))
    .unwrap();
    assert!(p.case_sensitive);
    assert_eq!((p.before_ms, p.tz_offset_min), (Some(99), Some(-480)));
    assert_eq!(Params::from_value(&p.to_value()).unwrap(), p);
    let parsed = p.parsed();
    let q = p.query(&parsed);
    assert_eq!(q.text, "deploy");
    assert_eq!(q.also, ["the fix"]);
    assert_eq!(q.exclude, ["flaky"]);
    assert_eq!(q.role, Some(ts::Role::Assistant), "from: over role");
    assert_eq!(q.harness, Some(ts::Harness::Codex));
    assert_eq!(q.folder.as_deref(), Some("cmux"));
    assert!(q.since_ms.unwrap() > 5, "the later start");
    assert_eq!(q.before_ms, Some(99));
    assert!(q.case_sensitive);
    assert!(parsed.problems.is_empty());

    // Plain words read as they always did.
    let plain = Params::from_value(&json!({"query": "four agent pages"})).unwrap();
    let q = plain.query(&plain.parsed());
    assert_eq!(
        (q.text.as_str(), q.also.clone()),
        ("four", vec!["agent".to_owned(), "pages".to_owned()])
    );
    assert!(!q.case_sensitive && q.exclude.is_empty() && q.before_ms.is_none());

    for bad in [
        json!({"query": "x", "caseSensitive": "yes"}),
        json!({"query": "x", "tzOffsetMinutes": 5000}),
        json!({"query": "x", "tzOffsetMinutes": 1.5}),
    ] {
        assert!(Params::from_value(&bad).is_err(), "{bad}");
    }
}

/// `in:` never widens a search on the daemon: pinned to a chat, `in:all` is a problem.
#[test]
fn in_all_never_widens_a_search_of_one_chat() {
    let pinned = Params::from_value(&json!({"query": "x in:all", "sessionId": "s"})).unwrap();
    let parsed = pinned.parsed();
    assert_eq!(parsed.scope, Some(ts::Scope::All));
    assert_eq!(parsed.problems.len(), 1);
    assert_eq!(pinned.query(&parsed).sessions, None, "the hub sets it from sessionId");
    let harness =
        Params::from_value(&json!({"query": "x in:all", "harnessSessionId": "h"})).unwrap();
    let parsed = harness.parsed();
    assert_eq!(parsed.problems.len(), 1);
    assert_eq!(harness.query(&parsed).sessions, Some(vec!["h".to_owned()]));
    let this = Params::from_value(&json!({"query": "x in:this"})).unwrap();
    assert_eq!(this.parsed().problems.len(), 1);
    assert!(
        Params::from_value(&json!({"query": "x in:this", "sessionId": "s"}))
            .unwrap()
            .parsed()
            .problems
            .is_empty()
    );
    assert!(
        Params::from_value(&json!({"query": "x in:all"})).unwrap().parsed().problems.is_empty()
    );
}

#[test]
fn search_syntax_answers_how_a_query_reads_and_what_comes_next() {
    let got = syntax(&json!({"query": "deploy fr", "tzOffsetMinutes": 60})).unwrap();
    assert_eq!(got["parsed"]["terms"].as_array().unwrap().len(), 2);
    assert_eq!(
        got["completions"],
        json!([{"insert": "from:", "label": "from:", "detail": "Who said it: me or agent", "start": 7, "end": 9}])
    );
    let at = syntax(&json!({"query": "since: x", "cursor": 6})).unwrap();
    assert_eq!(at["completions"].as_array().unwrap().len(), 5);
    assert_eq!(at["parsed"]["problems"].as_array().unwrap().len(), 1);
    let chips = syntax(&json!({"query": "from:me -flaky"})).unwrap();
    assert_eq!(chips["parsed"]["role"], "user");
    assert_eq!(chips["parsed"]["chips"][1]["kind"], "exclude");
    assert!(syntax(&json!({})).is_err());
    assert!(syntax(&json!({"query": "x", "cursor": "end"})).is_err());
    assert!(syntax(&json!({"query": "x", "tzOffsetMinutes": 9999})).is_err());
    // An empty query reads as nothing (a search box before its first key).
    let empty = syntax(&json!({"query": ""})).unwrap();
    assert_eq!(empty["completions"], json!([]));
}
