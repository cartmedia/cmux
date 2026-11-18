//! The search box's query language, one parser for every surface that searches chats (Find in
//! Chat, the palette's chats page, the `transcript_search` tool, `acpmux search`). Leo,
//! 2026-11-18: "I know that VS Code has like a decent kind of searching thing with arguments and
//! whatnot, likewise with Obsidian."
//!
//! - words separated by spaces: every one must be in the message (AND), in any order
//! - `"exact phrase"`: one term with its spaces; an unclosed quote runs to the end
//! - `-word`, `-"a phrase"`: the message must not hold it. A lone `-`, `--x`, or a `-` inside a
//!   word is literal
//! - `from:me` | `from:agent` (also `from:user`, `from:assistant`, `from:tool`)
//! - `agent:claude` | `agent:codex`: the harness
//! - `in:this` | `in:all`: this chat or every chat
//! - `folder:NAME`, `repo:NAME`: the chat's working directory holds NAME (any case)
//! - `since:` / `before:` `7d`, `24h`, `2w`, `30m`, `today`, `yesterday`, `YYYY-MM-DD` (a local
//!   day): `since:` from that start on, `before:` up to it (not in)
//!
//! A known key with an empty or bad value is searched as written and gives a problem; an unknown
//! `key:value` (`error: foo`, a URL) is searched as written. Keys are any case; quoting a token
//! (`"from:me"`) makes it literal. Case sensitivity is a toggle of the caller's
//! (`Query::case_sensitive`), not an operator. Every span is in UTF-16 code units of the query (what
//! a text field's selection counts).

use serde_json::{Value, json};

use crate::index::Query;
use crate::parse::{Harness, Role};

const MINUTE_MS: i64 = 60_000;
const HOUR_MS: i64 = 60 * MINUTE_MS;
const DAY_MS: i64 = 24 * HOUR_MS;

/// What "now" and "today" are for `since:` and `before:`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clock {
    /// Ms since the epoch.
    pub now_ms: i64,
    /// Minutes the local time is ahead of UTC (local = UTC + offset; UTC-8 is -480).
    pub tz_offset_min: i32,
}

impl Clock {
    /// The current time, in a zone `tz_offset_min` minutes ahead of UTC.
    pub fn now(tz_offset_min: i32) -> Self {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        Self { now_ms, tz_offset_min }
    }

    fn offset_ms(self) -> i64 {
        i64::from(self.tz_offset_min) * MINUTE_MS
    }

    /// The start (ms since the epoch) of the local day `days` days after today's.
    fn day_start(self, days: i64) -> i64 {
        let local = self.now_ms + self.offset_ms();
        (local.div_euclid(DAY_MS) + days) * DAY_MS - self.offset_ms()
    }
}

/// Which chats a search covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    This,
    All,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::This => "this",
            Self::All => "all",
        }
    }
}

/// Text a message must hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Term {
    pub text: String,
    /// It was quoted (`"a phrase"`).
    pub phrase: bool,
}

/// What kind of part of the query a [`Chip`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChipKind {
    Phrase,
    Exclude,
    From,
    Agent,
    In,
    Folder,
    Since,
    Before,
}

impl ChipKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Phrase => "phrase",
            Self::Exclude => "exclude",
            Self::From => "from",
            Self::Agent => "agent",
            Self::In => "in",
            Self::Folder => "folder",
            Self::Since => "since",
            Self::Before => "before",
        }
    }
}

/// One part of the query that is more than a word, as a search box shows it: removing it removes
/// `query[start..end)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chip {
    pub kind: ChipKind,
    /// What it holds, normalized (`me`, `claude`, `7d`, the phrase's text).
    pub value: String,
    /// How a person reads it (`From me`, `Since 7d`).
    pub label: String,
    /// UTF-16 offsets into the query.
    pub start: usize,
    pub end: usize,
}

/// One suggestion for the token at the cursor: `insert` replaces `query[start..end)` (UTF-16).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    pub insert: String,
    pub label: String,
    pub detail: String,
    pub start: usize,
    pub end: usize,
}

/// A query, read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    /// What each message must hold (in the order written).
    pub terms: Vec<Term>,
    /// What no message may hold.
    pub exclude: Vec<String>,
    pub role: Option<Role>,
    pub harness: Option<Harness>,
    pub scope: Option<Scope>,
    pub folder: Option<String>,
    /// From this time on (ms since the epoch, in).
    pub since_ms: Option<i64>,
    /// Before this time (ms since the epoch, not in).
    pub before_ms: Option<i64>,
    pub chips: Vec<Chip>,
    /// What was not understood, for a person (`since:soon is not a time ...`); the token is
    /// searched as written.
    pub problems: Vec<String>,
}

impl Parsed {
    /// Whether no term is left to find (only operators, or nothing).
    pub fn is_empty_text(&self) -> bool {
        self.terms.iter().all(|t| t.text.trim().is_empty())
    }

    /// Puts what it asks into `q`: its terms become `q.text` and `q.also` (replacing them), its
    /// filters `q`'s (a filter it has not keeps `q`'s); `since` and `before` keep the narrower of
    /// `q`'s and its own. The scope is the caller's to apply (`q.sessions`).
    pub fn apply(&self, q: &mut Query) {
        let mut terms = self.terms.iter().map(|t| t.text.clone()).filter(|t| !t.trim().is_empty());
        q.text = terms.next().unwrap_or_default();
        q.also = terms.collect();
        q.exclude.extend(self.exclude.iter().cloned());
        if self.role.is_some() {
            q.role = self.role;
        }
        if self.harness.is_some() {
            q.harness = self.harness;
        }
        if self.folder.is_some() {
            q.folder.clone_from(&self.folder);
        }
        q.since_ms = later(q.since_ms, self.since_ms);
        q.before_ms = earlier(q.before_ms, self.before_ms);
    }

    /// `{terms: [{text, phrase}], exclude, role?, agent?, in?, folder?, sinceMs?, beforeMs?,
    /// chips: [{kind, value, label, start, end}], problems}`.
    pub fn to_json(&self) -> Value {
        let mut v = json!({
            "terms": self.terms.iter().map(|t| json!({"text": t.text, "phrase": t.phrase})).collect::<Vec<_>>(),
            "exclude": self.exclude,
            "chips": self.chips.iter().map(|c| json!({
                "kind": c.kind.as_str(), "value": c.value, "label": c.label,
                "start": c.start, "end": c.end,
            })).collect::<Vec<_>>(),
            "problems": self.problems,
        });
        if let Some(role) = self.role {
            v["role"] = json!(role.as_str());
        }
        if let Some(harness) = self.harness {
            v["agent"] = json!(harness.as_str());
        }
        if let Some(scope) = self.scope {
            v["in"] = json!(scope.as_str());
        }
        if let Some(folder) = &self.folder {
            v["folder"] = json!(folder);
        }
        if let Some(since) = self.since_ms {
            v["sinceMs"] = json!(since);
        }
        if let Some(before) = self.before_ms {
            v["beforeMs"] = json!(before);
        }
        v
    }
}

/// The later of two starts (None: no start).
pub fn later(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}

/// The earlier of two ends (None: no end).
pub fn earlier(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// The operator keys, as written in a query (lowercase).
const KEYS: [(&str, &str); 7] = [
    ("from", "Who said it: me or agent"),
    ("agent", "Which agent: claude or codex"),
    ("in", "This chat or all chats"),
    ("folder", "The chat's folder holds this"),
    ("repo", "The chat's folder holds this"),
    ("since", "From then on: 7d, 24h, today, 2026-11-01"),
    ("before", "Before then: 7d, today, 2026-11-01"),
];

/// The values `complete` offers after each key: (value, label, detail).
fn values(key: &str) -> &'static [(&'static str, &'static str, &'static str)] {
    match key {
        "from" => &[
            ("me", "from:me", "My prompts"),
            ("agent", "from:agent", "The agent's replies"),
            ("tool", "from:tool", "Tool calls and output"),
        ],
        "agent" => &[("claude", "agent:claude", "Claude Code"), ("codex", "agent:codex", "Codex")],
        "in" => &[("this", "in:this", "This chat"), ("all", "in:all", "Every chat")],
        "since" => &[
            ("today", "since:today", "Since midnight"),
            ("yesterday", "since:yesterday", "Since yesterday's start"),
            ("24h", "since:24h", "The last 24 hours"),
            ("7d", "since:7d", "The last 7 days"),
            ("30d", "since:30d", "The last 30 days"),
        ],
        "before" => &[
            ("today", "before:today", "Before today"),
            ("yesterday", "before:yesterday", "Before yesterday"),
            ("7d", "before:7d", "More than 7 days ago"),
            ("30d", "before:30d", "More than 30 days ago"),
        ],
        _ => &[],
    }
}

/// One whitespace-separated token (a quote holds its spaces), with byte and UTF-16 spans.
struct Token<'a> {
    raw: &'a str,
    start16: usize,
    end16: usize,
}

fn tokens(query: &str) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut pos16 = 0usize;
    let mut start: Option<(usize, usize)> = None;
    let mut quoted = false;
    for (i, c) in query.char_indices() {
        if c.is_whitespace() && !quoted {
            if let Some((b, s16)) = start.take() {
                out.push(Token { raw: &query[b..i], start16: s16, end16: pos16 });
            }
        } else {
            if start.is_none() {
                start = Some((i, pos16));
            }
            if c == '"' {
                quoted = !quoted;
            }
        }
        pos16 += c.len_utf16();
    }
    if let Some((b, s16)) = start {
        out.push(Token { raw: &query[b..], start16: s16, end16: pos16 });
    }
    out
}

/// A token's text with its quotes taken out.
fn unquote(raw: &str) -> String {
    raw.chars().filter(|c| *c != '"').collect()
}

/// `key:value` with a known key: the key (lowercase) and the value (unquoted).
fn operator(raw: &str) -> Option<(&'static str, String)> {
    let (key, value) = raw.split_once(':')?;
    let key = key.to_ascii_lowercase();
    let known = KEYS.iter().find(|(k, _)| *k == key)?.0;
    Some((known, unquote(value)))
}

/// Reads `query` (see the module doc).
pub fn parse(query: &str, clock: Clock) -> Parsed {
    let mut p = Parsed::default();
    for t in tokens(query) {
        let raw = t.raw;
        let chip = |kind: ChipKind, value: &str, label: String| Chip {
            kind,
            value: value.to_owned(),
            label,
            start: t.start16,
            end: t.end16,
        };
        if raw.starts_with('"') {
            let text = unquote(raw);
            if text.trim().is_empty() {
                continue;
            }
            p.chips.push(chip(ChipKind::Phrase, &text, format!("\"{text}\"")));
            p.terms.push(Term { text, phrase: true });
            continue;
        }
        if let Some(rest) = raw.strip_prefix('-')
            && !rest.is_empty()
            && !rest.starts_with('-')
        {
            let text = unquote(rest);
            if !text.trim().is_empty() {
                p.chips.push(chip(ChipKind::Exclude, &text, format!("Not {text}")));
                p.exclude.push(text);
                continue;
            }
        }
        if let Some((key, value)) = operator(raw) {
            match read_operator(key, &value, clock, &mut p) {
                Ok((kind, value, label)) => {
                    p.chips.push(chip(kind, &value, label));
                    continue;
                }
                Err(problem) => p.problems.push(problem),
            }
        }
        let text = unquote(raw);
        if !text.is_empty() {
            let phrase = raw.contains('"') && text.contains(char::is_whitespace);
            p.terms.push(Term { text, phrase });
        }
    }
    p
}

/// Applies one known operator to `p`: its chip's kind, value and label, or what is wrong.
fn read_operator(
    key: &str,
    value: &str,
    clock: Clock,
    p: &mut Parsed,
) -> Result<(ChipKind, String, String), String> {
    let lower = value.trim().to_lowercase();
    if lower.is_empty() {
        return Err(format!("{key}: needs a value ({})", hint(key)));
    }
    match key {
        "from" => {
            let (role, value, label) = match lower.as_str() {
                "me" | "user" => (Role::User, "me", "From me"),
                "agent" | "assistant" => (Role::Assistant, "agent", "From agent"),
                "tool" => (Role::Tool, "tool", "From tools"),
                _ => return Err(format!("from:{value} is not me, agent or tool")),
            };
            p.role = Some(role);
            Ok((ChipKind::From, value.into(), label.into()))
        }
        "agent" => {
            let harness = match lower.as_str() {
                "claude" => Harness::Claude,
                "codex" => Harness::Codex,
                _ => return Err(format!("agent:{value} is not claude or codex")),
            };
            p.harness = Some(harness);
            let label = if harness == Harness::Claude { "Claude" } else { "Codex" };
            Ok((ChipKind::Agent, harness.as_str().into(), label.into()))
        }
        "in" => {
            let (scope, label) = match lower.as_str() {
                "this" => (Scope::This, "This chat"),
                "all" => (Scope::All, "All chats"),
                _ => return Err(format!("in:{value} is not this or all")),
            };
            p.scope = Some(scope);
            Ok((ChipKind::In, scope.as_str().into(), label.into()))
        }
        "folder" | "repo" => {
            let name = value.trim().to_owned();
            p.folder = Some(name.clone());
            let label = format!("Folder {name}");
            Ok((ChipKind::Folder, name, label))
        }
        "since" | "before" => {
            let Some(at) = time_value(&lower, clock) else {
                return Err(format!("{key}:{value} is not a time ({})", hint(key)));
            };
            if key == "since" {
                p.since_ms = later(p.since_ms, Some(at));
                Ok((ChipKind::Since, lower.clone(), format!("Since {lower}")))
            } else {
                p.before_ms = earlier(p.before_ms, Some(at));
                Ok((ChipKind::Before, lower.clone(), format!("Before {lower}")))
            }
        }
        _ => Err(format!("{key}: is not an operator")),
    }
}

fn hint(key: &str) -> &'static str {
    match key {
        "from" => "me, agent or tool",
        "agent" => "claude or codex",
        "in" => "this or all",
        "folder" | "repo" => "a folder name",
        _ => "7d, 24h, 2w, 30m, today, yesterday or YYYY-MM-DD",
    }
}

/// The start of `value` (lowercase): `7d`, `24h`, `2w`, `30m` ago, `today`'s or `yesterday`'s
/// local midnight, or a `YYYY-MM-DD` local day's.
fn time_value(value: &str, clock: Clock) -> Option<i64> {
    match value {
        "today" => return Some(clock.day_start(0)),
        "yesterday" => return Some(clock.day_start(-1)),
        _ => {}
    }
    if let Some(day) = civil_day(value) {
        return Some(day * DAY_MS - clock.offset_ms());
    }
    let unit = value.chars().last()?;
    let count: i64 = value.get(..value.len() - unit.len_utf8())?.parse().ok()?;
    let ms = match unit {
        'm' => MINUTE_MS,
        'h' => HOUR_MS,
        'd' => DAY_MS,
        'w' => 7 * DAY_MS,
        _ => return None,
    };
    if count < 0 {
        return None;
    }
    Some(clock.now_ms.saturating_sub(count.saturating_mul(ms)))
}

/// `YYYY-MM-DD` as days since 1970-01-01 (a real date only).
fn civil_day(value: &str) -> Option<i64> {
    let b = value.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> {
        let s = value.get(r)?;
        if !s.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        s.parse().ok()
    };
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days_in = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if d < 1 || d > days_in {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// Suggestions for the token the cursor (UTF-16 offset into `query`) is in or just after: the
/// keys its start begins (`fr` -> `from:`), or, after a known key's colon, that key's values
/// (`since:` -> `today`, `7d`, ...). None in a quote, after a `-`, for an empty token, or for
/// `folder:` (its values are the caller's to know).
pub fn complete(query: &str, cursor_utf16: usize) -> Vec<Completion> {
    let Some(token) =
        tokens(query).into_iter().find(|t| t.start16 <= cursor_utf16 && cursor_utf16 <= t.end16)
    else {
        return Vec::new();
    };
    // The token's text up to the cursor.
    let mut typed = String::new();
    let mut at16 = token.start16;
    for c in token.raw.chars() {
        if at16 >= cursor_utf16 {
            break;
        }
        typed.push(c);
        at16 += c.len_utf16();
    }
    if typed.is_empty() || typed.starts_with(['"', '-']) {
        return Vec::new();
    }
    let span = |insert: String, label: &str, detail: &str| Completion {
        insert,
        label: label.to_owned(),
        detail: detail.to_owned(),
        start: token.start16,
        end: token.end16,
    };
    match typed.split_once(':') {
        None => {
            let lower = typed.to_ascii_lowercase();
            KEYS.iter()
                .filter(|(k, _)| k.starts_with(&lower))
                .map(|(k, detail)| span(format!("{k}:"), &format!("{k}:"), detail))
                .collect()
        }
        Some((key, value)) => {
            let key = key.to_ascii_lowercase();
            let value = value.to_ascii_lowercase();
            values(&key)
                .iter()
                .filter(|(v, _, _)| v.starts_with(&value) && *v != value)
                .map(|(v, label, detail)| span(format!("{key}:{v} "), label, detail))
                .collect()
        }
    }
}

#[cfg(test)]
#[path = "syntax_tests.rs"]
mod tests;
