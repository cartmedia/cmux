//! Chat search scopes and operators at Leo's scale, on `find_first_hit.rs`'s harness (Leo,
//! 2026-11-18: "Like recency is usually what's desired, I assume, when people are searching. And
//! then sometimes, you know, maybe people want to add more search options.").
//!
//! Data: `find_first_hit.rs`'s defaults (one 500 MB Claude Code session, a 20 MB subagent, and
//! more sessions of 5 MB each), laid out as Claude Code does (`projects/<cwd>/<id>.jsonl`,
//! `projects/<cwd>/<id>/subagents/agent-*.jsonl`), so a search lists its sources the way the wire
//! does (`Roots::all` for every chat, `Roots::session` for one).
//!
//! Dates: every line's `timestamp` comes from its byte position in its file, relative to the time
//! the bench starts ("now"), so `since:7d` means what it would on the machine:
//! - the big session runs from 120 days ago to an hour ago, evenly by bytes (its last 7 days are
//!   its newest ~6%);
//! - extra session `k` spans 2 days; one in ten (`k % 10 == 0`) ends 0-5 days ago (the recent
//!   slice), the rest end 8-180 days ago.
//!
//! Words (each turn decides from a hash of its file's seed and its number, not the text's stream):
//! - `nightly`: one of the generator's 64 words, so in almost every doc (past `EXACT_DOCS`);
//! - `deploy`: in the reply of 1 turn in 16 (and `Deploy` opens the prompt of half of those); of
//!   those, 1 in 3 also says `flaky` and 1 in 4 says the phrase `the green build`;
//! - `zephyrine`: rare, in 1 turn in 1500, any age;
//! - `archaeopteryx`: only in turns older than 30 days (1 in 50 of them).
//!
//! Each line carries a `cwd` as Claude Code's do: the big session and one extra in four are in
//! `/u/Projects/cmux`, one in four in `/u/Projects/leo-workspace`, the rest in `/u/Projects/projN`.
//!
//! A search is timed as `_acpmux/transcript_search` runs it: the query text through
//! `syntax::parse` and `Parsed::apply` (the scope's `sinceMs` and session set as the caller's),
//! then the sources listed and `acpmux_transcript_search::search` (catch-up of each source, then
//! the index), warm (every transcript already read). `_acpmux/search_syntax` is timed alone: parse
//! and complete (and the answer's JSON) at every prefix of the query, one per keystroke.
//!
//! Scaling: the same queries at `SCALE_STEPS` extra sessions (50, 200, 800): the extra sessions
//! are written and indexed once, step by step (each step adds to the last), so the disk peak is
//! the last step's (~4.5 GB of transcripts and their index at the defaults).
//!
//! Run by hand, on the fleet:
//! `cargo test -p acpmux-transcript-search --release --test search_operators -- --ignored --nocapture`.
//! `SCALE_MB` (500), `SCALE_SUBAGENT_MB` (20), `SCALE_HOST_FILE_MB` (5), `SCALE_STEPS`
//! (`50,200,800`), `AFTER_ROUNDS` (100; a query stops early once it has run 4 s and 5 rounds),
//! `SYNTAX_ROUNDS` (200). JSON lines go to stdout and, when `FAST_CHECKS_OUT` is set, to
//! `$FAST_CHECKS_OUT/search_operators.jsonl`.

use std::fs::File;
use std::io::{BufWriter, Write as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use acpmux_transcript_search::syntax::{self, Clock};
use acpmux_transcript_search::{Harness, Index, Query, Role, Roots, Source};
use serde_json::{Value, json};

fn env_num(key: &str, default: u64) -> u64 {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

const DAY_MS: i64 = 86_400_000;

// ---- The generator: `find_first_hit.rs`'s turns, dated and with planted words. ----

struct Words(u64);

const VOCAB: &[&str] = &[
    "the",
    "agent",
    "session",
    "transcript",
    "search",
    "index",
    "turn",
    "page",
    "build",
    "merge",
    "test",
    "linux",
    "daemon",
    "peer",
    "query",
    "result",
    "commit",
    "branch",
    "review",
    "fix",
    "render",
    "sidebar",
    "chat",
    "prompt",
    "reply",
    "tool",
    "file",
    "line",
    "check",
    "green",
    "nightly",
    "cache",
    "store",
    "reader",
    "writer",
    "window",
    "scroll",
    "row",
    "column",
    "snippet",
    "context",
    "compaction",
    "summary",
    "and",
    "of",
    "to",
    "in",
    "is",
    "for",
    "with",
    "on",
    "that",
    "this",
    "it",
    "as",
    "we",
    "run",
    "time",
    "ms",
    "bytes",
    "open",
    "close",
    "state",
    "path",
];

impl Words {
    fn next(&mut self) -> &'static str {
        self.0 =
            self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        VOCAB[((self.0 >> 33) % VOCAB.len() as u64) as usize]
    }

    fn text(&mut self, bytes: usize) -> String {
        let mut out = String::with_capacity(bytes + 16);
        let mut n = 0;
        while out.len() < bytes {
            if n > 0 {
                out.push(if n % 14 == 0 { '\n' } else { ' ' });
            }
            out.push_str(self.next());
            n += 1;
        }
        out
    }
}

/// splitmix64: the per-turn dice, apart from the text's stream.
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// RFC 3339 (UTC, ms) of `ms` since the epoch.
fn stamp(ms: i64) -> String {
    let (s, milli) = (ms.div_euclid(1000), ms.rem_euclid(1000));
    let (d, r) = (s.div_euclid(86_400), s.rem_euclid(86_400));
    let z = d + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{milli:03}Z",
        r / 3600,
        r % 3600 / 60,
        r % 60
    )
}

/// What one turn says besides the generator's words.
#[derive(Default)]
struct Planted {
    deploy: bool,
    deploy_prompt: bool,
    flaky: bool,
    phrase: bool,
    rare: bool,
    old: bool,
}

impl Planted {
    fn roll(seed: u64, i: u64, age_ms: i64) -> Self {
        let h = mix(seed.wrapping_mul(1_000_003) ^ i);
        let deploy = h % 16 == 0;
        Self {
            deploy,
            deploy_prompt: deploy && (h >> 8) % 2 == 0,
            flaky: deploy && (h >> 12) % 3 == 0,
            phrase: deploy && (h >> 16) % 4 == 0,
            rare: (h >> 20) % 1500 == 0,
            old: age_ms > 30 * DAY_MS && (h >> 32) % 50 == 0,
        }
    }
}

#[derive(Default, Clone, Copy)]
struct Counts {
    turns: u64,
    deploy: u64,
    flaky: u64,
    phrase: u64,
    rare: u64,
    old: u64,
    /// Turns written in the last 7 days.
    recent: u64,
}

struct Span {
    /// The first and last line's time (ms since the epoch).
    from_ms: i64,
    to_ms: i64,
}

fn turn(
    w: &mut Words,
    i: u64,
    at_ms: i64,
    cwd: &str,
    plant: &Planted,
    filler: usize,
    out: &mut Vec<String>,
) {
    let t = stamp(at_ms);
    let mut prompt = w.text(500);
    if plant.deploy_prompt {
        prompt.insert_str(0, "Deploy it once ");
    }
    out.push(json!({"type": "user", "timestamp": t, "cwd": cwd, "message": {"role": "user", "content": prompt}}).to_string());
    for k in 0..3 {
        out.push(json!({"type": "assistant", "timestamp": t, "cwd": cwd, "message": {"content": [
            {"type": "tool_use", "id": format!("t{i}-{k}"), "name": "Bash", "input": {"command": w.text(150)}}]}})
        .to_string());
        out.push(
            json!({"type": "user", "timestamp": t, "cwd": cwd, "message": {"content": [
            {"type": "tool_result", "tool_use_id": format!("t{i}-{k}"), "content": w.text(1500)}]}})
            .to_string(),
        );
    }
    let mut reply = w.text(1000);
    if plant.deploy {
        reply.push_str("\nWe deploy after the check");
    }
    if plant.flaky {
        reply.push_str(", the flaky one");
    }
    if plant.phrase {
        reply.push_str("; deploy the green build");
    }
    if plant.rare {
        reply.push_str("\nthe zephyrine path");
    }
    if plant.old {
        reply.push_str("\narchaeopteryx cache");
    }
    out.push(json!({"type": "assistant", "timestamp": t, "cwd": cwd, "message": {"content": [{"type": "text", "text": reply}]}}).to_string());
    out.push(
        json!({"type": "api-request-blob", "timestamp": t, "blob": w.text(filler)}).to_string(),
    );
}

/// Writes about `mb` MB of turns to `path`, dated evenly by bytes over `span`, as of `now_ms`.
fn write_transcript(
    path: &Path,
    mb: u64,
    seed: u64,
    span: &Span,
    cwd: &str,
    now_ms: i64,
) -> Counts {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    let mut w = Words(seed);
    let mut f = BufWriter::new(File::create(path).unwrap());
    let total = (mb << 20).max(1);
    let (mut bytes, mut i, mut batch, mut counts) = (0u64, 0u64, Vec::new(), Counts::default());
    while bytes < total {
        let at_ms = span.from_ms
            + ((span.to_ms - span.from_ms) as f64 * (bytes as f64 / total as f64)) as i64;
        let plant = Planted::roll(seed, i, now_ms - at_ms);
        turn(&mut w, i, at_ms, cwd, &plant, 48 * 1024, &mut batch);
        counts.turns += 1;
        counts.deploy += u64::from(plant.deploy);
        counts.flaky += u64::from(plant.flaky);
        counts.phrase += u64::from(plant.phrase);
        counts.rare += u64::from(plant.rare);
        counts.old += u64::from(plant.old);
        counts.recent += u64::from(now_ms - at_ms <= 7 * DAY_MS);
        for l in batch.drain(..) {
            bytes += l.len() as u64 + 1;
            writeln!(f, "{l}").unwrap();
        }
        i += 1;
    }
    f.flush().unwrap();
    counts
}

fn add(a: &mut Counts, b: Counts) {
    a.turns += b.turns;
    a.deploy += b.deploy;
    a.flaky += b.flaky;
    a.phrase += b.phrase;
    a.rare += b.rare;
    a.old += b.old;
    a.recent += b.recent;
}

fn project_dir(cwd: &str) -> String {
    cwd.replace('/', "-")
}

fn extra_cwd(k: u64) -> String {
    match k % 4 {
        0 => "/u/Projects/cmux".into(),
        1 => "/u/Projects/leo-workspace".into(),
        _ => format!("/u/Projects/proj{}", k % 9),
    }
}

fn extra_span(k: u64, now_ms: i64) -> Span {
    let end_age = if k % 10 == 0 {
        (k / 10 % 6) as i64 * DAY_MS + 3_600_000
    } else {
        (8 + (k * 37) % 172) as i64 * DAY_MS
    };
    Span { from_ms: now_ms - end_age - 2 * DAY_MS, to_ms: now_ms - end_age }
}

// ---- Stats and output. ----

fn pct(v: &[Duration], p: f64) -> f64 {
    let mut v = v.to_vec();
    v.sort();
    let i = ((v.len() as f64 - 1.0) * p).round() as usize;
    v[i].as_secs_f64() * 1000.0
}

/// Runs `f` up to `rounds` times, stopping early once 4 s and 5 rounds are spent.
fn timed<T>(rounds: usize, mut f: impl FnMut() -> T) -> (f64, f64, usize, T) {
    let mut v = Vec::with_capacity(rounds);
    let mut last = None;
    let started = Instant::now();
    for _ in 0..rounds.max(1) {
        let t = Instant::now();
        last = Some(f());
        v.push(t.elapsed());
        if v.len() >= 5 && started.elapsed() > Duration::from_secs(4) {
            break;
        }
    }
    (pct(&v, 0.5), pct(&v, 0.95), v.len(), last.unwrap())
}

struct Out(Option<File>);

impl Out {
    fn line(&mut self, v: Value) {
        let s = v.to_string();
        println!("JSON {s}");
        if let Some(f) = self.0.as_mut() {
            writeln!(f, "{s}").unwrap();
        }
    }
}

fn host() -> Value {
    let cpu = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|c| {
            c.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim().to_owned())
        })
        .unwrap_or_default();
    let uname = std::process::Command::new("uname")
        .arg("-a")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default();
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0);
    json!({"cpu": cpu, "threads": threads, "uname": uname,
        "runner": std::env::var("RUNNER_NAME").unwrap_or_default()})
}

// ---- The bench. ----

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScopeKind {
    All,
    Recent,
    This,
}

impl ScopeKind {
    fn name(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Recent => "Recent",
            Self::This => "This chat",
        }
    }
}

const SCOPES: [ScopeKind; 3] = [ScopeKind::All, ScopeKind::Recent, ScopeKind::This];

/// (label, query text, case sensitive)
const QUERIES: &[(&str, &str, bool)] = &[
    ("ubiquitous word", "nightly", false),
    ("common word", "deploy", false),
    ("rare word", "zephyrine", false),
    ("only in old turns", "archaeopteryx", false),
    ("1 op: -word", "deploy -flaky", false),
    ("2 ops", "deploy -flaky from:agent", false),
    ("4 ops", "deploy \"the green build\" -flaky since:7d agent:claude", false),
    ("folder + from", "deploy folder:cmux from:me", false),
    ("ubiquitous, 2 ops", "nightly -flaky from:agent", false),
    ("caseSensitive Deploy", "Deploy", true),
    ("caseSensitive deploy", "deploy", true),
    ("old only, since:7d", "archaeopteryx since:7d", false),
];

struct Config {
    mb: u64,
    sub_mb: u64,
    file_mb: u64,
    steps: Vec<u64>,
    rounds: usize,
    syntax_rounds: usize,
    out: Out,
}

fn run(mut cfg: Config) -> Vec<Value> {
    let host = host();
    println!("host: {host}");
    cfg.out.line(json!({"bench": "search_operators", "kind": "host", "host": host}));
    let clock = Clock::now(0);
    let now_ms = clock.now_ms;
    let recent_ms = now_ms - 7 * DAY_MS;

    let dir = tempfile::tempdir().unwrap();
    let projects = dir.path().join("projects");
    let mut roots = Roots::default();
    std::fs::create_dir_all(&projects).unwrap();
    roots.add_claude(projects.clone());

    // The big session (and its subagent), 120 days long, ending an hour ago.
    let t = Instant::now();
    let big_cwd = "/u/Projects/cmux";
    let big_dir = projects.join(project_dir(big_cwd));
    let big_span = Span { from_ms: now_ms - 120 * DAY_MS, to_ms: now_ms - 3_600_000 };
    let mut counts_big =
        write_transcript(&big_dir.join("big.jsonl"), cfg.mb, 7, &big_span, big_cwd, now_ms);
    let sub_span = Span { from_ms: now_ms - 40 * DAY_MS, to_ms: now_ms - 39 * DAY_MS };
    let sub_counts = write_transcript(
        &big_dir.join("big/subagents/agent-a1.jsonl"),
        cfg.sub_mb,
        11,
        &sub_span,
        big_cwd,
        now_ms,
    );
    add(&mut counts_big, sub_counts);
    println!(
        "big session: {} turns ({} in the last 7 days), written in {:.0} s",
        counts_big.turns,
        counts_big.recent,
        t.elapsed().as_secs_f64()
    );

    let db = dir.path().join("index.sqlite");
    let index = Index::open(&db).unwrap();
    let t = Instant::now();
    for s in roots.session("big") {
        index.catch_up(&s).unwrap();
    }
    println!("big session indexed in {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);

    let mut rows = Vec::new();
    let mut written = 0u64;
    let mut counts_all = counts_big;
    for &step in &cfg.steps {
        let t = Instant::now();
        while written < step {
            let k = written;
            let cwd = extra_cwd(k);
            let path = projects.join(project_dir(&cwd)).join(format!("s{k}.jsonl"));
            let c = write_transcript(
                &path,
                cfg.file_mb,
                1000 + k,
                &extra_span(k, now_ms),
                &cwd,
                now_ms,
            );
            add(&mut counts_all, c);
            written += 1;
        }
        let write_s = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let all = roots.all();
        for s in &all {
            index.catch_up(s).unwrap();
        }
        let index_ms = t.elapsed().as_secs_f64() * 1000.0;
        let index_mb = std::fs::metadata(&db).unwrap().len() >> 20;
        let (files, docs) = index.stats().unwrap();
        let corpus_mb = cfg.mb + cfg.sub_mb + step * cfg.file_mb;
        println!(
            "step: {step} extra sessions ({corpus_mb} MB, {} files, {docs} docs; {} turns, {} recent); written {write_s:.0} s, indexed {index_ms:.0} ms; index {index_mb} MB",
            all.len(),
            counts_all.turns,
            counts_all.recent
        );
        cfg.out.line(json!({"bench": "search_operators", "kind": "step", "extra_sessions": step,
            "corpus_mb": corpus_mb, "files": files, "docs": docs, "sources": all.len(),
            "turns": counts_all.turns, "recent_turns": counts_all.recent,
            "planted": {"deploy": counts_all.deploy, "flaky": counts_all.flaky,
                "phrase": counts_all.phrase, "rare": counts_all.rare, "old": counts_all.old},
            "big_planted": {"deploy": counts_big.deploy, "rare": counts_big.rare,
                "old": counts_big.old, "recent_turns": counts_big.recent},
            "index_ms": index_ms, "index_mb": index_mb}));

        for &(label, text, case_sensitive) in QUERIES {
            for scope in SCOPES {
                let (big, recent) = (scope == ScopeKind::This, scope == ScopeKind::Recent);
                let query = || {
                    let parsed = syntax::parse(text, clock);
                    let mut q = Query::new("");
                    if recent {
                        q.since_ms = Some(recent_ms);
                    }
                    parsed.apply(&mut q);
                    q.case_sensitive = case_sensitive;
                    if big {
                        q.sessions = Some(vec!["big".into()]);
                    }
                    q
                };
                let sources =
                    || -> Vec<Source> { if big { roots.session("big") } else { roots.all() } };
                // The wire: parse, apply, list the sources, catch up, ask the index.
                let (p50, p95, n, (hits, done)) = timed(cfg.rounds, || {
                    let q = query();
                    acpmux_transcript_search::search(&index, &sources(), &q).unwrap()
                });
                let q = query();
                let (lp50, lp95, _, srcs) = timed(cfg.rounds, sources);
                let (cp50, cp95, _, ()) = timed(cfg.rounds, || {
                    for s in &srcs {
                        index.catch_up(s).unwrap();
                    }
                });
                let (ip50, ip95, _, page) = timed(cfg.rounds, || index.page(&q).unwrap());
                assert_eq!(page.hits.len(), hits.len(), "{label} {}: same page", scope.name());
                check(label, text, &q, &hits);
                let row = json!({"bench": "search_operators", "kind": "query",
                    "extra_sessions": step, "corpus_mb": corpus_mb, "case": label,
                    "query": text, "case_sensitive": case_sensitive, "scope": scope.name(),
                    "hits": hits.len(), "total": done.total, "sources": srcs.len(),
                    "wire": {"p50_ms": p50, "p95_ms": p95, "rounds": n},
                    "list_sources": {"p50_ms": lp50, "p95_ms": lp95},
                    "catch_up": {"p50_ms": cp50, "p95_ms": cp95},
                    "index_page": {"p50_ms": ip50, "p95_ms": ip95}});
                cfg.out.line(row.clone());
                rows.push(row);
            }
        }
    }

    // `_acpmux/search_syntax`: parse and complete at every prefix (one per keystroke).
    let mut syntax_rows = Vec::new();
    for &(label, text, _) in QUERIES {
        let mut v = Vec::new();
        let prefixes: Vec<&str> =
            text.char_indices().map(|(i, c)| &text[..i + c.len_utf8()]).collect();
        for _ in 0..cfg.syntax_rounds.max(1) {
            for p in &prefixes {
                let cursor = p.encode_utf16().count();
                let t = Instant::now();
                let parsed = syntax::parse(p, clock);
                let completions = syntax::complete(p, cursor);
                let answer = json!({"parsed": parsed.to_json(), "completions": completions.len()});
                std::hint::black_box(answer);
                v.push(t.elapsed());
            }
        }
        let row = json!({"bench": "search_operators", "kind": "search_syntax", "case": label,
            "query": text, "keystrokes": prefixes.len(), "samples": v.len(),
            "p50_us": pct(&v, 0.5) * 1000.0, "p95_us": pct(&v, 0.95) * 1000.0,
            "max_us": pct(&v, 1.0) * 1000.0});
        cfg.out.line(row.clone());
        syntax_rows.push(row);
    }

    print_tables(&cfg.steps, &rows, &syntax_rows);
    rows
}

/// What every hit of `q` must be.
fn check(label: &str, text: &str, q: &Query, hits: &[acpmux_transcript_search::Hit]) {
    for h in hits {
        if let Some(since) = q.since_ms {
            assert!(h.at_ms.is_some_and(|at| at >= since), "{label}: a hit before since");
        }
        if let Some(role) = q.role {
            assert_eq!(h.role, role, "{label}: role");
        }
        if let Some(sessions) = &q.sessions {
            assert!(sessions.contains(&h.session), "{label}: session");
        }
        assert_eq!(h.harness, Harness::Claude);
        if q.case_sensitive {
            assert!(h.snippet.contains(&q.text), "{label}: case-sensitive snippet {:?}", h.snippet);
        }
        if q.folder.is_some() {
            assert!(h.path.to_string_lossy().contains("-cmux"), "{label}: folder");
        }
    }
    if text.starts_with("archaeopteryx since:") {
        assert!(hits.is_empty(), "{label}: old-only words have no recent hit");
    }
    if q.role == Some(Role::User) {
        assert!(hits.iter().all(|h| h.role == Role::User));
    }
}

fn print_tables(steps: &[u64], rows: &[Value], syntax_rows: &[Value]) {
    let f = |v: &Value, k: &str| v[k]["p50_ms"].as_f64().unwrap_or(f64::NAN);
    let g = |v: &Value, k: &str| v[k]["p95_ms"].as_f64().unwrap_or(f64::NAN);
    for &step in steps {
        println!();
        println!("warm search through the wire's path, {step} extra sessions, ms p50/p95 (hits)");
        println!(
            "{:<24} {:<56} | {:>22} | {:>22} | {:>22}",
            "case", "query", "All", "Recent (7d)", "This chat"
        );
        for &(label, text, _) in QUERIES {
            let cell = |scope: &str| {
                rows.iter()
                    .find(|r| {
                        r["extra_sessions"] == json!(step)
                            && r["case"] == json!(label)
                            && r["scope"] == json!(scope)
                    })
                    .map_or("-".into(), |r| {
                        format!("{:.1}/{:.1} ({})", f(r, "wire"), g(r, "wire"), r["hits"])
                    })
            };
            println!(
                "{label:<24} {text:<56} | {:>22} | {:>22} | {:>22}",
                cell("All"),
                cell("Recent"),
                cell("This chat")
            );
        }
    }
    println!();
    println!("where the time goes (p50 ms): list sources / catch up / index page");
    for r in rows {
        println!(
            "{:>4} {:<24} {:<10} {:>8.1} {:>8.1} {:>8.1}",
            r["extra_sessions"],
            r["case"].as_str().unwrap_or_default(),
            r["scope"].as_str().unwrap_or_default(),
            f(r, "list_sources"),
            f(r, "catch_up"),
            f(r, "index_page")
        );
    }
    println!();
    println!("_acpmux/search_syntax per keystroke (parse + complete + JSON), us p50/p95/max");
    for r in syntax_rows {
        println!(
            "{:<24} {:>3} keys {:>8.1} {:>8.1} {:>8.1}",
            r["case"].as_str().unwrap_or_default(),
            r["keystrokes"],
            r["p50_us"].as_f64().unwrap_or(f64::NAN),
            r["p95_us"].as_f64().unwrap_or(f64::NAN),
            r["max_us"].as_f64().unwrap_or(f64::NAN)
        );
    }
}

#[test]
#[ignore = "scale bench: minutes and gigabytes; run by hand on the fleet"]
fn search_scopes_and_operators_at_scale() {
    let out = Out(std::env::var_os("FAST_CHECKS_OUT").map(|d| {
        std::fs::create_dir_all(&d).unwrap();
        File::create(PathBuf::from(d).join("search_operators.jsonl")).unwrap()
    }));
    let steps = std::env::var("SCALE_STEPS")
        .ok()
        .map(|s| s.split(',').filter_map(|n| n.trim().parse().ok()).collect::<Vec<u64>>())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| vec![50, 200, 800]);
    run(Config {
        mb: env_num("SCALE_MB", 500),
        sub_mb: env_num("SCALE_SUBAGENT_MB", 20),
        file_mb: env_num("SCALE_HOST_FILE_MB", 5),
        steps,
        rounds: env_num("AFTER_ROUNDS", 100) as usize,
        syntax_rounds: env_num("SYNTAX_ROUNDS", 200) as usize,
        out,
    });
}

/// The same bench at a toy size, so a plain `cargo test` keeps it working.
#[test]
fn search_scopes_and_operators_smoke() {
    let rows = run(Config {
        mb: 1,
        sub_mb: 1,
        file_mb: 1,
        steps: vec![2, 4],
        rounds: 2,
        syntax_rounds: 1,
        out: Out(None),
    });
    assert_eq!(rows.len(), 2 * QUERIES.len() * SCOPES.len());
}
