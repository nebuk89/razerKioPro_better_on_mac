import Foundation
import SystemExtensions

@MainActor
final class CameraExtensionManager: NSObject, OSSystemExtensionRequestDelegate {
    var onStatusChange: ((String, Bool) -> Void)?
    private var activating = false

    func activate() {
        activating = true
        update("Requesting camera extension activation…", busy: true)
        let request = OSSystemExtensionRequest.activationRequest(
            forExtensionWithIdentifier: KiyoConstants.extensionBundleIdentifier,
            queue: .main
        )
        request.delegate = self
        OSSystemExtensionManager.shared.submitRequest(request)
    }

    func deactivate() {
        activating = false
        update("Requesting camera extension removal…", busy: true)
        let request = OSSystemExtensionRequest.deactivationRequest(
            forExtensionWithIdentifier: KiyoConstants.extensionBundleIdentifier,
            queue: .main
        )
        request.delegate = self
        OSSystemExtensionManager.shared.submitRequest(request)
    }

    nonisolated func request(
        _ request: OSSystemExtensionRequest,
        actionForReplacingExtension existing: OSSystemExtensionProperties,
        withExtension ext: OSSystemExtensionProperties
    ) -> OSSystemExtensionRequest.ReplacementAction {
        .replace
    }

    nonisolated func requestNeedsUserApproval(_ request: OSSystemExtensionRequest) {
        Task { @MainActor in
            self.update(
                "Approve “Kiyo Pro 1080p Camera Extension” in System Settings → General → Login Items & Extensions → Camera Extensions.",
                busy: true
            )
        }
    }

    nonisolated func request(
        _ request: OSSystemExtensionRequest,
        didFinishWithResult result: OSSystemExtensionRequest.Result
    ) {
        Task { @MainActor in
            switch result {
            case .completed:
                self.update(
                    self.activating
                        ? "Camera extension activated. Choose “Kiyo Pro 1080p” in your video app."
                        : "Camera extension removed.",
                    busy: false
                )
            case .willCompleteAfterReboot:
                self.update("Restart macOS to complete the camera extension change.", busy: false)
            @unknown default:
                self.update("Camera extension returned an unknown result.", busy: false)
            }
        }
    }

    nonisolated func request(
        _ request: OSSystemExtensionRequest,
        didFailWithError error: Error
    ) {
        Task { @MainActor in
            let nsError = error as NSError
            let message: String
            if nsError.code == OSSystemExtensionError.extensionNotFound.rawValue {
                message = "The embedded camera extension was not found."
            } else if nsError.code == OSSystemExtensionError.unsupportedParentBundleLocation.rawValue {
                message = "Move Kiyo Pro Control to /Applications, reopen it, then activate the extension."
            } else {
                message = "Camera extension failed (\(nsError.domain) \(nsError.code)): \(error.localizedDescription)"
            }
            self.update(message, busy: false)
        }
    }

    private func update(_ status: String, busy: Bool) {
        onStatusChange?(status, busy)
    }
}
