public import Foundation
public import Observation

/// One choice of a page's filter switch (the chats page's This chat · Recent · All).
public struct PaletteFilterOption: Equatable, Sendable {
    public let id: String
    public let title: String

    public init(id: String, title: String) {
        self.id = id
        self.title = title
    }
}

/// A page's filter switch under the field: one option on at a time, picked with a click or
/// Cmd-[ and Cmd-] (the palette's other scope switches, like Cursor's search filters).
public struct PaletteFilterSwitch {
    public let options: [PaletteFilterOption]
    public let selected: String
    /// Picks option `id`: the page shows its rows for the shown query.
    public let select: @MainActor (String) -> Void

    public init(options: [PaletteFilterOption], selected: String, select: @escaping @MainActor (String) -> Void) {
        self.options = options
        self.selected = selected
        self.select = select
    }

    /// The option `step` places from the selected one (Cmd-] is +1), wrapping at the ends.
    public func option(stepping step: Int) -> PaletteFilterOption? {
        guard !options.isEmpty else { return nil }
        let index = options.firstIndex { $0.id == selected } ?? 0
        let count = options.count
        return options[((index + step) % count + count) % count]
    }
}

/// A suggestion for the query that Tab accepts (the chats page's search operators): its label is
/// drawn at the field's end with a Tab keycap, and `apply` gives the query with it in place.
public struct PaletteQueryCompletion {
    public let label: String
    public let apply: @MainActor (String) -> String

    public init(label: String, apply: @escaping @MainActor (String) -> String) {
        self.label = label
        self.apply = apply
    }
}

/// What a page that answers its own queries shows around the field and changes as it answers
/// (`PalettePageSpec.live`): the chips of what its source read in the query, the hint line, a
/// filter switch and the Tab completion. The page writes it; the palette draws it while the page is
/// on top, in place of the spec's own `tokens` and `hint`.
@MainActor @Observable
public final class PalettePageLive {
    /// Chips before the query; with ``PaletteToken/editsQuery`` a click edits the query.
    public var tokens: [PaletteToken] = []
    /// The line under the field; nil shows the spec's `hint`.
    public var hint: String?
    public var filters: PaletteFilterSwitch?
    public var completion: PaletteQueryCompletion?

    public init() {}
}
