use crate::activity_view::*;

const HOUR: i64 = 60 * 60 * 1000;
const DAY: i64 = 24 * HOUR;
/// 2026-11-18 15:00 UTC.
const NOW: i64 = 1_795_014_000_000;

fn item(id: &str, attention: Attention, recency_ms: i64) -> Item {
    Item {
        id: id.into(),
        attention,
        recency_ms,
        can_mark_read: true,
        can_archive: true,
        ..Item::default()
    }
}

fn request(items: Vec<Item>, snapshot: Option<Snapshot>) -> Request {
    Request { items, snapshot, now_ms: NOW, ..Request::default() }
}

fn ids(section: &Section) -> Vec<&str> {
    section.ids.iter().map(String::as_str).collect()
}

#[test]
fn tiers_match_codex() {
    use Attention::*;
    assert_eq!([Input, Approval, Error, Waiting].map(Attention::tier), [0; 4]);
    assert_eq!((Unread.tier(), Active.tier(), Idle.tier()), (1, 2, 3));
    assert!(Unread.needs_attention() && !Active.needs_attention());
}

#[test]
fn opening_ranks_by_tier_then_newest() {
    let plan = plan(&request(
        vec![
            item("idle", Attention::Idle, NOW - HOUR),
            item("active", Attention::Active, NOW - 1),
            item("unread-old", Attention::Unread, NOW - 3 * HOUR),
            item("unread-new", Attention::Unread, NOW - 2 * HOUR),
            item("input", Attention::Input, NOW - 5 * HOUR),
            item("error", Attention::Error, NOW - 4 * HOUR),
        ],
        None,
    ));
    assert_eq!(plan.snapshot.activated_ms, NOW);
    assert_eq!(plan.sections[0].kind, SectionKind::Priority);
    assert!(plan.sections[0].actions);
    assert_eq!(ids(&plan.sections[0]), ["error", "input", "unread-new", "unread-old", "active"]);
    // The read chat lists under Today, not in Priority.
    assert_eq!(plan.sections[1].kind, SectionKind::Day);
    assert_eq!(plan.sections[1].days_ago, Some(0));
    assert_eq!(ids(&plan.sections[1]), ["idle"]);
    assert!(!plan.sections[1].actions);
}

#[test]
fn snapshot_never_resorts_and_read_chats_stay() {
    let first = plan(&request(
        vec![
            item("a", Attention::Unread, NOW - 2 * HOUR),
            item("b", Attention::Input, NOW - 3 * HOUR),
        ],
        None,
    ));
    assert_eq!(first.snapshot.ids, ["b", "a"]);
    // b was answered and is now idle, a became input: no re-sort, b stays shown idle.
    let next = plan(&request(
        vec![item("a", Attention::Input, NOW), item("b", Attention::Idle, NOW)],
        Some(first.snapshot),
    ));
    assert_eq!(ids(&next.sections[0]), ["b", "a"]);
    assert!(next.can_clear_read);
}

#[test]
fn new_chats_go_on_top_and_newly_needy_ones_append() {
    let first = plan(&request(
        vec![item("a", Attention::Unread, NOW - HOUR), item("old", Attention::Idle, NOW - HOUR)],
        None,
    ));
    let mut created = item("new", Attention::Idle, NOW + 1);
    created.created_ms = Some(NOW + 1);
    let next = plan(&Request {
        now_ms: NOW + 10,
        ..request(
            vec![
                item("a", Attention::Unread, NOW - HOUR),
                item("old", Attention::Error, NOW + 2),
                created,
            ],
            Some(first.snapshot),
        )
    });
    assert_eq!(ids(&next.sections[0]), ["new", "a", "old"]);
    assert_eq!(next.snapshot.activated_ms, NOW, "the snapshot keeps its opening time");
}

#[test]
fn clear_read_drops_idle_and_keeps_order() {
    let first = plan(&request(
        vec![
            item("a", Attention::Input, NOW - HOUR),
            item("b", Attention::Unread, NOW - HOUR),
            item("c", Attention::Active, NOW - HOUR),
        ],
        None,
    ));
    let items = vec![
        item("a", Attention::Idle, NOW),
        item("b", Attention::Unread, NOW - HOUR),
        item("c", Attention::Input, NOW - HOUR),
    ];
    let cleared = clear_read(&request(items, Some(first.snapshot)));
    assert_eq!(ids(&cleared.sections[0]), ["b", "c"]);
    assert!(!cleared.can_clear_read);
    // The read chat lists under Today again.
    assert_eq!(ids(&cleared.sections[1]), ["a"]);
}

#[test]
fn chats_read_before_today_and_gone_chats_drop_out() {
    let first = Snapshot {
        ids: vec!["stale".into(), "gone".into(), "kept".into()],
        activated_ms: NOW - 2 * DAY,
    };
    let next = plan(&request(
        vec![
            item("stale", Attention::Idle, NOW - 2 * DAY),
            item("kept", Attention::Idle, NOW - HOUR),
        ],
        Some(first),
    ));
    assert_eq!(ids(&next.sections[0]), ["kept"]);
}

#[test]
fn day_groups_cover_seven_calendar_days() {
    let today = start_of_day(NOW, 0);
    let plan = plan(&Request {
        options: Options { show_priority: false, show_pinned: false },
        ..request(
            vec![
                item("today", Attention::Idle, today + HOUR),
                item("yesterday", Attention::Unread, today - 1),
                item("six", Attention::Idle, today - 6 * DAY + 1),
                item("seven", Attention::Idle, today - 6 * DAY - 1),
            ],
            None,
        )
    });
    let days: Vec<_> = plan.sections.iter().map(|s| (s.days_ago, ids(s))).collect();
    assert_eq!(
        days,
        [(Some(0), vec!["today"]), (Some(1), vec!["yesterday"]), (Some(6), vec!["six"])]
    );
    // With Priority off the first section carries the menu.
    assert!(plan.sections[0].actions);
    assert!(!plan.can_clear_read && plan.mark_read_ids.is_empty());
}

#[test]
fn time_zone_moves_the_day_line() {
    // 15:00 UTC is 00:00 the next day at UTC+9.
    assert_eq!(start_of_day(NOW, 9 * 60), NOW);
    assert_eq!(start_of_day(NOW, 0), NOW - 15 * HOUR);
    assert_eq!(start_of_day(NOW, -8 * 60), NOW - 7 * HOUR);
}

#[test]
fn pinned_section_takes_pinned_chats() {
    let mut pinned = item("pin", Attention::Unread, NOW - HOUR);
    pinned.pinned = true;
    let items = vec![pinned, item("a", Attention::Input, NOW)];
    let off = plan(&request(items.clone(), None));
    assert_eq!(off.sections.len(), 1);
    assert_eq!(ids(&off.sections[0]), ["a", "pin"]);
    let on = plan(&Request {
        options: Options { show_priority: true, show_pinned: true },
        ..request(items, None)
    });
    assert_eq!(on.sections[1].kind, SectionKind::Pinned);
    assert_eq!(ids(&on.sections[0]), ["a"]);
    assert_eq!(ids(&on.sections[1]), ["pin"]);
}

#[test]
fn empty_view_keeps_priority_header_or_a_lone_today() {
    let empty = plan(&request(Vec::new(), None));
    assert_eq!(empty.sections.len(), 1);
    assert_eq!(empty.sections[0].kind, SectionKind::Priority);
    assert!(empty.sections[0].ids.is_empty() && empty.sections[0].actions);
    let off = plan(&Request {
        options: Options { show_priority: false, show_pinned: false },
        ..request(Vec::new(), None)
    });
    assert_eq!(off.sections.len(), 1);
    assert_eq!((off.sections[0].kind, off.sections[0].days_ago), (SectionKind::Day, Some(0)));
    assert!(off.sections[0].actions);
}

#[test]
fn pages_of_ten_across_sections() {
    let items: Vec<Item> = (0..25_i64)
        .map(|n| {
            item(
                &format!("c{n:02}"),
                if n < 8 { Attention::Unread } else { Attention::Idle },
                NOW - n * 1000,
            )
        })
        .collect();
    let first = plan(&request(items.clone(), None));
    assert_eq!((first.shown, first.total, first.has_more), (10, 25, true));
    assert_eq!(first.sections[0].ids.len(), 8);
    assert_eq!(first.sections[1].ids.len(), 2);
    let all = plan(&Request { visible: 30, ..request(items, None) });
    assert_eq!((all.shown, all.has_more), (25, false));
}

#[test]
fn bulk_actions_name_priority_chats_only() {
    let mut question = item("ask", Attention::Input, NOW);
    question.can_mark_read = false;
    let mut remote = item("remote", Attention::Unread, NOW);
    remote.can_archive = false;
    let plan = plan(&request(
        vec![
            question,
            remote,
            item("run", Attention::Active, NOW),
            item("recent", Attention::Idle, NOW),
        ],
        None,
    ));
    assert_eq!(plan.mark_read_ids, ["remote"]);
    assert_eq!(plan.archive_ids, ["ask", "run"]);
    assert_eq!(plan.running_ids, ["run"]);
}

#[test]
fn request_and_plan_round_trip_json() {
    let raw = r#"{"items":[{"id":"a","attention":"approval","recency_ms":1}],"now_ms":2}"#;
    let request: Request = serde_json::from_str(raw).expect("request");
    let json = serde_json::to_value(plan(&request)).expect("plan");
    assert_eq!(json["sections"][0]["kind"], "priority");
    assert_eq!(json["sections"][0]["ids"][0], "a");
    assert!(json["sections"][0].get("days_ago").is_none());
}

#[test]
fn ten_thousand_chats_plan_in_one_pass() {
    let items: Vec<Item> = (0..10_000)
        .map(|n: i64| {
            let attention = match n % 7 {
                0 => Attention::Input,
                1 => Attention::Unread,
                2 => Attention::Active,
                _ => Attention::Idle,
            };
            item(&format!("chat-{n}"), attention, NOW - n * 60_000)
        })
        .collect();
    let started = std::time::Instant::now();
    let first = plan(&request(items.clone(), None));
    let next = plan(&request(items, Some(first.snapshot.clone())));
    // Hashed lookups and one sort: well under a second even unoptimized (quadratic would be minutes).
    assert!(started.elapsed() < std::time::Duration::from_secs(2), "{:?}", started.elapsed());
    assert_eq!(first.snapshot, next.snapshot);
    assert_eq!(next.shown, PAGE);
}
