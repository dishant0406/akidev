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
                    if !granted { message = (Copy.Settings.notificationsOffTitle, Copy.Settings.notificationsOffMessageIos) }
                }
            }
        )
    }

    private var languages: [String] { Array(Set(model.sources.flatMap(\.languages))).sorted() }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                AkiBackHeader(title: Copy.Settings.title, back: { dismiss() })
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
                AkiSection(title: Copy.Settings.appearance) {
                    AkiCard(padding: 16) { AkiSelect(title: Copy.Settings.theme, selection: $prefs.appearance, options: Appearance.allCases.map { ($0, $0.title) }) }
                }
    }

    @ViewBuilder private var readerSection: some View {
        @Bindable var prefs = model.prefs
                AkiSection(title: Copy.Settings.reader, footer: Copy.Settings.readerFooter) {
                    AkiCard(padding: 16) {
                        AkiSelect(title: Copy.Settings.readerKindLabel, selection: Binding(get: { prefs.readerKind ?? .manga }, set: { model.prefs.readerKind = $0 }), options: ReaderKind.allCases.map { ($0, $0.title) })
                    }
                }
    }

    @ViewBuilder private var contentSection: some View {
        @Bindable var prefs = model.prefs
                AkiSection(title: Copy.Settings.content, footer: Copy.Settings.contentFooterIos) {
                    AkiCard(padding: 16) {
                        VStack(spacing: 16) {
                            if !languages.isEmpty {
                                AkiSelect(title: Copy.Settings.language, selection: Binding(get: { prefs.languages.first ?? "" }, set: { prefs.languages = $0.isEmpty ? [] : [$0] }),
                                          options: [("", Copy.Home.all)] + languages.map { ($0, $0.uppercased()) })
                            }
                        }
                    }
                }
    }

    @ViewBuilder private var librarySection: some View {
        @Bindable var prefs = model.prefs
                AkiSection(title: Copy.Home.library, footer: Copy.Settings.libraryFooter) {
                    AkiCard(padding: 16) {
                        AkiSelect(title: Copy.Settings.whenAddingToLibrary, selection: Binding(get: { categories.contains { $0.id == prefs.saveCategory } || prefs.saveCategory == -1 ? prefs.saveCategory : 0 }, set: { prefs.saveCategory = $0 }), options: [(Int64(0), Copy.Library.noCategory), (Int64(-1), Copy.Settings.askEachTime)] + categories.map { ($0.id, Copy.Settings.putInCategory($0.name)) })
                    }
                }
    }

    @ViewBuilder private var mangaReadingSection: some View {
        @Bindable var prefs = model.prefs
                AkiSection(title: Copy.Settings.reading, footer: Copy.Settings.readingFooter) {
                    AkiCard(padding: 16) {
                        VStack(spacing: 16) {
                            AkiSelect(title: Copy.Settings.defaultMode, selection: $prefs.defaultMode, options: ReadingMode.allCases.map { ($0, $0.title) })
                            AkiSwitch(title: Copy.Settings.leftEdgeForward, subtitle: Copy.Settings.leftEdgeForwardHint, isOn: $prefs.invertTaps)
                            AkiSwitch(title: Copy.Reader.keepScreenOn, isOn: $prefs.keepAwake)
                        }
                    }
                }
    }

    @ViewBuilder private var readAloudSection: some View {
        @Bindable var prefs = model.prefs
                if model.readAloud.reachable {
                    AkiSection(title: Copy.Settings.readAloud, footer: Copy.Settings.readAloudFooter) {
                        AkiLink(value: VoicesRoute()) {
                            AkiRow(title: Copy.Settings.voiceModel, subtitle: model.readAloud.current?.name ?? Copy.Settings.noVoiceModelYet, icon: "headphones", showsChevron: true)
                        }
                    }
                }
    }

    @ViewBuilder private var updatesSection: some View {
        @Bindable var prefs = model.prefs
                AkiSection(title: Copy.Home.updates, footer: Copy.Settings.updatesFooterIos) {
                    AkiCard(padding: 16) {
                        VStack(spacing: 16) {
                            AkiSwitch(title: Copy.Settings.newChapterAlerts, subtitle: Copy.Settings.newChapterAlertsHint, isOn: alerts)
                            AkiSwitch(title: Copy.Settings.downloadNewChapters, subtitle: Copy.Settings.downloadNewChaptersHint, isOn: $prefs.autoDownload)
                            if prefs.autoDownload {
                                AkiSwitch(title: Copy.Settings.downloadOnWifiOnly, isOn: $prefs.downloadOnWifiOnly)
                            }
                        }
                    }
                }
    }

    @ViewBuilder private var downloadsSection: some View {
        @Bindable var prefs = model.prefs
                AkiSection(title: Copy.Settings.downloads) {
                    AkiCard(padding: 16) {
                        VStack(spacing: 16) {
                            HStack {
                                AkiText(Copy.Settings.keptOnThisDevice, .bodyStrong)
                                Spacer()
                                AkiText(bookSize(downloaded), .body, tone: .muted, digits: true)
                            }
                            AkiButton(Copy.Settings.clearImageCache, icon: "trash", fullWidth: true) { Task { await ImageLoader.shared.clear(); akiAnnounce(Copy.Settings.imageCacheCleared) } }
                        }
                    }
                }
    }

    @ViewBuilder private var dataSection: some View {
        @Bindable var prefs = model.prefs
                AkiSection(title: Copy.Settings.yourData, footer: Copy.Settings.yourDataFooter) {
                    VStack(spacing: 12) {
                        if let backupFile {
                            AkiShare(title: Copy.Settings.shareBackup, icon: "share", file: backupFile)
                        } else {
                            AkiButton(Copy.Settings.createBackup, icon: "server", fullWidth: true) { makeBackup() }
                        }
                        AkiButton(Copy.Settings.restoreFromBackup, icon: "restore", fullWidth: true) { restoring = true }
                        AkiButton(Copy.Settings.openCbzComic, icon: "zip", fullWidth: true) { importing = true }
                    }
                }
    }

    @ViewBuilder private var aboutSection: some View {
        @Bindable var prefs = model.prefs
                AkiSection(title: Copy.Books.about) {
                    AkiCard(padding: 16) {
                        VStack(alignment: .leading, spacing: 8) {
                            HStack(spacing: 12) {
                                AkiLogo(size: 44)
                                AkiText(Copy.Settings.akidevVersion(model.engine.version()), .bodyStrong, digits: true)
                            }
                            AkiText(Copy.Settings.aboutCreditsIos, .caption, tone: .muted)
                            AkiText(Copy.Settings.aboutDescription, .caption, tone: .muted)
                        }
                    }
                }
    }

    /// How books look and read before one is opened; the reader's own sheet changes the same things for the book in hand.
    @ViewBuilder private var booksSection: some View {
        @Bindable var prefs = model.prefs
        AkiSection(title: Copy.Home.books, footer: Copy.Settings.booksFooterIos) {
            AkiCard(padding: 16) {
                VStack(spacing: 16) {
                    AkiSelect(title: Copy.Settings.bookReadingStyle, selection: $prefs.bookStyle.scroll, options: [(false, Copy.Books.pages), (true, Copy.Books.scroll)])
                    AkiSelect(title: Copy.Settings.theme, selection: $prefs.bookStyle.theme, options: BookTheme.allCases.map { ($0, $0.title) })
                    AkiSelect(title: Copy.Books.font, selection: $prefs.bookStyle.font, options: BookFont.allCases.map { ($0, $0.title) })
                    AkiSwitch(title: Copy.Reader.keepScreenOn, isOn: $prefs.keepAwake)
                }
            }
        }
    }

    /// Everything that is about manga, for someone who reads books first: still all here, after what is about books.
    @ViewBuilder private var mangaGroup: some View {
        AkiText(Copy.Home.manga, .title).accessibilityAddTraits(.isHeader).padding(.top, 8)
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
        } catch { message = describe(error, Copy.Settings.couldNotMakeBackup) }
    }

    private func restore(_ text: String) async {
        do {
            let report = try await model.engine.restore(text: text)
            model.refresh()
            let missing = report.sourcesMissing.isEmpty ? "" : " " + Copy.Settings.couldNotBringBack(report.sourcesMissing.joined(separator: ", "))
            message = (Copy.Settings.backupRestored, Copy.Settings.restoreSummary("\(report.series)", Copy.Settings.restoredSourceCount(report.sourcesInstalled.count)) + missing)
        } catch { message = describe(error, Copy.Settings.couldNotRestoreBackup) }
    }

    private func importComic(_ url: URL) {
        do {
            _ = try model.engine.importFile(path: url.path)
            model.refresh()
            message = (Copy.Settings.addedToLibrary, Copy.Settings.findInLibrary)
        } catch { message = describe(error, Copy.Settings.couldNotAddFile) }
    }
}
