import CoreMediaIO
import Foundation

let providerSource = KiyoProviderSource(clientQueue: nil)
CMIOExtensionProvider.startService(provider: providerSource.provider)
CFRunLoopRun()
