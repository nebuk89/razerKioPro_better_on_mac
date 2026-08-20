import Foundation

enum KiyoConstants {
    static let extensionBundleIdentifier = "com.nebuk89.KiyoProControl.CameraExtension"
    static let virtualCameraName = "Kiyo Pro 1080p"
    static let physicalCameraName = "Razer Kiyo Pro"
    static let frameRate: Int32 = 60
    static let width: Int32 = 1920
    static let height: Int32 = 1080

    static let deviceID = UUID(uuidString: "725A7E96-2690-4C2A-A78D-DCB9A5211424")!
    static let sourceStreamID = UUID(uuidString: "5E8243BD-09DB-4B70-B832-4169B671F0FD")!
    static let sinkStreamID = UUID(uuidString: "A1193A61-ACEC-44F0-A204-250E45734D4C")!
}
