import CmuxAgentBrands
public import CmuxNextAgentPane
public import CmuxNextPalette
public import Foundation

/// The command palette's chats page (`agentPane.searchChats`, decision K1): the local acpmux
/// daemon's chats, newest first, each opening its chat as a session link does. acpmux searches its
/// own index (P-RUST): the page is a window over it (`palette.chats`) whose spec carries the query,
/// so each query gets acpmux's newest matches and the app holds no copy of the index. A query also
/// finds a chat by what was said in it (the window's `content`, acpmux `chats/text.rs`): such a row
/// shows a snippet of the message with the words emphasized, and opens the chat scrolled to that
/// message (``saidReveal(_:query:)``). It replaces
/// the agent pane's own Search chats sheet, so every search goes through the one command palette.
///
/// The composition root builds it with the feed's windows, the palette's choice history and the
/// app's Open Chat path; the page holds no app state of its own.
@MainActor
public struct AgentChatsPalettePage {
    /// The page's title, as the palette's search scopes name it.
    public static var title: String { AgentChatsPaletteStrings.title }

    static let window = "palette.chats"
    /// The rows one query shows: one page of acpmux's answer.
    static let pageLimit = AgentPaneChatsQuery.maximumLimit

    private let windows: AcpmuxChatWindows?
    private let history: @MainActor (String) async -> String?
    private let open: @MainActor (String, String?) -> Void
    private let focusedChat: @MainActor () -> AgentChatsFocusedChat?
    private let syntax: (@MainActor (String, Int) async -> AcpmuxSearchSyntaxAnswer?)?
    private let scopeMemory: AgentChatsScopeMemory
    private let now: @MainActor () -> Date

    /// - Parameters:
    ///   - windows: The chat feed's windows over acpmux's index; nil without a local acpmux.
    ///   - history: The person's past choices of chats for a query (`FrecencyStore.rankerHistory`
    ///     over the rows' `chat:<key>` use), so acpmux ranks them as the palette does; nil: none. It is
    ///     encoded off the main thread (`FrecencyStore.encodedRankerHistory`), never in a key's turn.
    ///   - open: Opens a chat by key (the app's Open Chat path), scrolled to a message when the
    ///     second value names one (an agent page's `revealTurn` value).
    ///   - focusedChat: The chat the window shows in front, read when the page opens: This chat searches
    ///     it. Nil (no chat in front) hides This chat.
    ///   - syntax: acpmux's `_acpmux/search_syntax` (query, cursor): the field's operator completions; nil: none.
    ///   - scopeMemory: Where the last scope picked is kept.
    public init(windows: AcpmuxChatWindows?, history: @escaping @MainActor (String) async -> String?,
                open: @escaping @MainActor (String, String?) -> Void,
                focusedChat: @escaping @MainActor () -> AgentChatsFocusedChat? = { nil },
                syntax: (@MainActor (String, Int) async -> AcpmuxSearchSyntaxAnswer?)? = nil,
                scopeMemory: AgentChatsScopeMemory = AgentChatsScopeMemory(),
                now: @escaping @MainActor () -> Date = { Date() }) {
        self.windows = windows
        self.history = history
        self.open = open
        self.focusedChat = focusedChat
        self.syntax = syntax
        self.scopeMemory = scopeMemory
        self.now = now
    }

    /// The page, starting with `initialQuery` typed (the more row of Search opens it with Search's query).
    /// Its scope switch (This chat · Recent · All) starts on the last scope picked, Recent the first time.
    public func page(initialQuery: String = "") -> PalettePageSpec {
        let counts = PastPageCounts()
        let state = ScopeState(scope: scopeMemory.scope, focused: focusedChat(), memory: scopeMemory)
        var spec = PalettePageSpec(id: "agentChats", title: AgentChatsPaletteStrings.title, placeholder: AgentChatsPaletteStrings.placeholder,
                                   symbol: "bubble.left.and.bubble.right", providers: [], initialQuery: initialQuery,
                                   onCancel: { [windows] in windows?.close(Self.window) },
                                   queryResults: { [windows, history, open, counts, state, syntax, now] query in
                                       state.asked(query, syntax: syntax)
                                       if state.scope == .thisChat, let focused = state.focused {
                                           return Self.chatAnswers(query, in: focused, state: state)
                                       }
                                       let since = state.scope == .all ? nil : Self.recentStart(now())
                                       return Self.answers(query, windows: windows, history: history, counts: counts,
                                                           sinceMs: since, onParsed: { [weak state] in state?.show(parsed: $0, query: query) },
                                                           open: open)
                                   },
                                   filtersByQuery: false)
        spec.live = state.live
        state.showFilters()
        // Search cuts the chats to its share: this row says how many more match (the cut rows and those past the
        // answer's page, as the answer counted them) and opens this page with the query.
        spec.moreRow = { [self, counts] notShown, query in
            Self.moreInSearch(hidden: notShown + counts.pastPage(query), page: self.page(initialQuery: query))
        }
        // acpmux connected, or its index came ready: the shown query is asked again.
        // A scope picked: the shown query is asked again in it too.
        spec.onSourceChange = { [windows, state] changed in
            let token = FollowToken()
            windows?.followAvailability(token) { changed() }
            state.reask.append(changed)
            return token
        }
        return spec
    }

    /// Search's row for the chats it does not show: how many more match; it opens `page` (this page with the query).
    static func moreInSearch(hidden: Int, page: PalettePageSpec) -> PaletteItem {
        let text = AgentChatsPaletteStrings.moreInSearch(hidden)
        return PaletteItem(id: "agentChats.moreInSearch", title: text, symbol: "ellipsis.circle",
                           primary: PaletteCommand(id: "open", title: AgentChatsPaletteStrings.title, symbol: "return",
                                                   effect: .push(page)))
    }

    /// The window's rows for `query`: acpmux's first answer to its spec, then the stream ends (the
    /// palette's spinner runs until it ends, and an empty answer shows the empty state). While acpmux
    /// still reads chats for the first time (`filling`), each answer shows and the stream stays open,
    /// so a chat whose words are read later still appears. An edit
    /// opens the next query's spec; the window stays open until the page closes.
    /// - Parameters:
    ///   - history: The person's choice history of chats (`FrecencyStore.rankerHistory`), so acpmux
    ///     ranks a chat chosen before as the palette did; nil: none.
    ///   - open: Opens a chat by key (the app's Open Chat path).
    static func answers(_ query: String, windows: AcpmuxChatWindows?, history: @escaping @MainActor (String) async -> String?,
                        counts: PastPageCounts? = nil, sinceMs: Int64? = nil,
                        onParsed: (@MainActor (AcpmuxSearchParsed?) -> Void)? = nil,
                        open: @escaping @MainActor (String, String?) -> Void) -> AsyncStream<[PaletteItem]> {
        // The choice history is encoded off the main thread first; a query the palette moved past by
        // then (its stream ended) opens no window, so an older spec never replaces a newer one.
        let text = query.trimmingCharacters(in: .whitespacesAndNewlines)
        let (stream, continuation) = AsyncStream.makeStream(of: [PaletteItem].self, bufferingPolicy: .bufferingNewest(1))
        let task = Task { @MainActor in
            let encoded = text.isEmpty || windows == nil ? nil : await history(query)
            guard !Task.isCancelled else { return continuation.finish() }
            for await rows in answers(query, windows: windows, history: encoded, counts: counts, sinceMs: sinceMs,
                                      onParsed: onParsed, open: open) { continuation.yield(rows) }
            continuation.finish()
        }
        continuation.onTermination = { _ in task.cancel() }
        return stream
    }

    /// - Parameters:
    ///   - sinceMs: Only chats updated since then (the Recent scope); nil: every chat.
    ///   - onParsed: Called with what acpmux read in the query (`parsed`) with each answer.
    static func answers(_ query: String, windows: AcpmuxChatWindows?, history: String? = nil, counts: PastPageCounts? = nil,
                        sinceMs: Int64? = nil, onParsed: (@MainActor (AcpmuxSearchParsed?) -> Void)? = nil,
                        open: @escaping @MainActor (String, String?) -> Void) -> AsyncStream<[PaletteItem]> {
        let (stream, continuation) = AsyncStream.makeStream(of: [PaletteItem].self, bufferingPolicy: .bufferingNewest(1))
        guard let windows else {
            continuation.yield([])
            continuation.finish()
            return stream
        }
        let text = query.trimmingCharacters(in: .whitespacesAndNewlines)
        let spec = AcpmuxChatWindows.Spec(window: window, limit: pageLimit, topLevel: true, query: text.isEmpty ? nil : text,
                                          untitled: AgentChatsPaletteStrings.untitled, history: text.isEmpty ? nil : history,
                                          count: true, content: true, sinceMs: sinceMs)
        // The follower lives as long as this query's stream: ending it releases the token.
        let token = FollowToken()
        let answer = { (rows: [AcpmuxChat]) in
            onParsed?(windows.window(window).parsed)
            var shown = items(rows, query: text, windows: windows, open: open)
            // More chats than one page holds: say so, and how to reach the rest (R-L5: a limit
            // that leaves chats out shows it).
            if let total = windows.window(window).total, total > rows.count {
                shown.append(text.isEmpty ? moreChatsNotice(more: total - rows.count) : moreNotice(shown: rows.count, total: total))
            }
            // How many matching chats this answer leaves past its page, kept with the query (Search's more row).
            counts?.record(query, pastPage: max(0, (windows.window(window).total ?? rows.count) - rows.count))
            continuation.yield(shown)
            if !windows.window(window).filling { continuation.finish() }
        }
        windows.window(window).follow(token) { rows in
            if windows.isAnswered(spec) { answer(rows) }
        }
        // acpmux will not answer (refused, or the connection dropped): the query ends with no rows at once,
        // so the palette never waits for chats that will not come.
        let none = {
            continuation.yield([])
            continuation.finish()
        }
        windows.window(window).followRefusal(token) { none() }
        windows.open(spec)
        if windows.isAnswered(spec) {
            answer(windows.window(window).rows)
        } else if !windows.isConnected || windows.isRefused(window) {
            none()
        }
        continuation.onTermination = { _ in withExtendedLifetime(token) {} }
        return stream
    }

    /// The last row when more chats match than the page shows: not a command.
    static func moreNotice(shown: Int, total: Int) -> PaletteItem {
        let text = AgentChatsPaletteStrings.moreMatch(shown: shown, total: total)
        return PaletteItem(id: "agentChats.more", title: text, symbol: "info.circle", isEnabled: false,
                           primary: PaletteCommand(id: "none", title: text, effect: .performKeepingOpen {}))
    }

    /// The last row of the empty query when more chats exist than the page shows: not a command.
    static func moreChatsNotice(more: Int) -> PaletteItem {
        let text = AgentChatsPaletteStrings.moreChats(more)
        return PaletteItem(id: "agentChats.more", title: text, symbol: "info.circle", isEnabled: false,
                           primary: PaletteCommand(id: "none", title: text, effect: .performKeepingOpen {}))
    }

    /// How many matching chats each query's answer left past its page, recorded when it answered: the more row
    /// reads it after the window closed or moved on to another spec.
    final class PastPageCounts {
        private var byQuery: [String: Int] = [:]

        func record(_ query: String, pastPage: Int) { byQuery[query] = pastPage }

        func pastPage(_ query: String) -> Int { byQuery[query] ?? 0 }
    }

    /// Owns one query's follower (released with its stream).
    final class FollowToken: Sendable {}

    /// The `revealTurn` value that opens a chat at the message a query found in it: the newest
    /// prompt or answer row whose text holds every word (the agent page's `said:` reveal,
    /// webviews/src/agent-session/acpmux/links.ts). A message the transcript index found (it has a
    /// time, older than the text acpmux holds of the chat) opens Find in Chat at the index hit
    /// nearest that time instead (`find:<atMs>:<query>`).
    static func saidReveal(_ said: AcpmuxChat.Said, query: String) -> String {
        if let atMs = said.atMs { return "find:\(atMs):" + query }
        return "said:" + (said.isAnswer ? "answer" : "prompt") + ":" + query
    }

    static func items(_ chats: [AcpmuxChat], query: String = "", windows: AcpmuxChatWindows,
                      open: @escaping @MainActor (String, String?) -> Void) -> [PaletteItem] {
        let formatter = RelativeDateTimeFormatter()
        return chats.map { chat in
            let reveal = chat.said.map { saidReveal($0, query: query) }
            var item = PaletteItem(
                id: "chat:\(chat.id)", title: chat.title ?? AgentChatsPaletteStrings.untitled,
                // A chat found by what was said shows the message's words where its folder was.
                subtitle: chat.said?.snippet ?? chat.cwd.map { ($0 as NSString).abbreviatingWithTildeInPath },
                accessory: chat.updatedAt.timeIntervalSince1970 > 0 ? formatter.localizedString(for: chat.updatedAt, relativeTo: Date()) : nil,
                symbol: "bubble.left", brand: chat.brand ?? AgentBrandCatalog.brand(for: chat.harness)?.rawValue,
                keywords: [chat.harness] + (chat.cwd.map { [$0] } ?? []),
                primary: PaletteCommand(id: "open", title: AgentChatsPaletteStrings.open, symbol: "return",
                                        effect: .perform {
                                            // The page closes with this command: its window too.
                                            windows.close(window)
                                            open(chat.id, reveal)
                                        }))
            item.subtitleHighlights = chat.said?.highlights ?? []
            return item
        }
    }
}

/// Text of the chats page. Keys live in Resources/Localizable.xcstrings.
struct AgentChatsPaletteStrings {
    static var title: String { String(localized: "handlers.agentChats.title", defaultValue: "Agent Chats", bundle: .module) }
    static var placeholder: String { String(localized: "handlers.agentChats.placeholder", defaultValue: "Search chats", bundle: .module) }
    static var open: String { String(localized: "handlers.agentChats.open", defaultValue: "Open Chat", bundle: .module) }
    static var untitled: String { String(localized: "handlers.agentChats.untitled", defaultValue: "New chat", bundle: .module) }
    /// The bundle of this module's catalog, so a test can read one language's table (`fr.lproj`).
    static var bundle: Bundle { .module }

    /// The row under the empty query's page when `more` chats are not shown. `bundle` and `locale`
    /// default to this module's catalog in the user's language; a test passes one language's table.
    static func moreChats(_ more: Int, bundle: Bundle = .module, locale: Locale = .current) -> String {
        String(localized: "handlers.agentChats.moreChats", defaultValue: "\(more) more chats. Type to search.",
               bundle: bundle, locale: locale)
    }

    /// Search's row for the `hidden` matching chats it does not show (it opens the chats page with the query).
    static func moreInSearch(_ hidden: Int, bundle: Bundle = .module, locale: Locale = .current) -> String {
        String(format: String(localized: "handlers.agentChats.moreInSearch", defaultValue: "More matching chats: %lld",
                              bundle: bundle, locale: locale), locale: locale, Int64(hidden))
    }

    /// The row under a page that shows only the best `shown` of `total` matching chats.
    static func moreMatch(shown: Int, total: Int) -> String {
        String(format: String(localized: "handlers.agentChats.moreMatch", defaultValue: "The best %1$lld of %2$lld matching chats. Type more to narrow.",
                              bundle: .module), Int64(shown), Int64(total))
    }
}
