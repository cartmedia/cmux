import AppKit
import CmuxNextDesign

/// A page's filter switch under the field (`PalettePageLive.filters`): its options as text
/// segments, the selected one on the selection fill (Cursor's search filters), and the keys that
/// move it (Cmd-[ and Cmd-]) at the trailing edge. Not in the key loop: the field keeps the keys.
final class PaletteFilterSwitchView: NSView {
    /// A click on option `id`.
    var onSelect: ((String) -> Void)?

    private var options: [PaletteFilterOption] = []
    private var selected = ""
    private var segments: [(button: PaletteClickView, label: PaletteLabel)] = []
    private let keys = PaletteKeycapsView()

    override init(frame: NSRect) {
        super.init(frame: frame)
        keys.keycaps = ["⌘[", "⌘]"]
        addSubview(keys)
        setAccessibilityElement(true)
        setAccessibilityRole(.radioGroup)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    nonisolated override var isFlipped: Bool { true }

    /// The row's height: a caption segment with its padding.
    static var height: CGFloat { ceil(Typography.caption.pointSize * 1.3) + 2 * Metrics.space2 + Metrics.space2 }

    func update(_ filters: PaletteFilterSwitch?) {
        isHidden = filters == nil
        guard let filters else { return }
        if filters.options != options {
            options = filters.options
            segments.forEach { $0.button.removeFromSuperview() }
            segments = options.map { option in
                let button = PaletteClickView()
                let label = PaletteText.label(Typography.caption, tone: .secondary)
                label.stringValue = option.title
                button.addSubview(label)
                button.onClick = { [weak self] in self?.onSelect?(option.id) }
                button.setAccessibilityElement(true)
                button.setAccessibilityRole(.radioButton)
                button.setAccessibilityLabel(option.title)
                button.setAccessibilityIdentifier("palette.filter.\(option.id)")
                addSubview(button)
                return (button, label)
            }
            selected = ""
            needsLayout = true
        }
        guard filters.selected != selected else { return }
        selected = filters.selected
        for (option, segment) in zip(options, segments) {
            let isOn = option.id == selected
            segment.button.fillsBackground = isOn
            segment.label.tone = isOn ? .primary : .secondary
            segment.button.setAccessibilityValue(isOn)
        }
    }

    override func layout() {
        super.layout()
        var x = PaletteLayout.horizontalPadding - Metrics.space4
        let padding = Metrics.space4
        for segment in segments {
            let labelWidth = PaletteText.fittingWidth(segment.label)
            let labelHeight = PaletteText.height(segment.label)
            let height = labelHeight + 2 * Metrics.space2
            segment.button.frame = pixelAligned(NSRect(x: x, y: (bounds.height - height) / 2, width: labelWidth + 2 * padding, height: height))
            segment.label.frame = segment.button.pixelAligned(NSRect(x: padding, y: Metrics.space2, width: labelWidth, height: labelHeight))
            x = segment.button.frame.maxX + Metrics.space2
        }
        let keySize = keys.intrinsicContentSize
        keys.frame = pixelAligned(NSRect(x: bounds.maxX - PaletteLayout.trailingInset - keySize.width,
                                         y: (bounds.height - keySize.height) / 2, width: keySize.width, height: keySize.height))
        keys.isHidden = keys.frame.minX < x
    }
}
