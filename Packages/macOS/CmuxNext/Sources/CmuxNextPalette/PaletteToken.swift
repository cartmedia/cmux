/// One chosen value of a multi-value step, drawn as a chip in the search field (New Group Chat's
/// members). A click on it, or Backspace on an empty query when it is the last one, shows
/// `remove()`: the same page without it. A chip of what a page's source read in the query (a
/// search operator, `PalettePageLive.tokens`) edits the query instead (``editsQuery``).
public struct PaletteToken {
    /// Stable value (a participant id or typed address).
    public let id: String
    /// The chip's text.
    public let title: String
    /// SF Symbol drawn before the title.
    public let symbol: String
    /// The page with this value removed.
    public let remove: @MainActor () -> PalettePageSpec
    /// The query without this chip's part of it; nil: the chip is a page's value (``remove``).
    public let editsQuery: (@MainActor (String) -> String)?

    public init(id: String, title: String, symbol: String = "person.fill", remove: @escaping @MainActor () -> PalettePageSpec) {
        self.id = id
        self.title = title
        self.symbol = symbol
        self.remove = remove
        editsQuery = nil
    }

    /// A chip over part of the query: a click gives the query `edit` returns (that part taken out).
    public init(id: String, title: String, symbol: String, editsQuery edit: @escaping @MainActor (String) -> String) {
        self.id = id
        self.title = title
        self.symbol = symbol
        // Never shown as a page: a query chip is not one of the page's values.
        remove = { PalettePageSpec(id: "token.query", title: title, placeholder: "", providers: []) }
        editsQuery = edit
    }
}
