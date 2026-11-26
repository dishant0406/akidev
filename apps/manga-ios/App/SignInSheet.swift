import AkiManga
import SwiftUI

/// "Sign in to a source": the fields its definition asks for, drawn here, and where they go. Only the site's answer is kept
/// (in the Keychain); the password is sent once and forgotten.
struct SignInSheet: View {
    let source: String
    /// Called once the person is signed in, as the sheet closes.
    var onSignedIn: () -> Void = {}
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var form: SignInForm?
    @State private var values: [String: String] = [:]
    @State private var working = false
    @State private var problem: String?
    @State private var loadFailed = false

    private var ready: Bool { form.map { $0.fields.allSatisfy { !(values[$0.id] ?? "").isEmpty } } ?? false }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                HStack(spacing: 12) {
                    AkiLogo(size: 36)
                    AkiText(Copy.Sources.signInTitle(model.name(of: source)), .title).accessibilityAddTraits(.isHeader)
                }
                if let form {
                    ForEach(form.fields, id: \.id) { field in
                        AkiField(label: field.name, text: binding(field.id), secure: field.secret, email: field.id.contains("email")) { Task { await signIn() } }
                    }
                    if let problem { AkiText(problem, .caption, tone: .destructive) }
                    AkiButton(working ? Copy.Sources.signingIn : Copy.Sources.signIn, variant: .primary, size: .large, fullWidth: true) { Task { await signIn() } }
                        .disabled(working || !ready)
                    AkiText(Copy.Sources.signInDetails(form.host), .caption, tone: .muted)
                } else if loadFailed {
                    AkiText(Copy.Sources.couldNotLoadSignIn, .caption, tone: .destructive)
                        .onAppear { akiAnnounce(Copy.Sources.couldNotLoadSignIn) }
                }
                AkiButton(Copy.Sources.cancel, variant: .ghost, fullWidth: true) { dismiss() }
            }
            .padding(24)
        }
        .onAppear {
            form = try? model.engine.signInForm(source: source)
            loadFailed = form == nil
        }
        .onChange(of: problem) { if let problem { akiAnnounce(problem) } }
    }

    private func binding(_ id: String) -> Binding<String> {
        Binding(get: { values[id] ?? "" }, set: { values[id] = $0; problem = nil })
    }

    private func signIn() async {
        guard !working, ready else { return }
        working = true
        defer { working = false }
        do {
            _ = try await model.engine.signIn(source: source, fields: values)
            model.refresh()
            // Empty the password before the sheet goes: iOS offers to save a password that is still in the field.
            for field in form?.fields ?? [] where field.secret { values[field.id] = "" }
            try? await Task.sleep(for: .milliseconds(150))
            onSignedIn()
            dismiss()
        } catch { problem = describe(error).message }
    }
}
