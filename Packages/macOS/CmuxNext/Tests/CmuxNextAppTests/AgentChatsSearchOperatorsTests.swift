@testable import CmuxNextAgentLanes
@testable import CmuxNextAgentPane
import CmuxNextPalette
import Foundation
import Testing

/// The chats page's scope switch and search operators (Leo, 2026-11-18: "recency is usually what's
/// desired ... when people are searching", and operators "like VS Code ... likewise with Obsidian").
/// acpmux parses every query; the app shows its chips and completions and edits the query text.
@MainActor @Suite struct AgentChatsSearchOperatorsTests {
    private static func memory() -> (AgentChatsScopeMemory, UserDefaults) {
        let name = "AgentChatsSearchOperatorsTests.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defaults.removePersistentDomain(forName: name)
        return (AgentChatsScopeMemory(defaults: defaults), defaults)
    }

    private static let focused = AgentChatsFocusedChat(search: { _ in nil }, reveal: { _ in })

    // MARK: Scope memory

    @Test func theScopeIsRecentUntilOneIsPickedAndThenRemembered() {
        let (memory, defaults) = Self.memory()
        #expect(memory.scope == .recent, "Recent by default")
        let state = AgentChatsPalettePage.ScopeState(scope: memory.scope, focused: Self.focused, memory: memory)
        state.pick(.all)
        #expect(defaults.string(forKey: AgentChatsScopeMemory.defaultsKey) == "all")
        #expect(AgentChatsScopeMemory(defaults: defaults).scope == .all, "the next page opens on the last pick")
        state.pick(.thisChat)
        #expect(AgentChatsScopeMemory(defaults: defaults).scope == .thisChat)
        defaults.set("bogus", forKey: AgentChatsScopeMemory.defaultsKey)
        #expect(AgentChatsScopeMemory(defaults: defaults).scope == .recent, "an unknown value reads as Recent")
    }

    @Test func thisChatIsHiddenWithNoChatInFrontAndThePickStaysKept() {
        let (memory, _) = Self.memory()
        memory.scope = .thisChat
        let state = AgentChatsPalettePage.ScopeState(scope: memory.scope, focused: nil, memory: memory)
        #expect(state.scope == .recent, "no chat in front: Recent")
        #expect(state.scopes == [.recent, .all])
        state.showFilters()
        #expect(state.live.filters?.options.map(\.id) == ["recent", "all"])
        #expect(memory.scope == .thisChat, "the kept pick is not overwritten by the fallback")
        let withChat = AgentChatsPalettePage.ScopeState(scope: memory.scope, focused: Self.focused, memory: memory)
        #expect(withChat.scope == .thisChat)
        #expect(withChat.scopes == [.thisChat, .recent, .all])
    }

    @Test func pickingAScopeAsksTheShownQueryAgain() {
        let (memory, _) = Self.memory()
        let state = AgentChatsPalettePage.ScopeState(scope: .recent, focused: nil, memory: memory)
        var asked = 0
        state.reask.append { asked += 1 }
        state.showFilters()
        state.live.filters?.select("all")
        #expect(asked == 1)
        #expect(state.live.filters?.selected == "all")
        state.live.filters?.select("all")
        #expect(asked == 1, "the same scope again asks nothing")
    }

    // MARK: The spec's sinceMs

    @Test func recentStartsSevenDaysBackToTheMinute() {
        let now = Date(timeIntervalSince1970: 1_800_000_000.5)
        let start = AgentChatsPalettePage.recentStart(now)
        #expect(start == (1_800_000_000 - 7 * 86_400) / 60 * 60 * 1000)
        #expect(AgentChatsPalettePage.recentStart(now.addingTimeInterval(20)) == start, "the same minute: the same spec")
    }

    @Test func theSpecSendsSinceMsOnlyWhenItHasOne() {
        let recent = AcpmuxChatWindows.Spec(window: "w", limit: 10, query: "parser", sinceMs: 1_234)
        let params = recent.params(generation: 1)
        #expect(params["sinceMs"] as? Int64 == 1_234)
        #expect(params["tzOffsetMinutes"] is Int, "since:today reads the local day")
        let all = AcpmuxChatWindows.Spec(window: "w", limit: 10)
        #expect(all.params(generation: 1)["sinceMs"] == nil)
        #expect(recent != AcpmuxChatWindows.Spec(window: "w", limit: 10, query: "parser"), "a scope change is a new spec")
    }

    @Test func theRecentPageSendsSinceMsAndAllSendsNone() async throws {
        let windows = AcpmuxChatWindows()
        var sent: [[String: any Sendable]] = []
        windows.connected { (_: String, params: [String: any Sendable]) in sent.append(params) }
        let (memory, _) = Self.memory()
        let now = Date(timeIntervalSince1970: 1_800_000_000)
        let page = AgentChatsPalettePage(windows: windows, history: { _ in nil }, open: { _, _ in }, scopeMemory: memory,
                                         now: { now }).page()
        let results = try #require(page.queryResults)
        _ = results("parser")
        for _ in 0..<500 where !sent.contains(where: { $0["query"] as? String == "parser" }) { await Task.yield() }
        #expect(sent.last?["sinceMs"] as? Int64 == AgentChatsPalettePage.recentStart(now))
        page.live?.filters?.select("all")
        _ = results("parser all")
        for _ in 0..<500 where !sent.contains(where: { $0["query"] as? String == "parser all" }) { await Task.yield() }
        #expect(sent.last?["query"] as? String == "parser all")
        #expect(sent.last?["sinceMs"] == nil, "All reaches every chat")
        #expect(page.live?.filters?.options.map(\.id) == ["recent", "all"], "no chat in front: no This chat")
    }

    // MARK: Chip removal

    private static func chip(_ query: String, _ part: String, kind: String = "from") -> AcpmuxSearchParsed.Chip {
        let range = (query as NSString).range(of: part)
        return .init(kind: kind, value: part, label: part, start: range.location, end: range.location + range.length)
    }

    @Test func removingAChipTakesOutItsSpanAndTidiesTheSeam() {
        let query = "parser from:me since:7d"
        #expect(AcpmuxSearchQueryEdit.removing(Self.chip(query, "from:me"), from: query) == "parser since:7d")
        #expect(AcpmuxSearchQueryEdit.removing(Self.chip(query, "parser"), from: query) == "from:me since:7d")
        #expect(AcpmuxSearchQueryEdit.removing(Self.chip(query, "since:7d"), from: query) == "parser from:me")
        let phrase = #"fix  "a  b"  -old"#
        #expect(AcpmuxSearchQueryEdit.removing(Self.chip(phrase, "-old", kind: "exclude"), from: phrase) == #"fix  "a  b""#,
                "spaces away from the seam (a phrase's) stay as typed")
    }

    @Test func removingCountsUTF16AndIgnoresAStaleSpan() {
        let query = "🦊 from:me fox"
        let chip = Self.chip(query, "from:me")
        #expect(chip.start == 3, "the emoji is two UTF-16 units")
        #expect(AcpmuxSearchQueryEdit.removing(chip, from: query) == "🦊 fox")
        let stale = AcpmuxSearchParsed.Chip(kind: "from", value: "me", label: "From me", start: 10, end: 40)
        #expect(AcpmuxSearchQueryEdit.removing(stale, from: "short") == "short", "a span past the text changes nothing")
    }

    @Test func aChipTokenEditsTheQueryAndShowsAcpmuxsLabel() {
        let query = "agent:codex parser"
        let tokens = AgentChatsPalettePage.ScopeState.tokens([Self.chip(query, "agent:codex", kind: "agent")])
        #expect(tokens.map(\.title) == ["agent:codex"])
        #expect(tokens.first?.symbol == "cpu")
        #expect(tokens.first?.editsQuery?(query) == "parser")
    }

    // MARK: Decode

    @Test func aWindowAnswerCarriesParsedAndADiffKeepsIt() throws {
        let windows = AcpmuxChatWindows()
        var sent: [[String: any Sendable]] = []
        windows.connected { (_: String, params: [String: any Sendable]) in sent.append(params) }
        let spec = AcpmuxChatWindows.Spec(window: "palette.chats", limit: 10, query: "from:me parser")
        windows.open(spec)
        let gen = try #require(sent.last?["gen"] as? UInt64)
        let parsed: [String: Any] = [
            "terms": [["text": "parser", "phrase": false]], "exclude": [], "role": "user", "sinceMs": 5_000,
            "chips": [["kind": "from", "value": "me", "label": "From me", "start": 0, "end": 7]], "problems": ["since: needs a time"],
        ]
        let full: [String: Any] = ["window": "palette.chats", "gen": NSNumber(value: gen), "revision": 1, "full": true,
                                   "keys": [], "rows": [], "parsed": parsed]
        windows.apply(try #require(AcpmuxChatWindows.Update(params: full)))
        let shown = try #require(windows.window("palette.chats").parsed)
        #expect(shown.chips == [.init(kind: "from", value: "me", label: "From me", start: 0, end: 7)])
        #expect(shown.problems == ["since: needs a time"])
        #expect(shown.sinceMs == 5_000)
        let diff: [String: Any] = ["window": "palette.chats", "gen": NSNumber(value: gen), "revision": 2, "keys": [], "rows": []]
        windows.apply(try #require(AcpmuxChatWindows.Update(params: diff)))
        #expect(windows.window("palette.chats").parsed == shown, "a diff without parsed keeps the last")
        let bare: [String: Any] = ["window": "palette.chats", "gen": NSNumber(value: gen), "revision": 3, "full": true, "keys": [], "rows": []]
        windows.apply(try #require(AcpmuxChatWindows.Update(params: bare)))
        #expect(windows.window("palette.chats").parsed == nil, "a full answer without parsed has none (an older daemon)")
    }

    @Test func searchSyntaxDecodesCompletionsAndTabAppliesOne() {
        let answer = AcpmuxSearchSyntaxAnswer(result: [
            "parsed": ["chips": [], "problems": [], "terms": [], "exclude": []],
            "completions": [["insert": "from:", "label": "from:", "detail": "who wrote it", "start": 7, "end": 9],
                            ["insert": "bad"]],
        ])
        #expect(answer.parsed?.chips.isEmpty == true)
        #expect(answer.completions == [.init(insert: "from:", label: "from:", detail: "who wrote it", start: 7, end: 9)])
        #expect(answer.completions[0].applied(to: "parser fr") == "parser from:", "a key keeps the caret for its value")
        let value = AcpmuxSearchCompletion(insert: "today", label: "today", start: 6, end: 8)
        #expect(value.applied(to: "since:to") == "since:today ", "a value gets a space for the next word")
        #expect(value.applied(to: "since:to fix") == "since:today fix")
    }

    @Test func aCompletionForAnOlderQueryChangesNothing() {
        let (memory, _) = Self.memory()
        let state = AgentChatsPalettePage.ScopeState(scope: .recent, focused: nil, memory: memory)
        state.asked("fr", syntax: nil)
        state.show(completions: [.init(insert: "from:", label: "from:", start: 0, end: 2)], query: "fr")
        #expect(state.live.completion?.label == "from:")
        #expect(state.live.completion?.apply("fr") == "from:")
        #expect(state.live.completion?.apply("fro") == "fro", "the query moved on: Tab changes nothing")
        state.asked("fro", syntax: nil)
        #expect(state.live.completion == nil, "a new query drops the old completion")
        state.show(completions: [.init(insert: "from:", label: "from:", start: 0, end: 2)], query: "fr")
        #expect(state.live.completion == nil, "an answer for an older query is not shown")
    }

    @Test func theOperatorsHintShowsForAnEmptyQueryAndAfterAKeyWithNoValue() {
        typealias State = AgentChatsPalettePage.ScopeState
        #expect(State.wantsOperatorHint(""))
        #expect(State.wantsOperatorHint("  "))
        #expect(State.wantsOperatorHint("parser from: "))
        #expect(!State.wantsOperatorHint("parser from:"), "still typing the value")
        #expect(!State.wantsOperatorHint("parser "))
        #expect(!State.wantsOperatorHint("from:me "))
        let (memory, _) = Self.memory()
        let state = State(scope: .recent, focused: nil, memory: memory)
        state.asked("", syntax: nil)
        #expect(state.live.hint == AgentChatsPaletteStrings.operatorsHint)
        state.asked("parser", syntax: nil)
        #expect(state.live.hint == nil)
        state.show(parsed: AcpmuxSearchParsed(problems: ["since:x is not a time"]), query: "parser")
        #expect(state.live.hint == "since:x is not a time", "acpmux's problem shows on the hint line")
    }

    // MARK: This chat

    @Test func aTranscriptHitDecodesItsMatchAsScalarOffsetsOnOneLine() throws {
        // "héllo\n  wörld": the match is "wörld", bytes 9..<15 (é and ö are two bytes each).
        let snippet = "héllo\n  wörld"
        let start = "héllo\n  ".utf8.count
        let hit = try #require(AcpmuxTranscriptHit(json: ["at": 1_700_000_000_000, "role": "user", "snippet": snippet,
                                                          "matchStart": start, "matchLen": "wörld".utf8.count]))
        #expect(hit.snippet == "héllo wörld")
        #expect(hit.highlights == [6, 7, 8, 9, 10])
        #expect(hit.role == "user")
    }

    @Test func thisChatRowsAreHitsNewestFirstOpeningFindInChat() throws {
        var revealed: [String] = []
        let chat = AgentChatsFocusedChat(search: { _ in nil }, reveal: { revealed.append($0) })
        let hits = [AcpmuxTranscriptHit(atMs: 1_000, role: "user", snippet: "old parser"),
                    AcpmuxTranscriptHit(atMs: 3_000, role: "assistant", snippet: "new parser", highlights: [4, 5]),
                    AcpmuxTranscriptHit(atMs: 2_000, role: "tool", snippet: "mid parser")]
        let rows = AgentChatsPalettePage.chatItems(hits, query: "parser", chat: chat, now: Date(timeIntervalSince1970: 10))
        #expect(rows.map(\.subtitle) == ["new parser", "mid parser", "old parser"])
        #expect(rows.first?.subtitleHighlights == [4, 5])
        #expect(rows.first?.title == AgentChatsPaletteStrings.speaker("assistant"))
        #expect(rows.allSatisfy { $0.accessory != nil }, "each hit says when")
        guard case .perform(let run) = try #require(rows.first).primary.effect else {
            Issue.record("a hit row runs its reveal")
            return
        }
        run()
        #expect(revealed == ["find:3000:parser"])
    }

    @Test(.timeLimit(.minutes(1))) func thisChatAsksTheChatsTranscriptAndShowsItsChips() async throws {
        let (memory, _) = Self.memory()
        var asked: [String] = []
        let parsed = AcpmuxSearchParsed(chips: [.init(kind: "from", value: "me", label: "From me", start: 7, end: 14)])
        let chat = AgentChatsFocusedChat(search: { query in
            asked.append(query)
            return AcpmuxTranscriptSearchAnswer(hits: [AcpmuxTranscriptHit(atMs: 9, role: "user", snippet: "parser")], parsed: parsed)
        }, reveal: { _ in })
        let state = AgentChatsPalettePage.ScopeState(scope: .thisChat, focused: chat, memory: memory)
        state.asked("parser from:me", syntax: nil)
        var answers: [[PaletteItem]] = []
        for await rows in AgentChatsPalettePage.chatAnswers("parser from:me", in: chat, state: state) { answers.append(rows) }
        #expect(asked == ["parser from:me"])
        #expect(answers.last?.count == 1)
        #expect(state.live.tokens.map(\.title) == ["From me"])
        #expect(state.live.tokens.first?.editsQuery?("parser from:me") == "parser")
        var empty: [[PaletteItem]] = []
        for await rows in AgentChatsPalettePage.chatAnswers("  ", in: chat, state: state) { empty.append(rows) }
        #expect(empty.allSatisfy(\.isEmpty), "an empty query asks nothing")
        #expect(asked.count == 1)
    }
}
