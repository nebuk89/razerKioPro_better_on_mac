import Foundation

@MainActor
final class AppModel: ObservableObject {
    @Published var extensionStatus = "Ready to install the 1080p camera extension."
    @Published var isActivating = false

    let extensionManager = CameraExtensionManager()
    @Published var settings = CameraSettingsModel()

    init() {
        extensionManager.onStatusChange = { [weak self] status, isActivating in
            self?.extensionStatus = status
            self?.isActivating = isActivating
        }
    }

    func activateExtension() {
        extensionManager.activate()
    }

    func deactivateExtension() {
        extensionManager.deactivate()
    }
}
