import AkiManga
import SwiftUI
import UniformTypeIdentifiers

enum SettingsRoute: Hashable { case root }

/// Everything the person can change about the app, and where their data goes in and out.
struct SettingsView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    @State private var backupFile: URL?
    @State private var restoring = false
    @State private var importing = false
    @State private var message: (title: String, message: String)?
    @State private var downloaded: UInt64 = 0
    @State private var categories: [CategoryInfo] = []

    /// Turning alerts on asks iOS for permission; if the person said no, the switch stays off and says where to change it.
    private var alerts: Binding<Bool> {
        Binding(
            get: { model.prefs.notifyUpdates },
            set: { on in
                guard on else { return model.prefs.notifyUpdates = false }
                Task {
                    let granted = await UpdateAlerts.requestPermission()
                    model.prefs.notifyUpdates = granted
                    if !granted { message = ("Notifications are off", "Turn them on for Akidev in the iOS Settings app, then try again.") }
                }
            }
        )
    }

    private var languages: [String] { Array(Set(model.sources.flatMap(\.languages))).sorted() }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                AkiBackHeader(title: "Settings", back: { dismiss() })
                if model.prefs.readerKind == .books {
                    readerSection
                    booksSection
                    readAloudSection
                    appearanceSection
                    dataSection
                    mangaGroup
                    aboutSection
                } else {
                    appearanceSection
                    readerSection
                    contentSection
                    librarySection
                    mangaReadingSection
                    booksSection
                    readAloudSection
                    updatesSection
                    downloadsSection
                    dataSection
                    aboutSection
                }
            }
            .akiPage(narrow: true)
        }
        .background(AkiBackdrop())
        .akiNoNavigationBar()
        .hidesTabBar()
        .akiFileImporter(isPresented: $restoring, types: [.json]) { url in
            guard let text = try? String(contentsOf: url, encoding: .utf8) else { return }
            Task { await restore(text) }
        }
        .akiFileImporter(isPresented: $importing, types: [UTType(filenameExtension: "cbz") ?? .zip, .zip]) { url in importComic(url) }
        .akiAlert(message?.title ?? "", message: message?.message, isPresented: Binding(get: { message != nil }, set: { if !$0 { message = nil } }))
        .task { await model.readAloud.check() }
        .onAppear {
            downloaded = ((try? model.engine.downloads()) ?? []).reduce(0) { $0 + $1.bytes }
            categories = (try? model.engine.categories()) ?? []
        }
    }

    @ViewBuilder private var appearanceSection: some View {
        @Bindable var prefs = model.prefs
                AkiSection(title: "Appearance") {
                    AkiCard(padding: 16) { AkiSelect(title: "Theme", selection: $prefs.appearance, options: Appearance.allCases.map { ($0, $0.title) }) }
                }
    }

    @ViewBuilder private var readerSection: some View {
        @Bindable var prefs = model.prefs
                AkiSection(title: "Reader", footer: "Manga and books are both always here. This decides which comes first: the order of the tabs, and the one the app opens on.") {
                    AkiCard(padding: 16) {
                        AkiSelect(title: "I mostly read", selection: Binding(get: { prefs.readerKind ?? .manga }, set: { model.prefs.readerKind = $0 }), options: ReaderKind.allCases.map { ($0, $0.title) })
                    }
                }
    }

    @ViewBuilder private var contentSection: some View {
        @Bindable var prefs = model.prefs
                AkiSection(title: "Content", footer: "Sources marked 18+ are not available in this app.") {
                    AkiCard(padding: 16) {
                        VStack(spacing: 16) {
                            if !languages.isEmpty {
                                AkiSelect(title: "Language", selection: Binding(get: { prefs.languages.first ?? "" }, set: { prefs.languages = $0.isEmpty ? [] : [$0] }),
                                          options: [("", "All")] + languages.map { ($0, $0.uppercased()) })
                            }
                        }
                    }
                }
    }

    @ViewBuilder private var librarySection: some View {
        @Bindable var prefs = model.prefs
                AkiSection(title: "Library", footer: "Choose what happens to a series when you add it to your library. Create categories from the Library menu.") {
                    AkiCard(padding: 16) {
                        AkiSelect(title: "When adding to library", selection: Binding(get: { categories.contains { $0.id == prefs.saveCategory } || prefs.saveCategory == -1 ? prefs.saveCategory : 0 }, set: { prefs.saveCategory = $0 }), options: [(Int64(0), "No category"), (Int64(-1), "Ask each time")] + categories.map { ($0.id, "Put in \($0.name)") })
                    }
                }
    }

    @ViewBuilder private var mangaReadingSection: some View {
        @Bindable var prefs = model.prefs
                AkiSection(title: "Reading", footer: "A series keeps its own mode once you change it in the reader.") {
                    AkiCard(padding: 16) {
                        VStack(spacing: 16) {
                            AkiSelect(title: "Default mode", selection: $prefs.defaultMode, options: ReadingMode.allCases.map { ($0, $0.title) })
                            AkiSwitch(title: "Left edge goes forward", subtitle: "In paged reading, tap the left edge for the next page.", isOn: $prefs.invertTaps)
                            AkiSwitch(title: "Keep the screen on", isOn: $prefs.keepAwake)
                        }
                    }
                }
    }

    @ViewBuilder private var readAloudSection: some View {
        @Bindable var prefs = model.prefs
                if model.readAloud.reachable {
                    AkiSection(title: "Reading aloud", footer: "Books are read in a voice that runs on this phone. Download a voice model once. Reading aloud then needs no network.") {
                        AkiLink(value: VoicesRoute()) {
                            AkiRow(title: "Voice model", subtitle: model.readAloud.current?.name ?? "None yet. Choose one to download.", icon: "headphones", showsChevron: true)
                        }
                    }
                }
    }

    @ViewBuilder private var updatesSection: some View {
        @Bindable var prefs = model.prefs
                AkiSection(title: "Updates", footer: "Akidev checks the series in your library in the background when iOS allows it, usually a few times a day and less if you rarely open the app. The check asks your sources for new chapters and sends nothing else. The alert is made on this device.") {
                    AkiCard(padding: 16) {
                        VStack(spacing: 16) {
                            AkiSwitch(title: "New chapter alerts", subtitle: "A notification when a series in your library gets a chapter.", isOn: alerts)
                            AkiSwitch(title: "Download new chapters", subtitle: "Keeps them on this device to read offline.", isOn: $prefs.autoDownload)
                            if prefs.autoDownload {
                                AkiSwitch(title: "Download only on Wi-Fi", isOn: $prefs.downloadOnWifiOnly)
                            }
                        }
                    }
                }
    }

    @ViewBuilder private var downloadsSection: some View {
        @Bindable var prefs = model.prefs
                AkiSection(title: "Downloads") {
                    AkiCard(padding: 16) {
                        VStack(spacing: 16) {
                            HStack {
                                AkiText("Kept on this device", .bodyStrong)
                                Spacer()
                                AkiText(bookSize(downloaded), .body, tone: .muted, digits: true)
                            }
                            AkiButton("Clear image cache", icon: "trash", fullWidth: true) { Task { await ImageLoader.shared.clear(); akiAnnounce("Image cache cleared") } }
                        }
                    }
                }
    }

    @ViewBuilder private var dataSection: some View {
        @Bindable var prefs = model.prefs
                AkiSection(title: "Your data", footer: "A backup holds your library, what you have read, categories, repositories and source settings. Sources come back from their repositories.") {
                    VStack(spacing: 12) {
                        if let backupFile {
                            AkiShare(title: "Share backup", icon: "share", file: backupFile)
                        } else {
                            AkiButton("Create backup", icon: "server", fullWidth: true) { makeBackup() }
                        }
                        AkiButton("Restore from backup", icon: "restore", fullWidth: true) { restoring = true }
                        AkiButton("Open a CBZ comic file", icon: "zip", fullWidth: true) { importing = true }
                    }
                }
    }

    @ViewBuilder private var aboutSection: some View {
        @Bindable var prefs = model.prefs
                AkiSection(title: "About") {
                    AkiCard(padding: 16) {
                        VStack(alignment: .leading, spacing: 8) {
                            HStack(spacing: 12) {
                                AkiLogo(size: 44)
                                AkiText("Akidev \(model.engine.version())", .bodyStrong, digits: true)
                            }
                            AkiText("Icons: Solar by 480 Design, CC BY 4.0.", .caption, tone: .muted)
                            AkiText("A reader with no content of its own. Sources come from repositories you add. The people who make them, and the sites they read, are not affiliated with Akidev.", .caption, tone: .muted)
                        }
                    }
                }
    }

    /// How books look and read before one is opened; the reader's own sheet changes the same things for the book in hand.
    @ViewBuilder private var booksSection: some View {
        @Bindable var prefs = model.prefs
        AkiSection(title: "Books", footer: "How a book opens. Once you are in a book, its Type and theme sheet changes these for you as you read.") {
            AkiCard(padding: 16) {
                VStack(spacing: 16) {
                    AkiSelect(title: "Reading", selection: $prefs.bookStyle.scroll, options: [(false, "Pages"), (true, "Scroll")])
                    AkiSelect(title: "Theme", selection: $prefs.bookStyle.theme, options: BookTheme.allCases.map { ($0, $0.title) })
                    AkiSelect(title: "Font", selection: $prefs.bookStyle.font, options: BookFont.allCases.map { ($0, $0.title) })
                    AkiSwitch(title: "Keep the screen on", isOn: $prefs.keepAwake)
                }
            }
        }
    }

    /// Everything that is about manga, for someone who reads books first: still all here, after what is about books.
    @ViewBuilder private var mangaGroup: some View {
        AkiText("Manga", .title).accessibilityAddTraits(.isHeader).padding(.top, 8)
        contentSection
        librarySection
        mangaReadingSection
        updatesSection
        downloadsSection
    }

    private func makeBackup() {
        do {
            let text = try model.engine.backup()
            let file = FileManager.default.temporaryDirectory.appendingPathComponent("AkiManga-\(Date.now.formatted(.iso8601.year().month().day())).json")
            try text.write(to: file, atomically: true, encoding: .utf8)
            backupFile = file
        } catch { message = describe(error, "Could not make the backup") }
    }

    private func restore(_ text: String) async {
        do {
            let report = try await model.engine.restore(text: text)
            model.refresh()
            let missing = report.sourcesMissing.isEmpty ? "" : " Could not bring back: \(report.sourcesMissing.joined(separator: ", "))."
            message = ("Backup restored", "\(report.series) series and \(plural(report.sourcesInstalled.count, "source", "sources")) restored.\(missing)")
        } catch { message = describe(error, "Could not restore the backup") }
    }

    private func importComic(_ url: URL) {
        do {
            _ = try model.engine.importFile(path: url.path)
            model.refresh()
            message = ("Added to your library", "Find it in your library.")
        } catch { message = describe(error, "Could not add this file") }
    }
}
