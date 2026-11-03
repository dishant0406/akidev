import AVFoundation
import SwiftUI

/// The camera, looking for a QR code: a rounded picture that calls `found` with each code's text (the same code is not repeated
/// within two seconds). The camera is asked for when this appears; without it, the view says so and opens Settings.
struct AkiScanner: View {
    let found: (String) -> Void
    @State private var allowed: Bool?

    var body: some View {
        // A ZStack, not a Group: while the camera is being asked for there is nothing to show, and a view with no content takes no
        // space and runs no task.
        ZStack {
            if allowed == true {
                Camera(found: found)
                    .clipShape(RoundedRectangle(cornerRadius: Aki.thumbRadius, style: .continuous))
            } else if allowed == false {
                VStack(spacing: 12) {
                    AkiText("Allow the camera to scan a code", .body, tone: .muted, align: .center)
                    AkiButton("Open Settings", variant: .ghost, size: .small) {
                        if let url = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(url) }
                    }
                }
                .padding(24)
            }
        }
        .frame(maxWidth: .infinity)
        .frame(height: 200)
        .task { allowed = await Self.cameraAllowed() }
    }

    /// A phone with no camera (the Simulator) answers like a phone that was refused.
    private static func cameraAllowed() async -> Bool {
        guard AVCaptureDevice.default(for: .video) != nil else { return false }
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized: return true
        case .notDetermined: return await AVCaptureDevice.requestAccess(for: .video)
        default: return false
        }
    }
}

private struct Camera: UIViewRepresentable {
    let found: (String) -> Void

    func makeUIView(context: Context) -> Preview {
        let view = Preview()
        view.start(found)
        return view
    }

    func updateUIView(_ view: Preview, context: Context) {}

    static func dismantleUIView(_ view: Preview, coordinator: ()) { view.stop() }
}

private final class Preview: UIView, @preconcurrency AVCaptureMetadataOutputObjectsDelegate {
    override class var layerClass: AnyClass { AVCaptureVideoPreviewLayer.self }
    private let session = AVCaptureSession()
    private var found: (String) -> Void = { _ in }
    private var last: (text: String, at: Date)?

    func start(_ found: @escaping (String) -> Void) {
        self.found = found
        guard let camera = AVCaptureDevice.default(for: .video), let input = try? AVCaptureDeviceInput(device: camera), session.canAddInput(input) else { return }
        session.addInput(input)
        let output = AVCaptureMetadataOutput()
        guard session.canAddOutput(output) else { return }
        session.addOutput(output)
        output.setMetadataObjectsDelegate(self, queue: .main)
        output.metadataObjectTypes = [.qr]
        let picture = layer as? AVCaptureVideoPreviewLayer
        picture?.session = session
        picture?.videoGravity = .resizeAspectFill
        let session = session
        // Starting a camera waits for it, so it is not done on the main thread.
        DispatchQueue.global().async { session.startRunning() }
    }

    func stop() {
        let session = session
        DispatchQueue.global().async { session.stopRunning() }
    }

    func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput objects: [AVMetadataObject], from connection: AVCaptureConnection) {
        guard let text = (objects.first as? AVMetadataMachineReadableCodeObject)?.stringValue else { return }
        if let last, last.text == text, Date().timeIntervalSince(last.at) < 2 { return }
        last = (text, Date())
        found(text)
    }
}
