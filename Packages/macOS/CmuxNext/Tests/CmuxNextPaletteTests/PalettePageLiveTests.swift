@testable import CmuxNextPalette
import Testing

/// A page that answers its own queries changes what the field shows as it answers
/// (`PalettePageLive`): chips of the query that a click takes out of the query, a Tab completion,
/// and a filter switch Cmd-[ and Cmd-] move (Leo, 2026-11-18: chat search with operators, "like
/// VS Code ... likewise with Obsidian", and recency first).
@MainActor
@Suite
struct PalettePageLiveTests {
    private static func page(query: String, live: PalettePageLive) -> PalettePageSpec {
        var page = PalettePageSpec(id: "live", title: "Live", placeholder: "Search", providers: [], initialQuery: query,
                                   queryResults: { _ in
                                       let (stream, continuation) = AsyncStream.makeStream(of: [PaletteItem].self)
                                       continuation.yield([])
                                       continuation.finish()
                                       return stream
                                   }, filtersByQuery: false)
        page.live = live
        return page
    }

    @Test func aQueryChipClickTakesItsPartOutOfTheQuery() async {
        let live = PalettePageLive()
        live.tokens = [PaletteToken(id: "from", title: "From me", symbol: "person", editsQuery: { query in
            query.replacingOccurrences(of: "from:me ", with: "")
        })]
        let model = PaletteModel(persistence: nil)
        defer { model.send(.close) }
        model.reset(to: Self.page(query: "from:me parser", live: live))
        await model.settle()
        #expect(model.shownTokens.map(\.title) == ["From me"], "the live chips show in the field")
        model.removeToken(at: 0)
        #expect(model.query == "parser", "one click edits the query; the page stays")
        #expect(model.currentPageID == "live")
    }

    @Test func tabTakesTheCompletionOnOffer() async {
        let live = PalettePageLive()
        live.completion = PaletteQueryCompletion(label: "from:") { $0 + "om:" }
        let model = PaletteModel(persistence: nil)
        defer { model.send(.close) }
        model.reset(to: Self.page(query: "fr", live: live))
        await model.settle()
        #expect(model.handle(.openActions))
        #expect(model.query == "from:")
        #expect(model.actionsMenu == nil, "Tab took the completion, not the Actions menu")
    }

    @Test func theFilterSwitchStepsAndWraps() async {
        let live = PalettePageLive()
        var picked: [String] = []
        let options = ["this", "recent", "all"].map { PaletteFilterOption(id: $0, title: $0) }
        live.filters = PaletteFilterSwitch(options: options, selected: "recent") { picked.append($0) }
        let model = PaletteModel(persistence: nil)
        defer { model.send(.close) }
        model.reset(to: Self.page(query: "", live: live))
        await model.settle()
        #expect(model.stepFilter(1))
        #expect(model.stepFilter(-1))
        #expect(picked == ["all", "this"])
        let switcher = PaletteFilterSwitch(options: options, selected: "all") { _ in }
        #expect(switcher.option(stepping: 1)?.id == "this", "Cmd-] past the last wraps to the first")
        #expect(switcher.option(stepping: -1)?.id == "recent")
    }

    @Test func aPageWithoutLiveChromeKeepsItsChosenValues() async {
        let model = PaletteModel(persistence: nil)
        defer { model.send(.close) }
        var page = PalettePageSpec(id: "plain", title: "Plain", placeholder: "", providers: [])
        page.tokens = [PaletteToken(id: "a", title: "Aziz", remove: { PalettePageSpec(id: "plain", title: "Plain", placeholder: "", providers: []) })]
        model.reset(to: page)
        await model.settle()
        #expect(model.shownTokens.map(\.title) == ["Aziz"])
        #expect(!model.stepFilter(1), "no switch: Cmd-] is not the page's")
        #expect(model.pageLive == nil)
    }
}
