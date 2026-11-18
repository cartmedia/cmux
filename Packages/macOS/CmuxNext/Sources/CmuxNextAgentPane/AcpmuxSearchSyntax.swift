public import Foundation

/// What acpmux read in a search query (`acpmux-transcript-search` `syntax.rs`, `Parsed::to_json`):
/// the operators it found, as chips over the query's text, and what it could not read. acpmux
/// parses every query; the app only shows its answer and edits the query text, so there is one
/// parser (Leo, 2026-11-18: search operators "like VS Code ... likewise with Obsidian").
public nonisolated struct AcpmuxSearchParsed: Sendable, Equatable {
    /// One operator of the query, over `query[start..<end)` (UTF-16 offsets).
    public nonisolated struct Chip: Sendable, Equatable, Hashable {
        /// `phrase`, `exclude`, `from`, `agent`, `in`, `folder`, `since` or `before`.
        public var kind: String
        public var value: String
        /// The chip's text (acpmux words it: "From me", "Since 7 days ago").
        public var label: String
        public var start: Int
        public var end: Int

        public init(kind: String, value: String, label: String, start: Int, end: Int) {
            (self.kind, self.value, self.label, self.start, self.end) = (kind, value, label, start, end)
        }
    }

    public var chips: [Chip]
    /// Operators acpmux read as plain words, and why (`since:` with a bad value).
    public var problems: [String]
    /// `in:this` or `in:all` when the query names a scope.
    public var scope: String?
    public var sinceMs: Int64?
    public var beforeMs: Int64?

    public init(chips: [Chip] = [], problems: [String] = [], scope: String? = nil, sinceMs: Int64? = nil, beforeMs: Int64? = nil) {
        (self.chips, self.problems, self.scope, self.sinceMs, self.beforeMs) = (chips, problems, scope, sinceMs, beforeMs)
    }

    /// The `parsed` object of an answer; nil when it holds none (an older daemon).
    public init?(json: Any?) {
        guard let json = json as? [String: Any] else { return nil }
        chips = (json["chips"] as? [[String: Any]] ?? []).compactMap { chip in
            guard let kind = chip["kind"] as? String, let start = (chip["start"] as? NSNumber)?.intValue,
                  let end = (chip["end"] as? NSNumber)?.intValue, start <= end else { return nil }
            let value = chip["value"] as? String ?? ""
            return Chip(kind: kind, value: value, label: chip["label"] as? String ?? value, start: start, end: end)
        }
        problems = json["problems"] as? [String] ?? []
        scope = json["in"] as? String
        sinceMs = (json["sinceMs"] as? NSNumber)?.int64Value
        beforeMs = (json["beforeMs"] as? NSNumber)?.int64Value
    }
}

/// One suggestion of `_acpmux/search_syntax`: `insert` replaces `query[start..<end)` (UTF-16).
public nonisolated struct AcpmuxSearchCompletion: Sendable, Equatable, Hashable {
    public var insert: String
    public var label: String
    public var detail: String
    public var start: Int
    public var end: Int

    public init(insert: String, label: String, detail: String = "", start: Int, end: Int) {
        (self.insert, self.label, self.detail, self.start, self.end) = (insert, label, detail, start, end)
    }

    init?(json: [String: Any]) {
        guard let insert = json["insert"] as? String, let start = (json["start"] as? NSNumber)?.intValue,
              let end = (json["end"] as? NSNumber)?.intValue, start <= end else { return nil }
        self.init(insert: insert, label: json["label"] as? String ?? insert, detail: json["detail"] as? String ?? "",
                  start: start, end: end)
    }

    /// `query` with this suggestion in its span. A key's suggestion ends in its colon (`from:`), so the
    /// caret stays to type the value; a value's gets a space after it, to type the next word.
    public func applied(to query: String) -> String {
        let text = query as NSString
        let start = min(max(0, start), text.length)
        let end = min(max(start, end), text.length)
        var insert = insert
        let rest = text.substring(from: end)
        if !insert.hasSuffix(":"), !insert.hasSuffix(" "), !rest.hasPrefix(" ") { insert += " " }
        return text.substring(to: start) + insert + rest
    }
}

/// The answer of `_acpmux/search_syntax {query, cursor}`: what acpmux read and its suggestions at the cursor.
public nonisolated struct AcpmuxSearchSyntaxAnswer: Sendable, Equatable {
    public var parsed: AcpmuxSearchParsed?
    public var completions: [AcpmuxSearchCompletion]

    public init(parsed: AcpmuxSearchParsed? = nil, completions: [AcpmuxSearchCompletion] = []) {
        (self.parsed, self.completions) = (parsed, completions)
    }

    public init(result: [String: Any]) {
        parsed = AcpmuxSearchParsed(json: result["parsed"])
        completions = (result["completions"] as? [[String: Any]] ?? []).compactMap(AcpmuxSearchCompletion.init(json:))
    }
}

/// Query edits over acpmux's chips. Nothing here reads operators: the spans are acpmux's.
public nonisolated enum AcpmuxSearchQueryEdit {
    /// `query` without `chip`'s span (`query[start..<end)`, UTF-16), the spaces at the seam made one
    /// (none at either end of the query); spaces elsewhere (inside a phrase) stay as typed. A span past
    /// the text (an answer for an older query) changes nothing.
    public static func removing(_ chip: AcpmuxSearchParsed.Chip, from query: String) -> String {
        let text = query as NSString
        guard chip.start >= 0, chip.start <= chip.end, chip.end <= text.length else { return query }
        var left = text.substring(to: chip.start)
        var right = text.substring(from: chip.end)
        while left.hasSuffix(" ") { left.removeLast() }
        while right.hasPrefix(" ") { right.removeFirst() }
        return left.isEmpty || right.isEmpty ? left + right : left + " " + right
    }
}

/// One hit of `_acpmux/transcript_search` (acpmux `transcript_search.rs`): a message of a chat's
/// transcript that holds the query.
public nonisolated struct AcpmuxTranscriptHit: Sendable, Equatable, Hashable {
    /// The message's time (ms since the epoch).
    public var atMs: Int64
    /// `user`, `assistant` or `tool`.
    public var role: String
    /// The snippet on one line (its line breaks and runs of white space as one space).
    public var snippet: String
    /// Unicode scalar offsets of `snippet` that are the match.
    public var highlights: [Int]
    public var path: String?
    public var line: Int?

    public init(atMs: Int64, role: String, snippet: String, highlights: [Int] = [], path: String? = nil, line: Int? = nil) {
        (self.atMs, self.role, self.snippet, self.highlights, self.path, self.line) = (atMs, role, snippet, highlights, path, line)
    }

    public init?(json: [String: Any]) {
        guard let at = (json["at"] as? NSNumber)?.int64Value, let raw = json["snippet"] as? String else { return nil }
        let start = (json["matchStart"] as? NSNumber)?.intValue ?? -1
        let length = (json["matchLen"] as? NSNumber)?.intValue ?? 0
        let (snippet, highlights) = Self.oneLine(raw, matchBytes: start..<(start + max(0, length)))
        self.init(atMs: at, role: json["role"] as? String ?? "assistant", snippet: snippet, highlights: highlights,
                  path: json["path"] as? String, line: (json["line"] as? NSNumber)?.intValue)
    }

    /// `text` on one line, and the scalar offsets in it of the scalars `matchBytes` (UTF-8 bytes of `text`) covers.
    static func oneLine(_ text: String, matchBytes: Range<Int>) -> (String, [Int]) {
        var scalars = String.UnicodeScalarView()
        var highlights: [Int] = []
        var byte = 0
        var lastWasSpace = true
        for scalar in text.unicodeScalars {
            let width = String(scalar).utf8.count
            defer { byte += width }
            if CharacterSet.whitespacesAndNewlines.contains(scalar) {
                if !lastWasSpace { scalars.append(" ") }
                lastWasSpace = true
                continue
            }
            if matchBytes.contains(byte) { highlights.append(scalars.count) }
            scalars.append(scalar)
            lastWasSpace = false
        }
        if scalars.last == " " { scalars.removeLast() }
        return (String(scalars), highlights)
    }
}

/// One answer of `_acpmux/transcript_search`: its hits, newest first, and what acpmux read in the query.
public nonisolated struct AcpmuxTranscriptSearchAnswer: Sendable, Equatable {
    public var hits: [AcpmuxTranscriptHit]
    public var total: Int?
    public var parsed: AcpmuxSearchParsed?

    public init(hits: [AcpmuxTranscriptHit] = [], total: Int? = nil, parsed: AcpmuxSearchParsed? = nil) {
        (self.hits, self.total, self.parsed) = (hits, total, parsed)
    }

    public init(result: [String: Any]) {
        hits = (result["hits"] as? [[String: Any]] ?? []).compactMap(AcpmuxTranscriptHit.init(json:))
        total = (result["total"] as? NSNumber)?.intValue
        parsed = AcpmuxSearchParsed(json: result["parsed"])
    }
}

/// The local time zone's offset from UTC in minutes, for `since:today` (acpmux's `tzOffsetMinutes`).
nonisolated func acpmuxTimeZoneOffsetMinutes(at date: Date = Date()) -> Int {
    TimeZone.current.secondsFromGMT(for: date) / 60
}

extension AcpmuxEnvironment {
    /// `_acpmux/search_syntax {query, cursor, tzOffsetMinutes}`: what acpmux reads in `query` and
    /// its suggestions at `cursor` (UTF-16). Pure on acpmux's side (no index); nil when the daemon
    /// does not answer it (an older one).
    public nonisolated func searchSyntax(query: String, cursor: Int) async -> AcpmuxSearchSyntaxAnswer? {
        await AcpmuxSearchCalls.searchSyntax(socketPath: socketPath, query: query, cursor: cursor)
    }
}

/// The socket calls of chat search, off the caller's actor.
public nonisolated enum AcpmuxSearchCalls {
    /// `_acpmux/search_syntax` on the daemon at `socketPath`.
    @concurrent public static func searchSyntax(socketPath: String, query: String, cursor: Int) async -> AcpmuxSearchSyntaxAnswer? {
        let params: [String: any Sendable] = ["query": query, "cursor": cursor, "tzOffsetMinutes": acpmuxTimeZoneOffsetMinutes()]
        guard let result = try? await AcpmuxStatusClient.call(socketPath: socketPath, method: "_acpmux/search_syntax",
                                                              params: params, deadline: .seconds(3)) else { return nil }
        return AcpmuxSearchSyntaxAnswer(result: result)
    }

    /// `_acpmux/transcript_search` over one acpmux session's transcript (`sessionId`), newest first:
    /// This chat on the palette's chats page. Nil when acpmux did not answer.
    @concurrent public static func transcriptSearch(socketPath: String, sessionId: String, query: String,
                                                    limit: Int = 50) async -> AcpmuxTranscriptSearchAnswer? {
        let params: [String: any Sendable] = ["sessionId": sessionId, "query": query, "order": "newest", "limit": limit,
                                              "context": 160, "tzOffsetMinutes": acpmuxTimeZoneOffsetMinutes()]
        guard let result = try? await AcpmuxStatusClient.call(socketPath: socketPath, method: "_acpmux/transcript_search",
                                                              params: params, deadline: .seconds(10)) else { return nil }
        return AcpmuxTranscriptSearchAnswer(result: result)
    }
}
