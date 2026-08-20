import Foundation

@MainActor
final class CameraSettingsModel: ObservableObject {
    @Published var brightness = 128.0
    @Published var contrast = 128.0
    @Published var saturation = 128.0
    @Published var sharpness = 128.0
    @Published var whiteBalance = 5000.0
    @Published var exposureTime = 156.0
    @Published var focus = 0.0
    @Published var zoom = 100.0
    @Published var whiteBalanceAuto = true
    @Published var focusAuto = true
    @Published var hdr = "off"
    @Published var hdrMode = "bright"
    @Published var fieldOfView = "wide"
    @Published var autofocusMode = "responsive"
    @Published var status = "Connect the Razer Kiyo Pro to load its controls."
    @Published var isBusy = false

    private let commandQueue = DispatchQueue(label: "com.nebuk89.kiyo.controls")

    func load() {
        isBusy = true
        status = "Reading controls from the Kiyo Pro…"
        let keys = [
            "brightness", "contrast", "saturation", "sharpness", "white_balance",
            "exposure_time", "focus", "zoom", "white_balance_auto", "focus_auto",
        ]
        commandQueue.async {
            var values: [String: String] = [:]
            do {
                for key in keys {
                    values[key] = try KiyoHelper.run(["get", key])
                }
                Task { @MainActor in
                    self.apply(values)
                    self.status = "Live settings loaded. Slider changes apply when you release them."
                    self.isBusy = false
                }
            } catch {
                Task { @MainActor in
                    self.status = error.localizedDescription
                    self.isBusy = false
                }
            }
        }
    }

    func set(_ key: String, _ value: String) {
        isBusy = true
        status = "Applying \(key)…"
        commandQueue.async {
            do {
                _ = try KiyoHelper.run(["set", "\(key)=\(value)"])
                Task { @MainActor in
                    self.status = "\(key) saved and applied."
                    self.isBusy = false
                }
            } catch {
                Task { @MainActor in
                    self.status = error.localizedDescription
                    self.isBusy = false
                }
            }
        }
    }

    func installReconnectService() {
        isBusy = true
        commandQueue.async {
            do {
                _ = try KiyoHelper.run(["install"])
                Task { @MainActor in
                    self.status = "Reconnect service installed. Saved settings will return automatically."
                    self.isBusy = false
                }
            } catch {
                Task { @MainActor in
                    self.status = error.localizedDescription
                    self.isBusy = false
                }
            }
        }
    }

    private func apply(_ values: [String: String]) {
        brightness = Double(values["brightness"] ?? "") ?? brightness
        contrast = Double(values["contrast"] ?? "") ?? contrast
        saturation = Double(values["saturation"] ?? "") ?? saturation
        sharpness = Double(values["sharpness"] ?? "") ?? sharpness
        whiteBalance = Double(values["white_balance"] ?? "") ?? whiteBalance
        exposureTime = Double(values["exposure_time"] ?? "") ?? exposureTime
        focus = Double(values["focus"] ?? "") ?? focus
        zoom = Double(values["zoom"] ?? "") ?? zoom
        whiteBalanceAuto = values["white_balance_auto"] == "on"
        focusAuto = values["focus_auto"] == "on"
    }
}

private enum KiyoHelper {
    static func run(_ arguments: [String]) throws -> String {
        let executable = Bundle.main.bundleURL
            .appendingPathComponent("Contents/Helpers/kiyo")
        guard FileManager.default.isExecutableFile(atPath: executable.path) else {
            throw HelperError("The bundled Kiyo control helper is missing.")
        }

        let process = Process()
        let output = Pipe()
        let errors = Pipe()
        process.executableURL = executable
        process.arguments = arguments
        process.standardOutput = output
        process.standardError = errors
        try process.run()
        process.waitUntilExit()

        let stdout = String(
            data: output.fileHandleForReading.readDataToEndOfFile(),
            encoding: .utf8
        )?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let stderr = String(
            data: errors.fileHandleForReading.readDataToEndOfFile(),
            encoding: .utf8
        )?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        guard process.terminationStatus == 0 else {
            throw HelperError(stderr.isEmpty ? "Kiyo control failed." : stderr)
        }
        return stdout
    }
}

private struct HelperError: LocalizedError {
    let message: String
    init(_ message: String) { self.message = message }
    var errorDescription: String? { message }
}
