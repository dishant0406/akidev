import AkiManga
import SwiftUI

struct SourcesView: View {
    @Environment(AppModel.self) private var model
    @State private var adding = false
    @State private var link = ""
    @State private var problem: String?

    var body: some View {
        NavigationStack {
            List {
                Section {
                    ForEach(model.sources, id: \.id) { source in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(source.name).foregroundStyle(Aki.text)
                            Text("\(source.languages.joined(separator: ", ").uppercased()) · v\(source.version)\(source.nsfw ? " · 18+" : "")")
                                .font(.caption).foregroundStyle(Aki.faint)
                        }
                        .swipeActions {
                            if source.id != "mangadex" {
                                Button("Remove", role: .destructive) { try? model.engine.removeSource(id: source.id); model.refresh() }
                            }
                        }
                    }
                } footer: {
                    Text("Aki Manga ships with MangaDex. Other sources are added by their address, from a list you trust: the app does not offer or endorse any.")
                }
                if let note = model.note { Section { Text(note).font(.footnote).foregroundStyle(Aki.dim) } }
            }
            .scrollContentBackground(.hidden)
            .background(Aki.canvas)
            .navigationTitle("Sources")
            .toolbar { ToolbarItem(placement: .topBarTrailing) { Button("Add", systemImage: "plus") { adding = true } } }
            .alert("Add a source", isPresented: $adding) {
                TextField("https://…/source.json", text: $link).textInputAutocapitalization(.never).autocorrectionDisabled()
                Button("Add") { Task { await add() } }
                Button("Cancel", role: .cancel) { link = "" }
            } message: { Text("The address of a source definition.") }
            .alert("That did not work", isPresented: Binding(get: { problem != nil }, set: { if !$0 { problem = nil } })) {
                Button("OK", role: .cancel) {}
            } message: { Text(problem ?? "") }
        }
    }

    private func add() async {
        defer { link = "" }
        do { try await model.install(from: link) } catch { problem = describe(error).message }
    }
}
