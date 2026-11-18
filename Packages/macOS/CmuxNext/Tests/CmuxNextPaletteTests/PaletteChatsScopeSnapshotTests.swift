import AppKit
import CmuxNextActions
import CmuxNextDesign
import CmuxTheme
@testable import CmuxNextPalette
import CmuxNextTestSupport
import Foundation
import Testing

/// Draws the chats page's search chrome (Leo, 2026-11-18: recency first, and search operators "like
/// VS Code ... likewise with Obsidian"), light and dark, one PNG per state into
/// `CMUX_PALETTE_CHATS_SNAPSHOT_DIR` (else `.build/palette-chats/` in the package): the scope
/// switch on This chat, Recent and All, operator chips in the field, and a Tab completion open.
/// The page here is a stand-in with the chats page's live chrome (`PalettePageLive`) and sample
/// rows; the chats page's behavior is checked by `AgentChatsSearchOperatorsTests`.
@MainActor
@Suite(.paletteRanker, .closesItsWindows)
struct PaletteChatsScopeSnapshotTests {
    struct State {
        let name: String
        let scope: String
        let query: String
        let chips: [String]
        let completion: String?
        let hint: String?
        let rows: [PaletteItem]
    }

    private static let options = [PaletteFilterOption(id: "thisChat", title: "This Chat"),
                                  PaletteFilterOption(id: "recent", title: "Recent (7 days)"),
                                  PaletteFilterOption(id: "all", title: "All")]
    private static let operators = "from:me  from:agent  agent:claude  folder:name  since:7d  before:2026-01-31  \"exact phrase\"  -word"

    private static func row(_ id: String, _ title: String, _ subtitle: String, _ when: String, symbol: String = "bubble.left",
                            highlights: [Int] = []) -> PaletteItem {
        var item = PaletteItem(id: id, title: title, subtitle: subtitle, accessory: when, symbol: symbol,
                               primary: PaletteCommand(id: "open", title: "Open Chat", symbol: "return", effect: .perform {}))
        item.subtitleHighlights = highlights
        return item
    }

    private static func highlights(_ text: String, _ word: String) -> [Int] {
        guard let range = text.range(of: word) else { return [] }
        let start = text.unicodeScalars.distance(from: text.unicodeScalars.startIndex, to: range.lowerBound)
        return Array(start..<(start + word.unicodeScalars.count))
    }

    static let chats = [
        row("c1", "Port the parser to Rust", "~/Projects/todo-app", "2 min. ago"),
        row("c2", "Sidebar drag and drop", "~/Projects/todo-app", "1 hr. ago"),
        row("c3", "Fix flaky login test", "~/Projects/todo-app", "yesterday"),
        row("c4", "Dark mode colors", "~/Projects/todo-app", "3 days ago"),
    ]

    static let states: [State] = {
        let hit1 = "The parser now keeps comments when it reflows a block"
        let hit2 = "Can you make the parser keep comments?"
        let found = "We should keep the parser in Rust and call it over FFI"
        return [
            State(name: "recent", scope: "recent", query: "", chips: [], completion: nil, hint: operators, rows: chats),
            State(name: "this-chat", scope: "thisChat", query: "parser", chips: [], completion: nil, hint: nil, rows: [
                row("h1", "Agent", hit1, "4 min. ago", highlights: highlights(hit1, "parser")),
                row("h2", "You", hit2, "6 min. ago", symbol: "person", highlights: highlights(hit2, "parser")),
            ]),
            State(name: "all", scope: "all", query: "parser", chips: [], completion: nil, hint: nil, rows: [
                row("c1", "Port the parser to Rust", found, "2 min. ago", highlights: highlights(found, "parser")),
                row("c9", "Old parser notes", "~/Projects/todo-app", "5 mo. ago"),
            ]),
            State(name: "chips", scope: "recent", query: "parser from:me agent:codex since:7d", chips: ["From me", "Codex", "Since 7 days ago"],
                  completion: nil, hint: nil, rows: [row("h2", "Port the parser to Rust", hit2, "6 min. ago", highlights: highlights(hit2, "parser"))]),
            State(name: "autocomplete", scope: "recent", query: "parser fr", chips: [], completion: "from:", hint: nil, rows: chats.prefix(2).map { $0 }),
        ]
    }()

    private static func page(_ state: State) -> PalettePageSpec {
        let live = PalettePageLive()
        live.filters = PaletteFilterSwitch(options: options, selected: state.scope) { _ in }
        live.tokens = state.chips.map { PaletteToken(id: $0, title: $0, symbol: "line.3.horizontal.decrease", editsQuery: { $0 }) }
        live.hint = state.hint
        live.completion = state.completion.map { label in PaletteQueryCompletion(label: label) { $0 } }
        let rows = state.rows
        var page = PalettePageSpec(id: "agentChats", title: "Agent Chats", placeholder: "Search chats", symbol: "bubble.left.and.bubble.right",
                                   providers: [], initialQuery: state.query,
                                   queryResults: { _ in
                                       let (stream, continuation) = AsyncStream.makeStream(of: [PaletteItem].self)
                                       continuation.yield(rows)
                                       continuation.finish()
                                       return stream
                                   }, filtersByQuery: false)
        page.live = live
        return page
    }

    private static func themeInput(dark: Bool) -> ThemeInput {
        dark ? ThemeInput(background: ThemeRGB(hex: 0x1E1E1E), foreground: ThemeRGB(hex: 0xD4D4D4))
            : ThemeInput(background: ThemeRGB(hex: 0xFFFFFF), foreground: ThemeRGB(hex: 0x24292F))
    }

    private static func refresh(_ view: NSView) {
        view.viewDidChangeEffectiveAppearance()
        view.needsLayout = true
        view.needsDisplay = true
        view.subviews.forEach(refresh)
    }

    private static func render(_ state: State, dark: Bool, windows: inout [NSWindow], scopes: inout [ThemeScope]) async throws -> Data {
        let appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
        let palette = PaletteController(registry: ActionRegistry.standard(), sources: PaletteSources(), frecencyPersistence: nil)
        defer { palette.closeForTest() }
        palette.show(page: page(state))
        await palette.model.settle()
        let view = PaletteContentView(model: palette.model)
        view.appearance = appearance
        let window = NSWindow(contentRect: view.frame, styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.appearance = appearance
        window.contentView = view
        windows.append(window)
        let input = Self.themeInput(dark: dark)
        let scope = ThemeScope(level: .room)
        scope.setOverride(ThemeSpec(dark ? "Dark+" : "GitHub Light"), input: input, animated: false)
        scope.root(view)
        scopes.append(scope)
        Self.refresh(view)
        view.layoutSubtreeIfNeeded()
        view.displayIfNeeded()
        let content = view.glass.contentView
        content.layoutSubtreeIfNeeded()
        let bounds = content.bounds.width > 0 ? content.bounds : view.bounds
        let rep = try #require(content.bitmapImageRepForCachingDisplay(in: bounds))
        content.cacheDisplay(in: bounds, to: rep)
        let image = NSImage(size: bounds.size)
        image.lockFocus()
        // The palette's opaque surface for this theme stands in for the glass (which cacheDisplay does not draw).
        ThemeTokens.derive(from: input).opaqueOverlayFill(lift: ChromeTunables.opaqueOverlayLift.value).nsColor.setFill()
        NSRect(origin: .zero, size: bounds.size).fill()
        rep.draw(in: NSRect(origin: .zero, size: bounds.size), from: .zero, operation: .sourceOver, fraction: 1,
                 respectFlipped: true, hints: nil)
        image.unlockFocus()
        let tiff = try #require(image.tiffRepresentation)
        return try #require(NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]))
    }

    @Test func drawsEachStateLightAndDark() async throws {
        _ = NSApplication.shared
        var windows: [NSWindow] = []
        var scopes: [ThemeScope] = []
        defer { windows.forEach { $0.close() } }
        let package = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let fallback = package.appendingPathComponent(".build/palette-chats").path
        let directory = ProcessInfo.processInfo.environment["CMUX_PALETTE_CHATS_SNAPSHOT_DIR"].flatMap { $0.isEmpty ? nil : $0 } ?? fallback
        try FileManager.default.createDirectory(atPath: directory, withIntermediateDirectories: true)
        for state in Self.states {
            for dark in [false, true] {
                let png = try await Self.render(state, dark: dark, windows: &windows, scopes: &scopes)
                #expect(png.count > 1000)
                try png.write(to: URL(fileURLWithPath: directory).appendingPathComponent("palette-\(state.name)-\(dark ? "dark" : "light").png"))
            }
        }
    }
}
