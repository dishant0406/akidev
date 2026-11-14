import SwiftUI

extension Binding {
    /// A yes or no for a screen's `sheet` choice: true while it is `value`, and setting it false closes whatever is open.
    @MainActor func equals<Choice: Equatable & Sendable>(_ value: Choice) -> Binding<Bool> where Value == Choice? {
        Binding<Bool>(get: { wrappedValue == value }, set: { if !$0 { wrappedValue = nil } })
    }
}

extension View {
    /// A message with one button.
    func akiAlert(_ title: String, message: String?, isPresented: Binding<Bool>) -> some View {
        alert(title, isPresented: isPresented) { Button("OK", role: .cancel) {} } message: { Text(message ?? "") }
    }

    /// A question with a text field: an address, a name.
    func akiPrompt(_ title: String, message: String, placeholder: String, confirm: String, text: Binding<String>, isPresented: Binding<Bool>, onConfirm: @escaping () -> Void) -> some View {
        alert(title, isPresented: isPresented) {
            TextField(placeholder, text: text).textInputAutocapitalization(.never).autocorrectionDisabled()
            Button(confirm, action: onConfirm)
            Button("Cancel", role: .cancel) { text.wrappedValue = "" }
        } message: { Text(message) }
    }

    /// A question with two ways to answer, neither of them a warning: a friend's phone asks to send a book.
    func akiQuestion(_ title: String, message: String, yes: String, no: String, isPresented: Binding<Bool>, onYes: @escaping () -> Void, onNo: @escaping () -> Void) -> some View {
        alert(title, isPresented: isPresented) {
            Button(yes, action: onYes)
            Button(no, role: .cancel, action: onNo)
        } message: { Text(message) }
    }

    /// A yes or no for something that cannot be undone.
    func akiConfirm(_ title: String, message: String, confirm: String, isPresented: Binding<Bool>, onConfirm: @escaping () -> Void) -> some View {
        alert(title, isPresented: isPresented) {
            Button(confirm, role: .destructive, action: onConfirm)
            Button("Cancel", role: .cancel) {}
        } message: { Text(message) }
    }

    /// A sheet that rises over the screen, on the page colour so the cards inside it show.
    func akiSheet<Sheet: View>(isPresented: Binding<Bool>, full: Bool = false, @ViewBuilder content: @escaping () -> Sheet) -> some View {
        sheet(isPresented: isPresented) {
            content().presentationDetents(full ? [.large] : [.medium, .large]).presentationDragIndicator(.visible).presentationBackground(Aki.background)
        }
    }

    /// The same sheet for something chosen by value (an address waiting to be confirmed).
    func akiSheet<Item: Identifiable, Sheet: View>(item: Binding<Item?>, @ViewBuilder content: @escaping (Item) -> Sheet) -> some View {
        sheet(item: item) { value in
            content(value).presentationDetents([.medium, .large]).presentationDragIndicator(.visible).presentationBackground(Aki.background)
        }
    }

    /// A screen that covers everything: the reader.
    func akiFullScreen<Item: Identifiable, Cover: View>(item: Binding<Item?>, onDismiss: @escaping () -> Void = {}, @ViewBuilder content: @escaping (Item) -> Cover) -> some View {
        fullScreenCover(item: item, onDismiss: onDismiss, content: content)
    }
}
