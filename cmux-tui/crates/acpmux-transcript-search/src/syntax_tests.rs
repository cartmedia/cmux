use super::*;

/// 2026-11-18T21:55:00Z.
const NOW: i64 = 1_795_038_900_000;

fn clock() -> Clock {
    Clock { now_ms: NOW, tz_offset_min: 0 }
}

fn p(query: &str) -> Parsed {
    parse(query, clock())
}

fn terms(parsed: &Parsed) -> Vec<(&str, bool)> {
    parsed.terms.iter().map(|t| (t.text.as_str(), t.phrase)).collect()
}

fn utc(text: &str) -> i64 {
    crate::parse::parse_rfc3339_ms(text).unwrap()
}

#[test]
fn now_is_the_time_the_tests_say() {
    assert_eq!(utc("2026-11-18T21:55:00Z"), NOW);
}

#[test]
fn plain_words_are_terms_and_nothing_else() {
    let got = p("  four agent   pages ");
    assert_eq!(terms(&got), [("four", false), ("agent", false), ("pages", false)]);
    assert!(got.chips.is_empty() && got.problems.is_empty() && got.exclude.is_empty());
    assert_eq!(
        (got.role, got.harness, got.scope, got.since_ms, got.before_ms),
        (None, None, None, None, None)
    );
    assert!(!got.is_empty_text());
    assert!(p("").is_empty_text());
    assert!(p("   ").terms.is_empty());
}

#[test]
fn phrases_keep_their_spaces_and_an_unclosed_quote_runs_to_the_end() {
    let got = p(r#"fix "the four agent" pages"#);
    assert_eq!(terms(&got), [("fix", false), ("the four agent", true), ("pages", false)]);
    assert_eq!(got.chips.len(), 1);
    let chip = &got.chips[0];
    assert_eq!(
        (chip.kind, chip.value.as_str(), chip.start, chip.end),
        (ChipKind::Phrase, "the four agent", 4, 20)
    );
    assert_eq!(chip.label, "\"the four agent\"");

    let open = p(r#"a "b c  d"#);
    assert_eq!(terms(&open), [("a", false), ("b c  d", true)]);
    assert_eq!((open.chips[0].start, open.chips[0].end), (2, 9));
    // An empty phrase is nothing.
    assert!(p(r#""" x"#).chips.is_empty());
    assert_eq!(terms(&p(r#""" x"#)), [("x", false)]);
}

#[test]
fn minus_excludes_a_word_or_a_phrase_and_is_literal_otherwise() {
    let got = p(r#"build -flaky -"timed out" x"#);
    assert_eq!(terms(&got), [("build", false), ("x", false)]);
    assert_eq!(got.exclude, ["flaky", "timed out"]);
    assert_eq!(got.chips.iter().map(|c| c.kind).collect::<Vec<_>>(), [ChipKind::Exclude; 2]);
    assert_eq!(got.chips[0].label, "Not flaky");
    assert_eq!((got.chips[1].start, got.chips[1].end), (13, 25));

    let literal = p("- --verbose foo-bar a-");
    assert_eq!(
        terms(&literal),
        [("-", false), ("--verbose", false), ("foo-bar", false), ("a-", false)]
    );
    assert!(literal.exclude.is_empty() && literal.chips.is_empty());
}

#[test]
fn from_agent_in_and_folder_filter() {
    let got = p("from:me agent:codex in:this folder:cmux deploy");
    assert_eq!(got.role, Some(Role::User));
    assert_eq!(got.harness, Some(Harness::Codex));
    assert_eq!(got.scope, Some(Scope::This));
    assert_eq!(got.folder.as_deref(), Some("cmux"));
    assert_eq!(terms(&got), [("deploy", false)]);
    let kinds: Vec<_> =
        got.chips.iter().map(|c| (c.kind, c.value.as_str(), c.label.as_str())).collect();
    assert_eq!(
        kinds,
        [
            (ChipKind::From, "me", "From me"),
            (ChipKind::Agent, "codex", "Codex"),
            (ChipKind::In, "this", "This chat"),
            (ChipKind::Folder, "cmux", "Folder cmux"),
        ]
    );
    assert!(got.problems.is_empty());

    for (q, role) in [
        ("from:user", Role::User),
        ("from:agent", Role::Assistant),
        ("from:assistant", Role::Assistant),
        ("from:tool", Role::Tool),
    ] {
        assert_eq!(p(q).role, Some(role), "{q}");
        assert!(p(q).is_empty_text(), "{q}");
    }
    assert_eq!(p("in:all").scope, Some(Scope::All));
    assert_eq!(p("agent:claude").harness, Some(Harness::Claude));
    assert_eq!(p("repo:manaflow/cmux").folder.as_deref(), Some("manaflow/cmux"));
    assert_eq!(p(r#"folder:"my dir" x"#).folder.as_deref(), Some("my dir"));
}

#[test]
fn keys_and_values_are_any_case() {
    let got = p("FROM:Me Agent:CLAUDE In:All SINCE:Today");
    assert_eq!(got.role, Some(Role::User));
    assert_eq!(got.harness, Some(Harness::Claude));
    assert_eq!(got.scope, Some(Scope::All));
    assert_eq!(got.since_ms, Some(utc("2026-11-18T00:00:00Z")));
    assert!(got.terms.is_empty());
}

#[test]
fn bad_values_are_searched_as_written_and_say_why() {
    let got = p("from:bob agent:gpt in:here since:soon before: folder: x");
    assert_eq!(
        terms(&got),
        [
            ("from:bob", false),
            ("agent:gpt", false),
            ("in:here", false),
            ("since:soon", false),
            ("before:", false),
            ("folder:", false),
            ("x", false)
        ]
    );
    assert_eq!(got.problems.len(), 6, "{:?}", got.problems);
    assert!(got.problems[0].contains("from:bob"));
    assert!(got.problems[3].contains("not a time"));
    assert!(got.chips.is_empty());
    assert_eq!(
        (got.role, got.harness, got.scope, got.since_ms, got.before_ms),
        (None, None, None, None, None)
    );
    assert_eq!(got.folder, None);
}

#[test]
fn unknown_keys_urls_and_quoted_operators_are_literal() {
    let got = p(r#"error: foo:bar https://example.com/a?b=c "from:me" TODO:"#);
    assert_eq!(
        terms(&got),
        [
            ("error:", false),
            ("foo:bar", false),
            ("https://example.com/a?b=c", false),
            ("from:me", true),
            ("TODO:", false)
        ]
    );
    assert!(got.problems.is_empty());
    assert_eq!(got.role, None);
}

#[test]
fn since_and_before_read_spans_days_and_dates() {
    let c = clock();
    assert_eq!(p("since:7d").since_ms, Some(NOW - 7 * DAY_MS));
    assert_eq!(p("since:24h").since_ms, Some(NOW - 24 * HOUR_MS));
    assert_eq!(p("since:2w").since_ms, Some(NOW - 14 * DAY_MS));
    assert_eq!(p("since:30m").since_ms, Some(NOW - 30 * MINUTE_MS));
    assert_eq!(p("since:0d").since_ms, Some(NOW));
    assert_eq!(p("since:today").since_ms, Some(utc("2026-11-18T00:00:00Z")));
    assert_eq!(p("since:yesterday").since_ms, Some(utc("2026-11-17T00:00:00Z")));
    assert_eq!(p("since:2026-11-01").since_ms, Some(utc("2026-11-01T00:00:00Z")));
    assert_eq!(p("before:2026-11-01").before_ms, Some(utc("2026-11-01T00:00:00Z")));
    assert_eq!(p("before:today").before_ms, Some(utc("2026-11-18T00:00:00Z")));
    assert_eq!(p("before:7d").before_ms, Some(NOW - 7 * DAY_MS));
    let chip = &p("since:7D").chips[0];
    assert_eq!(
        (chip.kind, chip.value.as_str(), chip.label.as_str()),
        (ChipKind::Since, "7d", "Since 7d")
    );
    assert_eq!(p("before:today").chips[0].label, "Before today");
    // Not dates.
    for bad in [
        "since:2026-02-30",
        "since:2026-13-01",
        "since:7x",
        "since:d",
        "since:-1d",
        "since:2026-1-01",
    ] {
        let got = parse(bad, c);
        assert_eq!(got.since_ms, None, "{bad}");
        assert_eq!(got.problems.len(), 1, "{bad}");
        assert_eq!(terms(&got), [(bad, false)]);
    }
    assert_eq!(p("since:2024-02-29").since_ms, Some(utc("2024-02-29T00:00:00Z")));
    // Two starts: the later; two ends: the earlier.
    let both = p("since:7d since:24h before:today before:2026-11-01");
    assert_eq!(both.since_ms, Some(NOW - 24 * HOUR_MS));
    assert_eq!(both.before_ms, Some(utc("2026-11-01T00:00:00Z")));
}

#[test]
fn local_days_follow_the_offset() {
    // 21:55Z is 13:55 in UTC-8 (still the 18th) and 06:55 on the 19th in UTC+9.
    let west = Clock { now_ms: NOW, tz_offset_min: -480 };
    assert_eq!(parse("since:today", west).since_ms, Some(utc("2026-11-18T08:00:00Z")));
    assert_eq!(parse("since:yesterday", west).since_ms, Some(utc("2026-11-17T08:00:00Z")));
    assert_eq!(parse("since:2026-11-01", west).since_ms, Some(utc("2026-11-01T08:00:00Z")));
    let east = Clock { now_ms: NOW, tz_offset_min: 540 };
    assert_eq!(parse("since:today", east).since_ms, Some(utc("2026-11-18T15:00:00Z")));
    assert_eq!(parse("before:2026-11-19", east).before_ms, Some(utc("2026-11-18T15:00:00Z")));
    // Spans do not care.
    assert_eq!(parse("since:7d", east).since_ms, Some(NOW - 7 * DAY_MS));
}

#[test]
fn spans_are_utf16_with_emoji_and_cjk() {
    // 😀 is two UTF-16 units, 検索 two (one each).
    let got = p("😀 from:me 検索 \"日本 語\" -😀x");
    assert_eq!(terms(&got), [("😀", false), ("検索", false), ("日本 語", true)]);
    let spans: Vec<_> = got.chips.iter().map(|c| (c.kind, c.start, c.end)).collect();
    assert_eq!(
        spans,
        [(ChipKind::From, 3, 10), (ChipKind::Phrase, 14, 20), (ChipKind::Exclude, 21, 25)]
    );
    assert_eq!(got.exclude, ["😀x"]);
    let utf16: Vec<u16> = "😀 from:me 検索 \"日本 語\" -😀x".encode_utf16().collect();
    assert_eq!(String::from_utf16(&utf16[3..10]).unwrap(), "from:me");
    assert_eq!(String::from_utf16(&utf16[14..20]).unwrap(), "\"日本 語\"");
    assert_eq!(utf16.len(), 25);
}

#[test]
fn apply_fills_the_query_and_keeps_the_narrower_times() {
    let got =
        p(r#"deploy "the fix" -flaky from:agent agent:claude folder:cmux since:7d before:today"#);
    let mut q = Query::new("");
    q.since_ms = Some(NOW - DAY_MS);
    q.before_ms = Some(utc("2026-11-19T00:00:00Z"));
    q.exclude = vec!["kept".into()];
    got.apply(&mut q);
    assert_eq!(q.text, "deploy");
    assert_eq!(q.also, ["the fix"]);
    assert_eq!(q.exclude, ["kept", "flaky"]);
    assert_eq!(q.role, Some(Role::Assistant));
    assert_eq!(q.harness, Some(Harness::Claude));
    assert_eq!(q.folder.as_deref(), Some("cmux"));
    assert_eq!(q.since_ms, Some(NOW - DAY_MS), "the later start");
    assert_eq!(q.before_ms, Some(utc("2026-11-18T00:00:00Z")), "the earlier end");

    // No filter of its own keeps the query's.
    let mut q = Query::new("old");
    q.role = Some(Role::Tool);
    p("new").apply(&mut q);
    assert_eq!((q.text.as_str(), q.role), ("new", Some(Role::Tool)));
}

#[test]
fn to_json_is_the_wire_shape() {
    let got = p(r#"a "b c" -d from:me agent:codex in:all folder:x since:2026-11-01 before:today"#);
    let v = got.to_json();
    assert_eq!(
        v["terms"],
        json!([{"text": "a", "phrase": false}, {"text": "b c", "phrase": true}])
    );
    assert_eq!(v["exclude"], json!(["d"]));
    assert_eq!(v["role"], "user");
    assert_eq!(v["agent"], "codex");
    assert_eq!(v["in"], "all");
    assert_eq!(v["folder"], "x");
    assert_eq!(v["sinceMs"], utc("2026-11-01T00:00:00Z"));
    assert_eq!(v["beforeMs"], utc("2026-11-18T00:00:00Z"));
    assert_eq!(
        v["chips"][0],
        json!({"kind": "phrase", "value": "b c", "label": "\"b c\"", "start": 2, "end": 7})
    );
    assert_eq!(v["chips"].as_array().unwrap().len(), 8);
    assert_eq!(v["problems"], json!([]));
    let plain = p("x").to_json();
    for k in ["role", "agent", "in", "folder", "sinceMs", "beforeMs"] {
        assert!(plain.get(k).is_none(), "{k}");
    }
}

fn inserts(query: &str, cursor: usize) -> Vec<String> {
    complete(query, cursor).into_iter().map(|c| c.insert).collect()
}

#[test]
fn completes_keys_from_a_prefix() {
    assert_eq!(inserts("fr", 2), ["from:"]);
    assert_eq!(inserts("deploy S", 8), ["since:"]);
    assert_eq!(inserts("b", 1), ["before:"]);
    assert_eq!(inserts("f", 1), ["from:", "folder:"]);
    let all = complete("x a", 3);
    assert_eq!(all.len(), 1);
    assert_eq!((all[0].insert.as_str(), all[0].start, all[0].end), ("agent:", 2, 3));
    assert!(!all[0].detail.is_empty());
    // Mid-token: what is before the cursor, the whole token replaced.
    let mid = complete("frxx", 2);
    let got: Vec<_> = mid.iter().map(|c| (c.insert.as_str(), c.start, c.end)).collect();
    assert_eq!(got, [("from:", 0, 4)]);
}

#[test]
fn completes_values_after_a_key() {
    assert_eq!(inserts("from:", 5), ["from:me ", "from:agent ", "from:tool "]);
    assert_eq!(inserts("from:a", 6), ["from:agent "]);
    assert_eq!(inserts("agent:", 6), ["agent:claude ", "agent:codex "]);
    assert_eq!(inserts("in:", 3), ["in:this ", "in:all "]);
    assert_eq!(
        inserts("x since:", 8),
        ["since:today ", "since:yesterday ", "since:24h ", "since:7d ", "since:30d "]
    );
    assert_eq!(inserts("since:y", 7), ["since:yesterday "]);
    assert_eq!(inserts("before:", 7).len(), 4);
    // The key is lowercased in what is inserted.
    assert_eq!(inserts("FROM:m", 6), ["from:me "]);
    let c = &complete("deploy agent:c", 14)[0];
    assert_eq!(
        (c.start, c.end, c.label.as_str(), c.detail.as_str()),
        (7, 14, "agent:claude", "Claude Code")
    );
    // Typed whole: nothing more to offer.
    assert!(complete("from:me", 7).is_empty());
}

#[test]
fn offers_nothing_where_nothing_fits() {
    assert!(complete("", 0).is_empty());
    assert!(complete("x ", 2).is_empty());
    assert!(complete("zzz", 3).is_empty());
    assert!(complete("\"fr", 3).is_empty());
    assert!(complete("-fr", 3).is_empty());
    assert!(complete("folder:", 7).is_empty());
    assert!(complete("error:", 6).is_empty());
    assert!(complete("fr", 99).is_empty());
}

#[test]
fn completion_spans_are_utf16() {
    let c = complete("😀 検 fr", 7);
    assert_eq!(c.len(), 1);
    assert_eq!((c[0].start, c[0].end), (5, 7));
}
