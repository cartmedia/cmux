import CmuxNextAgentLanes
import CmuxNextAgentPane
import CmuxNextPalette

extension AgentChatsPalettePage {
    /// The chats page over the app's feed: the palette's choice history of chats ranks it, and a
    /// row opens through the app's Open Chat path. This chat searches the agent chat the active
    /// window's focused pane shows, and acpmux completes the field's search operators.
    init(services: AppServices) {
        var syntax: (@MainActor (String, Int) async -> AcpmuxSearchSyntaxAnswer?)?
        if let socket = services.chatsFeed?.socketPath {
            syntax = { @MainActor (query: String, cursor: Int) async -> AcpmuxSearchSyntaxAnswer? in
                await AcpmuxSearchCalls.searchSyntax(socketPath: socket, query: query, cursor: cursor)
            }
        }
        self.init(windows: services.chatsFeed?.windows,
                  history: { [weak services] query in await services?.palette.model.rankerHistory(keyPrefix: "chat:", query: query) },
                  open: { [weak services] key, reveal in services?.chatsOpener.open(key, reveal: reveal) },
                  focusedChat: { [weak services] () -> AgentChatsFocusedChat? in
                      guard let services else { return nil }
                      return Self.focusedChat(services)
                  },
                  syntax: syntax)
    }

    /// The agent chat the active window's focused pane shows, on this Mac (a remote chat has no local socket).
    @MainActor
    private static func focusedChat(_ services: AppServices) -> AgentChatsFocusedChat? {
        guard let key = services.windows.active?.focusedPane?.currentTabKey, let view = services.agentTabs.existingView(key),
              let session = view.model.sessionId, !session.isEmpty, let socket = view.model.sessionCopySocketPath else { return nil }
        return AgentChatsFocusedChat(
            search: { query in await AcpmuxSearchCalls.transcriptSearch(socketPath: socket, sessionId: session, query: query) },
            reveal: { [weak services] reveal in
                guard let services else { return }
                // The chat's tab in front again (the palette came over it), then Find in Chat at the hit.
                _ = services.revealTab(key)
                services.agentTabs.revealTurn(reveal, in: key)
            })
    }
}
