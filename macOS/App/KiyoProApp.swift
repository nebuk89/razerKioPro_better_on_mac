import SwiftUI

@main
struct KiyoProApp: App {
    @StateObject private var model = AppModel()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(model)
                .frame(minWidth: 720, minHeight: 680)
        }
        .windowStyle(.titleBar)
    }
}
