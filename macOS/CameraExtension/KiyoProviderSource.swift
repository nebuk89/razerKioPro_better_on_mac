import CoreMediaIO
import Foundation

final class KiyoProviderSource: NSObject, CMIOExtensionProviderSource {
    private(set) var provider: CMIOExtensionProvider!
    private let deviceSource: KiyoDeviceSource

    init(clientQueue: DispatchQueue?) {
        deviceSource = KiyoDeviceSource(localizedName: KiyoConstants.virtualCameraName)
        super.init()
        provider = CMIOExtensionProvider(source: self, clientQueue: clientQueue)
        do {
            try provider.addDevice(deviceSource.device)
        } catch {
            fatalError("Unable to publish Kiyo virtual camera: \(error.localizedDescription)")
        }
    }

    func connect(to client: CMIOExtensionClient) throws {}
    func disconnect(from client: CMIOExtensionClient) {}

    var availableProperties: Set<CMIOExtensionProperty> {
        [.providerName, .providerManufacturer]
    }

    func providerProperties(
        forProperties properties: Set<CMIOExtensionProperty>
    ) throws -> CMIOExtensionProviderProperties {
        let result = CMIOExtensionProviderProperties(dictionary: [:])
        if properties.contains(.providerName) {
            result.name = "Kiyo Pro 1080p Provider"
        }
        if properties.contains(.providerManufacturer) {
            result.manufacturer = "Kiyo Pro Control"
        }
        return result
    }
    func setProviderProperties(_ providerProperties: CMIOExtensionProviderProperties) throws {}
}
}
