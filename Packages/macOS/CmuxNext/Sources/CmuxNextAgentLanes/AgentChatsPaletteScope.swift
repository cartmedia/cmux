public import CmuxNextAgentPane
public import CmuxNextPalette
public import Foundation

/// Where the chats page searches (Leo, 2026-11-18: "recency is usually what's desired ... when
/// people are searching"): the chat in front, the chats of the last 7 days, or every chat.
public enum AgentChatsScope: String, CaseIterable, Sendable {
    case thisChat
    case recent
    case all

    /// How far back Recent reaches.
    public static let recentDays = 7
}

/// The last scope picked on the chats page, kept across launches. Recent until one is picked.
public struct AgentChatsScopeMemory {
    public static let defaultsKey = "cmuxNext.palette.chats.scope"
    private let defaults: UserDefaults
    private let key: String

    public init(defaults: UserDefaults = .standard, key: String = AgentChatsScopeMemory.defaultsKey) {
        self.defaults = defaults
        self.key = key
    }

    public var scope: AgentChatsScope {
        get { defaults.string(forKey: key).flatMap(AgentChatsScope.init(rawValue:)) ?? .recent }
        nonmutating set { defaults.set(newValue.rawValue, forKey: key) }
    }
}

/// The chat in front when the chats page opened: This chat searches its transcript
/// (`_acpmux/transcript_search` with its session) and a hit opens Find in Chat there.
public struct AgentChatsFocusedChat {
    /// Searches the chat's transcript for a query, newest first; nil when acpmux did not answer.
    public let search: @MainActor (String) async -> AcpmuxTranscriptSearchAnswer?
    /// Scrolls the chat to a `revealTurn` value (`find:<atMs>:<query>`) and shows it.
    public let reveal: @MainActor (String) -> Void

    public init(search: @escaping @MainActor (String) async -> AcpmuxTranscriptSearchAnswer?,
                reveal: @escaping @MainActor (String) -> Void) {
        self.search = search
        self.reveal = reveal
    }
}

extension AgentChatsPalettePage {
    /// The start of Recent at `now`: 7 days back, to the minute (so a spec asked twice in a minute is the same).
    static func recentStart(_ now: Date) -> Int64 {
        let start = now.addingTimeInterval(-Double(AgentChatsScope.recentDays) * 86_400).timeIntervalSince1970
        return Int64((start / 60).rounded(.down)) * 60_000
    }

    /// One open chats page's scope and what it shows around the field (`PalettePageLive`).
    @MainActor
    final class ScopeState {
        private(set) var scope: AgentChatsScope
        let focused: AgentChatsFocusedChat?
        let live = PalettePageLive()
        private let memory: AgentChatsScopeMemory
        /// Ask the palette for the shown query again (`PalettePageSpec.onSourceChange`): a scope picked.
        var reask: [@MainActor () -> Void] = []
        /// The query last asked, so an answer for an older one changes nothing.
        private(set) var query = ""
        private var syntaxTask: Task<Void, Never>?

        init(scope: AgentChatsScope, focused: AgentChatsFocusedChat?, memory: AgentChatsScopeMemory) {
            // This chat needs a chat in front; without one the page searches Recent (the pick stays kept).
            self.scope = scope == .thisChat && focused == nil ? .recent : scope
            self.focused = focused
            self.memory = memory
        }

        /// The scopes on offer: This chat only with a chat in front.
        var scopes: [AgentChatsScope] { AgentChatsScope.allCases.filter { $0 != .thisChat || focused != nil } }

        func pick(_ scope: AgentChatsScope) {
            guard scope != self.scope, scopes.contains(scope) else { return }
            self.scope = scope
            memory.scope = scope
            showFilters()
            for ask in reask { ask() }
        }

        func showFilters() {
            live.filters = PaletteFilterSwitch(options: scopes.map { PaletteFilterOption(id: $0.rawValue, title: AgentChatsPaletteStrings.scope($0)) },
                                               selected: scope.rawValue) { [weak self] id in
                if let scope = AgentChatsScope(rawValue: id) { self?.pick(scope) }
            }
        }

        /// A query is asked: its chips and completion wait for acpmux's answers (an older query's never stay).
        func asked(_ query: String, syntax: (@MainActor (String, Int) async -> AcpmuxSearchSyntaxAnswer?)?) {
            let changed = query != self.query
            self.query = query
            if changed {
                live.completion = nil
                if query.trimmingCharacters(in: .whitespaces).isEmpty { live.tokens = [] }
            }
            live.hint = Self.wantsOperatorHint(query) ? AgentChatsPaletteStrings.operatorsHint : nil
            syntaxTask?.cancel()
            guard let syntax, !query.isEmpty else { return }
            let cursor = query.utf16.count
            syntaxTask = Task { @MainActor [weak self] in
                guard let answer = await syntax(query, cursor), !Task.isCancelled, let self, self.query == query else { return }
                self.show(completions: answer.completions, query: query)
                // This chat's answer carries `parsed` too; the syntax answer fills in the chips first.
                if let parsed = answer.parsed { self.show(parsed: parsed, query: query) }
            }
        }

        /// acpmux's chips for `query`, each taking its part out of the query with one click.
        func show(parsed: AcpmuxSearchParsed?, query: String) {
            guard query == self.query else { return }
            live.tokens = Self.tokens(parsed?.chips ?? [])
            if let problem = parsed?.problems.first { live.hint = problem }
        }

        func show(completions: [AcpmuxSearchCompletion], query: String) {
            guard query == self.query else { return }
            live.completion = completions.first.map { completion in
                PaletteQueryCompletion(label: completion.label) { shown in
                    // Tab on a query that moved on from the one this answers changes nothing.
                    shown == query ? completion.applied(to: shown) : shown
                }
            }
        }

        static func tokens(_ chips: [AcpmuxSearchParsed.Chip]) -> [PaletteToken] {
            chips.map { chip in
                PaletteToken(id: "\(chip.kind):\(chip.start)", title: chip.label, symbol: symbol(chip.kind),
                             editsQuery: { query in AcpmuxSearchQueryEdit.removing(chip, from: query) })
            }
        }

        static func symbol(_ kind: String) -> String {
            switch kind {
            case "phrase": "quote.opening"
            case "exclude": "minus.circle"
            case "from": "person"
            case "agent": "cpu"
            case "in": "scope"
            case "folder": "folder"
            case "since", "before": "calendar"
            default: "line.3.horizontal.decrease"
            }
        }

        /// The operators line shows for an empty query, and after a key typed with no value then a
        /// space (`from: `): the field is between words, so the operators are what to type next. Only the
        /// last word's shape is read; acpmux reads the operators.
        static func wantsOperatorHint(_ query: String) -> Bool {
            if query.trimmingCharacters(in: .whitespaces).isEmpty { return true }
            guard query.hasSuffix(" ") else { return false }
            let last = query.split(separator: " ").last.map(String.init) ?? ""
            return last.hasSuffix(":") && !last.hasPrefix("\"")
        }
    }

    /// This chat: the chat in front's transcript hits for `query`, newest first, each opening Find in
    /// Chat at its message (`find:<atMs>:<query>`). An empty query lists nothing (the operators hint says what to type).
    static func chatAnswers(_ query: String, in chat: AgentChatsFocusedChat, state: ScopeState) -> AsyncStream<[PaletteItem]> {
        let (stream, continuation) = AsyncStream.makeStream(of: [PaletteItem].self, bufferingPolicy: .bufferingNewest(1))
        let text = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else {
            continuation.yield([])
            continuation.finish()
            return stream
        }
        let task = Task { @MainActor [weak state] in
            let answer = await chat.search(text)
            guard !Task.isCancelled else { return continuation.finish() }
            state?.show(parsed: answer?.parsed, query: query)
            continuation.yield(chatItems(answer?.hits ?? [], query: text, chat: chat))
            continuation.finish()
        }
        continuation.onTermination = { _ in task.cancel() }
        return stream
    }

    static func chatItems(_ hits: [AcpmuxTranscriptHit], query: String, chat: AgentChatsFocusedChat,
                          now: Date = Date()) -> [PaletteItem] {
        let formatter = RelativeDateTimeFormatter()
        // Newest first, whatever order the answer came in.
        let sorted = hits.enumerated().sorted { $0.element.atMs != $1.element.atMs ? $0.element.atMs > $1.element.atMs : $0.offset < $1.offset }
        return sorted.map { pair in
            let (index, hit) = (pair.offset, pair.element)
            let reveal = "find:\(hit.atMs):" + query
            let date = Date(timeIntervalSince1970: Double(hit.atMs) / 1000)
            var item = PaletteItem(
                id: "hit:\(hit.atMs):\(index)", title: AgentChatsPaletteStrings.speaker(hit.role),
                subtitle: hit.snippet, accessory: formatter.localizedString(for: date, relativeTo: now),
                symbol: hit.role == "user" ? "person" : "bubble.left",
                primary: PaletteCommand(id: "open", title: AgentChatsPaletteStrings.showInChat, symbol: "return",
                                        effect: .perform { chat.reveal(reveal) }))
            item.subtitleHighlights = hit.highlights
            return item
        }
    }
}

extension AgentChatsPaletteStrings {
    static func scope(_ scope: AgentChatsScope) -> String {
        switch scope {
        case .thisChat: String(localized: "handlers.agentChats.scope.thisChat", defaultValue: "This Chat", bundle: .module)
        case .recent: String(localized: "handlers.agentChats.scope.recent", defaultValue: "Recent (7 days)", bundle: .module)
        case .all: String(localized: "handlers.agentChats.scope.all", defaultValue: "All", bundle: .module)
        }
    }

    /// The line under the field listing the search operators (VS Code's and Obsidian's search syntax).
    static var operatorsHint: String {
        String(localized: "handlers.agentChats.operatorsHint",
               defaultValue: "from:me  from:agent  agent:claude  folder:name  since:7d  before:2026-01-31  \"exact phrase\"  -word",
               bundle: .module)
    }

    static var showInChat: String { String(localized: "handlers.agentChats.showInChat", defaultValue: "Show in Chat", bundle: .module) }

    /// Who wrote a hit of This chat.
    static func speaker(_ role: String) -> String {
        switch role {
        case "user": String(localized: "handlers.agentChats.speaker.you", defaultValue: "You", bundle: .module)
        case "tool": String(localized: "handlers.agentChats.speaker.tool", defaultValue: "Tool", bundle: .module)
        default: String(localized: "handlers.agentChats.speaker.agent", defaultValue: "Agent", bundle: .module)
        }
    }
}
