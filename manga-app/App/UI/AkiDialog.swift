import SwiftUI

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

    /// A yes or no for something that cannot be undone.
    func akiConfirm(_ title: String, message: String, confirm: String, isPresented: Binding<Bool>, onConfirm: @escaping () -> Void) -> some View {
        alert(title, isPresented: isPresented) {
            Button(confirm, role: .destructive, action: onConfirm)
            Button("Cancel", role: .cancel) {}
        } message: { Text(message) }
    }

    /// A sheet that rises over the screen, on glass.
    func akiSheet<Sheet: View>(isPresented: Binding<Bool>, @ViewBuilder content: @escaping () -> Sheet) -> some View {
        sheet(isPresented: isPresented) {
            content().presentationDetents([.medium, .large]).presentationDragIndicator(.visible).presentationBackground(.regularMaterial)
        }
    }

    /// A screen that covers everything: the reader.
    func akiFullScreen<Item: Identifiable, Cover: View>(item: Binding<Item?>, onDismiss: @escaping () -> Void = {}, @ViewBuilder content: @escaping (Item) -> Cover) -> some View {
        fullScreenCover(item: item, onDismiss: onDismiss, content: content)
    }
}
