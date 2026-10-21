// swift-tools-version: 6.0
import PackageDescription

// The Rust engine (see desktop/manga and desktop/manga-ffi) and its generated Swift bindings.
// Run manga-app/scripts/build-ffi.sh to produce Akimanga.xcframework and akimanga.swift.
let package = Package(
    name: "AkiManga",
    platforms: [.iOS("26.0")],
    products: [.library(name: "AkiManga", targets: ["AkiManga"])],
    targets: [
        .binaryTarget(name: "akimangaFFI", path: "Akimanga.xcframework"),
        .target(
            name: "AkiManga",
            dependencies: ["akimangaFFI"],
            // The generated bindings are Swift 5 code.
            swiftSettings: [.swiftLanguageMode(.v5)]
        ),
    ]
)
