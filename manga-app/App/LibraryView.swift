import AkiManga
import SwiftUI

struct LibraryView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        NavigationStack {
            Group {
                if model.library.isEmpty {
                    StateCard(title: "Nothing here yet", message: "Save a series from Browse and it stays here, with how far you have read.")
                } else {
                    ScrollView {
                        LazyVGrid(columns: gridColumns, spacing: 18) {
                            ForEach(model.library, id: \.key) { item in
                                NavigationLink(value: SeriesRef(source: item.source, id: item.id, title: item.title, cover: item.cover)) {
                                    CoverTile(title: item.title, cover: item.cover, badge: Int(item.unread))
                                }.buttonStyle(.plain)
                            }
                        }.padding(16)
                    }
                }
            }
            .background(Aki.canvas)
            .navigationTitle("Library")
            .navigationDestination(for: SeriesRef.self) { SeriesView(ref: $0) }
        }
        .onAppear { model.refresh() }
    }
}

extension LibraryEntry {
    var key: String { "\(source)/\(id)" }
}
