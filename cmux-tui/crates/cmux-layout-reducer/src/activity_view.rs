//! The sidebar Priority view's activity mode (`sidebar.priorityView.style` = `activity`): the
//! Codex desktop app's activity view, as its shipped bundle builds it. While it is on it replaces
//! the whole sidebar list with a Priority section (the chats that need the person, then the ones
//! working, then read ones), an optional Pinned section, then the last seven calendar days in
//! Today / Yesterday / weekday groups.
//!
//! The Priority list is a snapshot taken when the view opens: it never re-sorts while it shows.
//! New chats go on top, newly needy chats are appended at the end, and a chat the person read stays
//! (shown idle) until Clear read chats or the view is turned off. The app holds the snapshot and
//! passes it back with each plan; the rule lives here, the words and the rows in the app.
//!
//! The app reaches it through `cmux_layout_reducer_json` (`activity_view.plan`,
//! `activity_view.clear_read`). A plan is O(n log n) in the chats: one sort and hashed lookups.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// How many rows the view shows before its next page, and how many each page adds (Codex's
/// `visibleItemCount`).
pub const PAGE: usize = 10;
/// The recent groups' span in calendar days, today included.
pub const RECENT_DAYS: i64 = 7;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// What a chat needs, as Codex ranks it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Attention {
    /// A question waits for the person.
    Input,
    /// A permission prompt waits for the person.
    Approval,
    /// The last turn failed or the agent is gone.
    Error,
    /// Waiting on the person some other way (red checks on its pull request).
    Waiting,
    /// A result nobody has read.
    Unread,
    /// A turn runs.
    Active,
    #[default]
    Idle,
}

impl Attention {
    /// Codex's tiers: `{input:0,approval:0,error:0,waiting:0,unread:1,active:2,idle:3}`.
    #[must_use]
    pub const fn tier(self) -> u8 {
        match self {
            Self::Input | Self::Approval | Self::Error | Self::Waiting => 0,
            Self::Unread => 1,
            Self::Active => 2,
            Self::Idle => 3,
        }
    }

    /// Needs attention: input, approval, error, waiting or unread.
    #[must_use]
    pub const fn needs_attention(self) -> bool {
        self.tier() <= 1
    }
}

/// One chat the sidebar lists.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Item {
    pub id: String,
    pub attention: Attention,
    /// When it last changed, milliseconds since the epoch `now_ms` uses.
    pub recency_ms: i64,
    /// When it started, when known: a chat started after the view opened goes on top.
    pub created_ms: Option<i64>,
    pub pinned: bool,
    /// Mark as Read applies to it (a result or a failure, not a question or red checks).
    pub can_mark_read: bool,
    /// Archive applies to it (its machine can archive it).
    pub can_archive: bool,
}

/// The Show options (Codex's options menu).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    /// The Priority section (default on).
    pub show_priority: bool,
    /// The Pinned section (default off).
    pub show_pinned: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { show_priority: true, show_pinned: false }
    }
}

/// The Priority list taken when the view opened, in its order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Snapshot {
    pub ids: Vec<String>,
    /// When the view opened, milliseconds since the epoch.
    pub activated_ms: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Request {
    pub items: Vec<Item>,
    /// The snapshot the last plan returned; none when the view just opened.
    pub snapshot: Option<Snapshot>,
    pub options: Options,
    pub now_ms: i64,
    /// The local time zone's offset from UTC in minutes (east positive), for the day groups.
    pub tz_offset_minutes: i32,
    /// How many rows show (the pages loaded); zero means one page.
    pub visible: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionKind {
    Priority,
    Pinned,
    /// One calendar day of the recent groups: `days_ago` 0 is Today, 1 Yesterday, else its weekday.
    Day,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section {
    pub kind: SectionKind,
    /// For a day group: how many calendar days before today.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days_ago: Option<i64>,
    /// The rows it shows, top first, cut to the visible pages.
    pub ids: Vec<String>,
    /// Its header carries the options menu and Clear read chats: the Priority header, else the
    /// first section's.
    pub actions: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    /// The snapshot to pass to the next plan.
    pub snapshot: Snapshot,
    pub sections: Vec<Section>,
    /// Rows shown and rows in all; `has_more` shows the next-page row.
    pub shown: usize,
    pub total: usize,
    pub has_more: bool,
    /// Clear read chats shows: the Priority section is on and one of its chats is idle (read).
    pub can_clear_read: bool,
    /// The Priority chats Mark all as read marks (none: the item greys).
    pub mark_read_ids: Vec<String>,
    /// The Priority chats Archive chats archives (recent chats never are), and the ones of them
    /// whose turn runs (the confirmation then says Stop and archive).
    pub archive_ids: Vec<String>,
    pub running_ids: Vec<String>,
}

/// Codex's order: tier, then newest first; the id breaks a tie so a plan is deterministic.
fn ranked<'a>(items: impl Iterator<Item = &'a Item>) -> Vec<&'a Item> {
    let mut list: Vec<&Item> = items.collect();
    list.sort_by(|a, b| {
        a.attention
            .tier()
            .cmp(&b.attention.tier())
            .then(b.recency_ms.cmp(&a.recency_ms))
            .then_with(|| a.id.cmp(&b.id))
    });
    list
}

fn newest_first(list: &mut [&Item]) {
    list.sort_by(|a, b| b.recency_ms.cmp(&a.recency_ms).then_with(|| a.id.cmp(&b.id)));
}

/// Local midnight of `now_ms`'s day, in epoch milliseconds.
#[must_use]
pub fn start_of_day(now_ms: i64, tz_offset_minutes: i32) -> i64 {
    let offset = i64::from(tz_offset_minutes) * 60 * 1000;
    (now_ms + offset).div_euclid(DAY_MS) * DAY_MS - offset
}

/// The snapshot for `request`: a fresh one ranked now when the view just opened, else the last one
/// with new chats on top, newly needy chats appended, and gone chats (archived, or read before
/// today) dropped. Its order never re-sorts.
#[must_use]
pub fn snapshot(request: &Request) -> Snapshot {
    let today = start_of_day(request.now_ms, request.tz_offset_minutes);
    let Some(previous) = &request.snapshot else {
        let ids = ranked(request.items.iter().filter(|item| item.attention != Attention::Idle))
            .into_iter()
            .map(|item| item.id.clone())
            .collect();
        return Snapshot { ids, activated_ms: request.now_ms };
    };
    let by_id: HashMap<&str, &Item> =
        request.items.iter().map(|item| (item.id.as_str(), item)).collect();
    let known: HashSet<&str> = previous.ids.iter().map(String::as_str).collect();
    let mut created: Vec<&Item> = request
        .items
        .iter()
        .filter(|item| {
            !known.contains(item.id.as_str())
                && item.created_ms.is_some_and(|at| at > previous.activated_ms)
        })
        .collect();
    newest_first(&mut created);
    let created_ids: HashSet<&str> = created.iter().map(|item| item.id.as_str()).collect();
    let kept = previous.ids.iter().filter(|id| {
        by_id
            .get(id.as_str())
            .is_some_and(|item| item.attention != Attention::Idle || item.recency_ms >= today)
    });
    let appended = ranked(request.items.iter().filter(|item| {
        item.attention != Attention::Idle
            && !known.contains(item.id.as_str())
            && !created_ids.contains(item.id.as_str())
    }));
    let mut seen = HashSet::new();
    let ids = created
        .iter()
        .map(|item| &item.id)
        .chain(kept)
        .chain(appended.iter().map(|item| &item.id))
        .filter(|id| seen.insert(id.as_str()))
        .cloned()
        .collect();
    Snapshot { ids, activated_ms: previous.activated_ms }
}

/// Lays the view out from `request`.
#[must_use]
pub fn plan(request: &Request) -> Plan {
    let snapshot = snapshot(request);
    layout(request, snapshot)
}

/// Clear read chats: the snapshot keeps only the chats that still need the person or work, in
/// their order, then the view lays out again.
#[must_use]
pub fn clear_read(request: &Request) -> Plan {
    let mut snapshot = snapshot(request);
    let by_id: HashMap<&str, &Item> =
        request.items.iter().map(|item| (item.id.as_str(), item)).collect();
    snapshot
        .ids
        .retain(|id| by_id.get(id.as_str()).is_some_and(|item| item.attention != Attention::Idle));
    layout(request, snapshot)
}

fn layout(request: &Request, snapshot: Snapshot) -> Plan {
    let options = request.options;
    let by_id: HashMap<&str, &Item> =
        request.items.iter().map(|item| (item.id.as_str(), item)).collect();
    let mut placed: HashSet<&str> = HashSet::new();
    let mut sections: Vec<(SectionKind, Option<i64>, Vec<&Item>)> = Vec::new();
    let pinned_apart = |item: &Item| options.show_pinned && item.pinned;
    let mut priority: Vec<&Item> = Vec::new();
    if options.show_priority {
        priority = snapshot
            .ids
            .iter()
            .filter_map(|id| by_id.get(id.as_str()).copied())
            .filter(|item| !pinned_apart(item))
            .collect();
        placed.extend(priority.iter().map(|item| item.id.as_str()));
        sections.push((SectionKind::Priority, None, priority.clone()));
    }
    if options.show_pinned {
        let mut pinned: Vec<&Item> = request.items.iter().filter(|item| item.pinned).collect();
        if !pinned.is_empty() {
            newest_first(&mut pinned);
            placed.extend(pinned.iter().map(|item| item.id.as_str()));
            sections.push((SectionKind::Pinned, None, pinned));
        }
    }
    let today = start_of_day(request.now_ms, request.tz_offset_minutes);
    let first_day = today - (RECENT_DAYS - 1) * DAY_MS;
    let mut recent: Vec<&Item> = request
        .items
        .iter()
        .filter(|item| !placed.contains(item.id.as_str()) && item.recency_ms >= first_day)
        .collect();
    newest_first(&mut recent);
    for item in recent {
        // Newest first, so each day's rows come together and the days run Today first. A chat
        // stamped later than now (another machine's clock) counts as today.
        let days_ago = if item.recency_ms >= today {
            0
        } else {
            (today - item.recency_ms + DAY_MS - 1) / DAY_MS
        };
        match sections.last_mut() {
            Some((SectionKind::Day, Some(day), rows)) if *day == days_ago => rows.push(item),
            _ => sections.push((SectionKind::Day, Some(days_ago), vec![item])),
        }
    }
    if sections.is_empty() {
        // A lone Today header carries the options menu.
        sections.push((SectionKind::Day, Some(0), Vec::new()));
    }
    let total: usize = sections.iter().map(|(_, _, rows)| rows.len()).sum();
    let visible = if request.visible == 0 { PAGE } else { request.visible };
    let mut left = visible;
    let mut out = Vec::with_capacity(sections.len());
    for (index, (kind, days_ago, rows)) in sections.into_iter().enumerate() {
        // A section the pages have not reached yet does not show; an empty Priority section
        // (Nothing needs attention) and the lone Today header do.
        if left == 0 && !rows.is_empty() {
            break;
        }
        let ids: Vec<String> = rows.iter().take(left).map(|item| item.id.clone()).collect();
        left -= ids.len();
        out.push(Section { kind, days_ago, ids, actions: index == 0 });
    }
    let shown = out.iter().map(|section| section.ids.len()).sum();
    let archive: Vec<&Item> = priority.iter().copied().filter(|item| item.can_archive).collect();
    Plan {
        can_clear_read: priority.iter().any(|item| item.attention == Attention::Idle),
        mark_read_ids: priority
            .iter()
            .filter(|item| item.can_mark_read && item.attention.needs_attention())
            .map(|item| item.id.clone())
            .collect(),
        running_ids: archive
            .iter()
            .filter(|item| item.attention == Attention::Active)
            .map(|item| item.id.clone())
            .collect(),
        archive_ids: archive.iter().map(|item| item.id.clone()).collect(),
        snapshot,
        sections: out,
        shown,
        total,
        has_more: shown < total,
    }
}
