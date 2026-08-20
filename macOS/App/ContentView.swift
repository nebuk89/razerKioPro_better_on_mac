import SwiftUI

struct ContentView: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                header
                extensionCard
                controlsCard
            }
            .padding(28)
        }
        .background(Color(nsColor: .windowBackgroundColor))
        .task {
            model.settings.load()
        }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Kiyo Pro Control")
                .font(.system(size: 30, weight: .bold))
            Text("A stable 1080p virtual camera and persistent hardware controls for macOS.")
                .foregroundStyle(.secondary)
        }
    }

    private var extensionCard: some View {
        GroupBox("1080p Camera Extension") {
            VStack(alignment: .leading, spacing: 14) {
                Label(model.extensionStatus, systemImage: "video.badge.checkmark")
                    .frame(maxWidth: .infinity, alignment: .leading)
                HStack {
                    Button("Activate Camera Extension") {
                        model.activateExtension()
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(model.isActivating)

                    Button("Remove Extension") {
                        model.deactivateExtension()
                    }
                    .disabled(model.isActivating)
                }
            }
            .padding(8)
        }
    }

    private var controlsCard: some View {
        GroupBox("Camera Settings") {
            VStack(alignment: .leading, spacing: 16) {
                HStack {
                    Label(model.settings.status, systemImage: "slider.horizontal.3")
                        .foregroundStyle(.secondary)
                    Spacer()
                    if model.settings.isBusy {
                        ProgressView().controlSize(.small)
                    }
                    Button("Reload") { model.settings.load() }
                }

                Divider()
                controlSliders
                Divider()
                automaticControls
                Divider()
                razerControls

                HStack {
                    Spacer()
                    Button("Restore Settings on Login and Reconnect") {
                        model.settings.installReconnectService()
                    }
                    .buttonStyle(.borderedProminent)
                }
            }
            .padding(8)
        }
    }

    private var controlSliders: some View {
        VStack(spacing: 12) {
            settingSlider("Brightness", value: $model.settings.brightness, range: 0...255) {
                model.settings.set("brightness", "\(Int(model.settings.brightness))")
            }
            settingSlider("Contrast", value: $model.settings.contrast, range: 0...255) {
                model.settings.set("contrast", "\(Int(model.settings.contrast))")
            }
            settingSlider("Saturation", value: $model.settings.saturation, range: 0...255) {
                model.settings.set("saturation", "\(Int(model.settings.saturation))")
            }
            settingSlider("Sharpness", value: $model.settings.sharpness, range: 0...255) {
                model.settings.set("sharpness", "\(Int(model.settings.sharpness))")
            }
            settingSlider("White balance", value: $model.settings.whiteBalance, range: 2000...7500, step: 10) {
                model.settings.set("white_balance", "\(Int(model.settings.whiteBalance))")
            }
            settingSlider("Exposure", value: $model.settings.exposureTime, range: 3...2047) {
                model.settings.set("exposure_time", "\(Int(model.settings.exposureTime))")
            }
            settingSlider("Focus", value: $model.settings.focus, range: 0...600) {
                model.settings.set("focus", "\(Int(model.settings.focus))")
            }
            settingSlider("Zoom", value: $model.settings.zoom, range: 100...400) {
                model.settings.set("zoom", "\(Int(model.settings.zoom))")
            }
        }
    }

    private var automaticControls: some View {
        HStack(spacing: 28) {
            Toggle("Auto white balance", isOn: $model.settings.whiteBalanceAuto)
                .onChange(of: model.settings.whiteBalanceAuto) { value in
                    model.settings.set("white_balance_auto", value ? "on" : "off")
                }
            Toggle("Autofocus", isOn: $model.settings.focusAuto)
                .onChange(of: model.settings.focusAuto) { value in
                    model.settings.set("focus_auto", value ? "on" : "off")
                }
        }
    }

    private var razerControls: some View {
        Grid(alignment: .leading, horizontalSpacing: 18, verticalSpacing: 12) {
            pickerRow("HDR", selection: $model.settings.hdr, values: ["off", "on"]) {
                model.settings.set("hdr", model.settings.hdr)
            }
            pickerRow("HDR scene", selection: $model.settings.hdrMode, values: ["bright", "dark"]) {
                model.settings.set("hdr_mode", model.settings.hdrMode)
            }
            pickerRow("Field of view", selection: $model.settings.fieldOfView, values: ["wide", "medium", "narrow"]) {
                model.settings.set("fov", model.settings.fieldOfView)
            }
            pickerRow("AF response", selection: $model.settings.autofocusMode, values: ["responsive", "passive"]) {
                model.settings.set("autofocus_mode", model.settings.autofocusMode)
            }
        }
    }

    private func settingSlider(
        _ title: String,
        value: Binding<Double>,
        range: ClosedRange<Double>,
        step: Double = 1,
        commit: @escaping () -> Void
    ) -> some View {
        HStack {
            Text(title).frame(width: 115, alignment: .leading)
            Slider(value: value, in: range, step: step) { editing in
                if !editing { commit() }
            }
            Text("\(Int(value.wrappedValue))")
                .monospacedDigit()
                .frame(width: 52, alignment: .trailing)
        }
    }

    private func pickerRow(
        _ title: String,
        selection: Binding<String>,
        values: [String],
        commit: @escaping () -> Void
    ) -> some View {
        GridRow {
            Text(title)
            Picker(title, selection: selection) {
                ForEach(values, id: \.self) { Text($0.capitalized).tag($0) }
            }
            .labelsHidden()
            .frame(width: 170)
            Button("Apply", action: commit)
        }
    }
}
