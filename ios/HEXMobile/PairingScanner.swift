import SwiftUI
import VisionKit

/// Scans the QR code `hex remote pair` prints and hands back its pairing link.
struct PairingScanner: UIViewControllerRepresentable {
    let onLink: (_ address: String, _ code: String) -> Void

    static var isAvailable: Bool {
        DataScannerViewController.isSupported && DataScannerViewController.isAvailable
    }

    func makeUIViewController(context: Context) -> DataScannerViewController {
        let scanner = DataScannerViewController(
            recognizedDataTypes: [.barcode(symbologies: [.qr])],
            isHighlightingEnabled: true
        )
        scanner.delegate = context.coordinator
        try? scanner.startScanning()
        return scanner
    }

    func updateUIViewController(_ scanner: DataScannerViewController, context: Context) {}

    func makeCoordinator() -> Coordinator {
        Coordinator(onLink: onLink)
    }

    final class Coordinator: NSObject, DataScannerViewControllerDelegate {
        private let onLink: (String, String) -> Void
        private var found = false

        init(onLink: @escaping (String, String) -> Void) {
            self.onLink = onLink
        }

        func dataScanner(
            _ scanner: DataScannerViewController,
            didAdd items: [RecognizedItem],
            allItems: [RecognizedItem]
        ) {
            for case .barcode(let barcode) in items {
                guard !found,
                      let payload = barcode.payloadStringValue,
                      let url = URL(string: payload),
                      let link = RemoteHexPairing.link(url)
                else { continue }
                found = true
                scanner.stopScanning()
                onLink(link.address, link.code)
            }
        }
    }
}
