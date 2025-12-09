//
//  RustBindings.swift
//  RustCameraTest
//
//  Created by Alex on 12/8/25.
//

import Foundation
import AVFoundation
import UIKit

// MARK: - Raw FFI declarations (matching asimov_camera_module.h)

typealias AsimovCameraFrameCallbackSwift =
@convention(c) (
    UnsafePointer<UInt8>?,
    Int,              // size_t len
    UInt32,           // width
    UInt32,           // height
    UInt32,           // stride
    UnsafeMutableRawPointer?
) -> Void

// repr(C) enum in Rust -> treat as Int32. 0 = Ok, non-zero = error.
typealias AsimovCameraErrorCodeSwift = Int32

@_silgen_name("asimov_camera_open")
func asimov_camera_open(
    _ device: UnsafePointer<CChar>?,
    _ width: UInt32,
    _ height: UInt32,
    _ fps: Double,
    _ frame_callback: AsimovCameraFrameCallbackSwift,
    _ user_data: UnsafeMutableRawPointer?,
    _ out_handle: UnsafeMutablePointer<UnsafeMutableRawPointer?>
) -> AsimovCameraErrorCodeSwift

@_silgen_name("asimov_camera_start")
func asimov_camera_start(
    _ handle: UnsafeMutableRawPointer?
) -> AsimovCameraErrorCodeSwift

@_silgen_name("asimov_camera_stop")
func asimov_camera_stop(
    _ handle: UnsafeMutableRawPointer?
) -> AsimovCameraErrorCodeSwift

@_silgen_name("asimov_camera_free")
func asimov_camera_free(
    _ handle: UnsafeMutableRawPointer?
)

@_silgen_name("asimov_camera_get_session")
func asimov_camera_get_session(
    _ handle: UnsafeMutableRawPointer?
) -> UnsafeMutableRawPointer?

// MARK: - Swift wrapper

/// Thin Swift wrapper around the Rust asimov-camera-module FFI.
final class AsimovCamera: NSObject {
    private var handle: UnsafeMutableRawPointer?
    private(set) var previewLayer: AVCaptureVideoPreviewLayer?

    var width: UInt32 = 1280
    var height: UInt32 = 720
    var fps: Double = 30.0

    // MARK: Public API

    /// Open the Rust camera driver and create an AVCaptureSession.
    /// `front: true` -> use front camera, `false` -> back camera.
    @discardableResult
    func open(front: Bool = true) -> Bool {
        let userData = Unmanaged.passUnretained(self).toOpaque()
        var rawHandle: UnsafeMutableRawPointer?

        // Resolve camera device ID on the Swift side
        let position: AVCaptureDevice.Position = front ? .front : .back
        let deviceID = AsimovCamera.deviceId(for: position)

        let err: AsimovCameraErrorCodeSwift

        if let deviceID {
            err = deviceID.withCString { cstr in
                asimov_camera_open(
                    cstr,
                    width,
                    height,
                    fps,
                    asimovFrameCallback,
                    userData,
                    &rawHandle
                )
            }
        } else {
            // Fallback to default device if we couldn't find requested one.
            err = asimov_camera_open(
                nil,
                width,
                height,
                fps,
                asimovFrameCallback,
                userData,
                &rawHandle
            )
        }

        if err != 0 {
            print("asimov_camera_open failed with code \(err)")
            return false
        }

        guard let h = rawHandle else {
            print("asimov_camera_open returned null handle")
            return false
        }

        self.handle = h

        if let sessionPtr = asimov_camera_get_session(h) {
            let session = unsafeBitCast(sessionPtr, to: AVCaptureSession.self)
            let layer = AVCaptureVideoPreviewLayer(session: session)
            layer.videoGravity = AVLayerVideoGravity.resizeAspectFill
            self.previewLayer = layer
        } else {
            print("asimov_camera_get_session returned null; preview will be unavailable")
        }

        return true
    }

    /// Start capturing frames.
    func start() {
        guard let handle else {
            print("start(): no handle")
            return
        }
        let err = asimov_camera_start(handle)
        if err != 0 {
            print("asimov_camera_start failed with code \(err)")
        }
    }

    /// Stop capturing frames.
    func stop() {
        guard let handle else { return }
        let err = asimov_camera_stop(handle)
        if err != 0 {
            print("asimov_camera_stop failed with code \(err)")
        }
    }

    /// Free the Rust handle and release the preview layer.
    func close() {
        guard let handle else { return }
        asimov_camera_free(handle)
        self.handle = nil
        self.previewLayer = nil
    }

    deinit {
        close()
    }

    // MARK: Frame handling from Rust

    func handleFrame(
        dataPtr: UnsafePointer<UInt8>,
        length: Int,
        width: UInt32,
        height: UInt32,
        stride: UInt32
    ) {
        // For now just log that frames are coming.
        print("Frame: \(width)x\(height), stride=\(stride), bytes=\(length)")
    }

    // MARK: Helpers

    private static func deviceId(for position: AVCaptureDevice.Position) -> String? {
        guard
            let device = AVCaptureDevice.default(.builtInWideAngleCamera,
                                                 for: .video,
                                                 position: position)
        else { return nil }
        return device.uniqueID
    }
}

// MARK: - Global callback exported to Rust

@_cdecl("asimovFrameCallback")
func asimovFrameCallback(
    data: UnsafePointer<UInt8>?,
    len: Int,
    width: UInt32,
    height: UInt32,
    stride: UInt32,
    user_data: UnsafeMutableRawPointer?
) {
    guard
        let data,
        let user_data
    else { return }

    let cam = Unmanaged<AsimovCamera>
        .fromOpaque(user_data)
        .takeUnretainedValue()

    cam.handleFrame(
        dataPtr: data,
        length: len,
        width: width,
        height: height,
        stride: stride
    )
}
