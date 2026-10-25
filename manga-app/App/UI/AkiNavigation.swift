import SwiftUI

/// A tap that pushes a screen for a value.
struct AkiLink<Value: Hashable, Label: View>: View {
    let value: Value
    @ViewBuilder var label: () -> Label

    var body: some View {
        NavigationLink(value: value, label: label).buttonStyle(AkiPressStyle())
    }
}

/// A tap that does something: wraps any content (a card, a cover) as a button without the default look.
struct AkiTap<Label: View>: View {
    let action: () -> Void
    @ViewBuilder var label: () -> Label

    var body: some View {
        Button(action: action, label: label).buttonStyle(AkiPressStyle())
    }
}

extension View {
    /// The screen a pushed value opens.
    func akiDestination<Value: Hashable, Destination: View>(for type: Value.Type, @ViewBuilder _ destination: @escaping (Value) -> Destination) -> some View {
        navigationDestination(for: type, destination: destination)
    }

    /// The tab bar floats above content: scrolling content leaves room for it.
    func underTabBar() -> some View {
        contentMargins(.bottom, 104, for: .scrollContent)
            .scrollEdgeEffectStyle(.soft, for: .top)
            .scrollEdgeEffectStyle(.soft, for: .bottom)
    }

    /// A screen's content: the side margins and top room every screen has, and on a wide screen (an iPad) a column in the middle instead of
    /// lines running the whole width. Lists of cards take a narrower column than a grid of covers.
    func akiPage(narrow: Bool = false) -> some View {
        padding(.horizontal, 20).padding(.top, 8)
            .frame(maxWidth: narrow ? 680 : 860)
            .frame(maxWidth: .infinity)
    }

    /// A custom header replaces the navigation bar.
    func akiNoNavigationBar() -> some View { toolbar(.hidden, for: .navigationBar) }
}

/// With a custom header the navigation bar is hidden; this keeps the swipe-from-the-edge back gesture.
extension UINavigationController: @retroactive UIGestureRecognizerDelegate {
    override open func viewDidLoad() {
        super.viewDidLoad()
        interactivePopGestureRecognizer?.delegate = self
    }

    public func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        viewControllers.count > 1
    }
}
