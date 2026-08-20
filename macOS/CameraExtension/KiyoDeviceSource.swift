import AVFoundation
import CoreMediaIO
import Foundation
import IOKit.audio
import os

final class KiyoDeviceSource: NSObject, CMIOExtensionDeviceSource {
    private(set) var device: CMIOExtensionDevice!

    private var sourceStream: KiyoSourceStream!
    private var sourceClientCount = 0
    private let captureQueue = DispatchQueue(
        label: "com.nebuk89.kiyo.extension.capture",
        qos: .userInteractive
    )
    private var captureSession: AVCaptureSession?

    init(localizedName: String) {
        super.init()

        device = CMIOExtensionDevice(
            localizedName: localizedName,
            deviceID: KiyoConstants.deviceID,
            legacyDeviceID: nil,
            source: self
        )

        var description: CMFormatDescription?
        let status = CMVideoFormatDescriptionCreate(
            allocator: kCFAllocatorDefault,
            codecType: kCVPixelFormatType_32BGRA,
            width: KiyoConstants.width,
            height: KiyoConstants.height,
            extensions: nil,
            formatDescriptionOut: &description
        )
        precondition(status == noErr && description != nil)

        let frameDuration = CMTime(value: 1, timescale: KiyoConstants.frameRate)
        let streamFormat = CMIOExtensionStreamFormat(
            formatDescription: description!,
            maxFrameDuration: frameDuration,
            minFrameDuration: frameDuration,
            validFrameDurations: [frameDuration]
        )
        sourceStream = KiyoSourceStream(
            localizedName: "\(localizedName) Video",
            streamID: KiyoConstants.sourceStreamID,
            streamFormat: streamFormat,
            device: device
        )

        do {
            try device.addStream(sourceStream.stream)
        } catch {
            fatalError("Unable to attach the Kiyo source stream: \(error.localizedDescription)")
        }
    }

    var availableProperties: Set<CMIOExtensionProperty> {
        [.deviceTransportType, .deviceModel]
    }

    func deviceProperties(
        forProperties properties: Set<CMIOExtensionProperty>
    ) throws -> CMIOExtensionDeviceProperties {
        let result = CMIOExtensionDeviceProperties(dictionary: [:])
        if properties.contains(.deviceTransportType) {
            result.transportType = kIOAudioDeviceTransportTypeVirtual
        }
        if properties.contains(.deviceModel) {
            result.model = "Razer Kiyo Pro 1080p Relay"
        }
        return result
    }

    func setDeviceProperties(_ deviceProperties: CMIOExtensionDeviceProperties) throws {}

    func startSource() {
        captureQueue.async {
            self.sourceClientCount += 1
            guard self.sourceClientCount == 1 else { return }
            self.ensureCameraAuthorization()
        }
    }

    func stopSource() {
        captureQueue.async {
            self.sourceClientCount = max(0, self.sourceClientCount - 1)
            if self.sourceClientCount == 0 {
                self.captureSession?.stopRunning()
                self.captureSession = nil
            }
        }
    }

    private func ensureCameraAuthorization() {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized:
            configureCapture()
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { granted in
                self.captureQueue.async {
                    if granted {
                        self.configureCapture()
                    } else {
                        Logger.extension.error("Camera access was denied")
                    }
                }
            }
        default:
            Logger.extension.error("Camera access is unavailable")
        }
    }

    private func configureCapture() {
        guard sourceClientCount > 0, captureSession == nil else { return }
        let externalDeviceTypes: [AVCaptureDevice.DeviceType]
        if #available(macOS 14.0, *) {
            externalDeviceTypes = [.external]
        } else {
            externalDeviceTypes = [.externalUnknown]
        }
        guard let camera = AVCaptureDevice.DiscoverySession(
            deviceTypes: externalDeviceTypes,
            mediaType: .video,
            position: .unspecified
        ).devices.first(where: { $0.localizedName == KiyoConstants.physicalCameraName }) else {
            Logger.extension.error("Razer Kiyo Pro was not found")
            return
        }

        let targetFrameRate = Double(KiyoConstants.frameRate)
        let frameRateTolerance = 0.01
        guard let (selectedFormat, selectedFrameRateRange) = camera.formats
            .compactMap({ format -> (AVCaptureDevice.Format, AVFrameRateRange)? in
                let dimensions = CMVideoFormatDescriptionGetDimensions(format.formatDescription)
                guard dimensions.width == KiyoConstants.width,
                      dimensions.height == KiyoConstants.height,
                      let range = format.videoSupportedFrameRateRanges.first(where: {
                          $0.minFrameRate - frameRateTolerance <= targetFrameRate
                              && $0.maxFrameRate + frameRateTolerance >= targetFrameRate
                      }) else {
                    return nil
                }
                return (format, range)
            })
            .max(by: {
                pixelFormatPriority(CMFormatDescriptionGetMediaSubType($0.0.formatDescription))
                    < pixelFormatPriority(CMFormatDescriptionGetMediaSubType($1.0.formatDescription))
            }) else {
            Logger.extension.error("Kiyo Pro does not expose 1080p60")
            return
        }

        do {
            try camera.lockForConfiguration()
            camera.activeFormat = selectedFormat
            let duration = selectedFrameRateRange.minFrameDuration
            camera.activeVideoMinFrameDuration = duration
            camera.activeVideoMaxFrameDuration = duration
            camera.unlockForConfiguration()

            let input = try AVCaptureDeviceInput(device: camera)
            let output = AVCaptureVideoDataOutput()
            output.alwaysDiscardsLateVideoFrames = true
            output.videoSettings = [
                kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA
            ]
            output.setSampleBufferDelegate(self, queue: captureQueue)

            let session = AVCaptureSession()
            session.beginConfiguration()
            guard session.canAddInput(input), session.canAddOutput(output) else {
                session.commitConfiguration()
                Logger.extension.error("Unable to configure Kiyo capture inputs")
                return
            }
            session.addInput(input)
            session.addOutput(output)
            session.commitConfiguration()
            captureSession = session
            session.startRunning()
            Logger.extension.info("Kiyo Pro capture started at 1920x1080/60")
        } catch {
            Logger.extension.error("Kiyo capture failed: \(error.localizedDescription)")
        }
    }

    private func pixelFormatPriority(_ format: FourCharCode) -> Int {
        switch format {
        case kCVPixelFormatType_32BGRA:
            return 3
        case kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange:
            return 2
        default:
            return 1
        }
    }
}

extension KiyoDeviceSource: AVCaptureVideoDataOutputSampleBufferDelegate {
    func captureOutput(
        _ output: AVCaptureOutput,
        didOutput sampleBuffer: CMSampleBuffer,
        from connection: AVCaptureConnection
    ) {
        guard sourceClientCount > 0 else { return }
        let imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer)
        guard let imageBuffer,
              CVPixelBufferGetWidth(imageBuffer) == Int(KiyoConstants.width),
              CVPixelBufferGetHeight(imageBuffer) == Int(KiyoConstants.height),
              CVPixelBufferGetPixelFormatType(imageBuffer) == kCVPixelFormatType_32BGRA else {
            Logger.extension.error("Capture delivered an incompatible frame")
            return
        }

        let now = CMClockGetTime(CMClockGetHostTimeClock())
        sourceStream.stream.send(
            sampleBuffer,
            discontinuity: [],
            hostTimeInNanoseconds: UInt64(now.seconds * Double(NSEC_PER_SEC))
        )
    }
}

private extension Logger {
    static let `extension` = Logger(
        subsystem: KiyoConstants.extensionBundleIdentifier,
        category: "camera"
    )
}
