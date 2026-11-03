import CoreImage.CIFilterBuiltins
import SwiftUI

/// A QR code for `text`: ink-coloured modules on a white tile, so a camera reads it in light and dark mode alike.
struct AkiQRCode: View {
    let text: String
    var size: CGFloat = 240

    var body: some View {
        Group {
            if let modules = Self.modules(text) {
                Image(uiImage: modules)
                    .renderingMode(.template)
                    .resizable()
                    .interpolation(.none)
                    .scaledToFit()
                    .foregroundStyle(Aki.onPrimary)
            }
        }
        .padding(18)
        .frame(width: size, height: size)
        .background(Color.white, in: RoundedRectangle(cornerRadius: Aki.thumbRadius, style: .continuous))
    }

    /// One pixel per module, the modules opaque and the rest clear, for the view to scale up without blurring.
    private static func modules(_ text: String) -> UIImage? {
        let make = CIFilter.qrCodeGenerator()
        make.message = Data(text.utf8)
        make.correctionLevel = "M"
        let clear = CIFilter.maskToAlpha()
        let invert = CIFilter.colorInvert()
        invert.inputImage = make.outputImage
        clear.inputImage = invert.outputImage
        guard let image = clear.outputImage, let cg = CIContext().createCGImage(image, from: image.extent) else { return nil }
        return UIImage(cgImage: cg)
    }
}
