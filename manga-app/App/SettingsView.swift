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

    private var languages: [String] { Array(Set(model.sources.flatMap(\.languages))).sorted() }

    var body: some View {
        @Bindable var prefs = model.prefs
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                AkiBackHeader(title: "Settings", back: { dismiss() })
                AkiSection(title: "Appearance") {
                    AkiCard(padding: 16) { AkiSelect(title: "Theme", selection: $prefs.appearance, options: Appearance.allCases.map { ($0, $0.title) }) }
                }
                AkiSection(title: "Content", footer: "Sources marked 18+ stay out of Discover and Sources until you turn this on.") {
                    AkiCard(padding: 16) {
                        VStack(spacing: 16) {
                            AkiSwitch(title: "Show 18+ sources", isOn: $prefs.showAdult)
                            if !languages.isEmpty {
                                AkiSelect(title: "Language", selection: Binding(get: { prefs.languages.first ?? "" }, set: { prefs.languages = $0.isEmpty ? [] : [$0] }),
                                          options: [("", "All")] + languages.map { ($0, $0.uppercased()) })
                            }
                        }
                    }
                }
                AkiSection(title: "Reading", footer: "A series keeps its own mode once you change it in the reader.") {
                    AkiCard(padding: 16) {
                        VStack(spacing: 16) {
                            AkiSelect(title: "Default mode", selection: $prefs.defaultMode, options: ReadingMode.allCases.map { ($0, $0.title) })
                            AkiSwitch(title: "Swap tap sides", subtitle: "In paged reading the left edge goes forward.", isOn: $prefs.invertTaps)
                            AkiSwitch(title: "Keep the screen on", isOn: $prefs.keepAwake)
                        }
                    }
                }
                AkiSection(title: "Downloads") {
                    AkiCard(padding: 16) {
                        VStack(spacing: 16) {
                            HStack {
                                AkiText("Kept on this device", .bodyStrong)
                                Spacer()
                                AkiText(ByteCountFormatter.string(fromByteCount: Int64(downloaded), countStyle: .file), .body, tone: .muted)
                            }
                            AkiButton("Clear image cache", icon: "trash", fullWidth: true) { Task { await ImageLoader.shared.clear() } }
                        }
                    }
                }
                AkiSection(title: "Your data", footer: "A backup holds your library, what you have read, categories, repositories and source settings. Sources come back from their repositories.") {
                    VStack(spacing: 12) {
                        if let backupFile {
                            AkiShare(title: "Share the backup", icon: "share", file: backupFile)
                        } else {
                            AkiButton("Create a backup", icon: "server", fullWidth: true) { makeBackup() }
                        }
                        AkiButton("Restore from a backup", icon: "restore", fullWidth: true) { restoring = true }
                        AkiButton("Open a comic file (CBZ)", icon: "zip", fullWidth: true) { importing = true }
                    }
                }
                AkiSection(title: "About") {
                    AkiCard(padding: 16) {
                        VStack(alignment: .leading, spacing: 8) {
                            HStack(spacing: 12) {
                                AkiLogo(size: 44)
                                AkiText("Akidev \(model.engine.version())", .bodyStrong)
                            }
                            AkiText("Icons: Solar by 480 Design, CC BY 4.0.", .caption, tone: .muted)
                            AkiText("A reader with no content of its own. Sources are plugins from repositories you add; the people who make them, and the sites they read, are not affiliated with Akidev.", .caption, tone: .muted)
                        }
                    }
                }
            }
            .padding(.horizontal, 20).padding(.top, 8)
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
        .onAppear { downloaded = ((try? model.engine.downloads()) ?? []).reduce(0) { $0 + $1.bytes } }
    }

    private func makeBackup() {
        do {
            let text = try model.engine.backup()
            let file = FileManager.default.temporaryDirectory.appendingPathComponent("AkiManga-\(Date.now.formatted(.iso8601.year().month().day())).json")
            try text.write(to: file, atomically: true, encoding: .utf8)
            backupFile = file
        } catch { message = describe(error) }
    }

    private func restore(_ text: String) async {
        do {
            let report = try await model.engine.restore(text: text)
            model.refresh()
            let missing = report.sourcesMissing.isEmpty ? "" : " Could not bring back: \(report.sourcesMissing.joined(separator: ", "))."
            message = ("Restored", "\(report.series) series and \(report.sourcesInstalled.count) sources.\(missing)")
        } catch { message = describe(error) }
    }

    private func importComic(_ url: URL) {
        do {
            _ = try model.engine.importFile(path: url.path)
            model.refresh()
            message = ("Added to your library", "Find it under Saved.")
        } catch { message = describe(error) }
    }
}
