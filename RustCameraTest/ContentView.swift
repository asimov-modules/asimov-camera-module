//
//  ContentView.swift
//  RustCameraTest
//
//  Created by Alex on 12/8/25.
//

import SwiftUI
import AVFoundation
import Combine

// Controller that owns AsimovCamera for SwiftUI.
final class CameraController: ObservableObject {
    private let camera = AsimovCamera()

    @Published var previewLayer: AVCaptureVideoPreviewLayer?

    private var activePosition: AVCaptureDevice.Position?

    // MARK: - Public API

    func startFront() {
        start(position: .front)
    }

    func startBack() {
        start(position: .back)
    }

    func stop() {
        camera.stop()
    }

    deinit {
        camera.close()
    }

    // MARK: - Internal

    private func start(position: AVCaptureDevice.Position) {
        // If we already opened this same camera, just start it again.
        if let current = activePosition,
           current == position,
           camera.previewLayer != nil
        {
            camera.start()
            return
        }

        // We are switching camera (or starting for the first time).
        // Make sure the previous one is fully cleaned up.
        camera.stop()
        camera.close()
        activePosition = nil
        previewLayer = nil

        // Open new camera (front/back)
        let opened = camera.open(front: position == .front)
        if opened {
            activePosition = position
            previewLayer = camera.previewLayer
            camera.start()
        } else {
            print("Failed to open camera for position \(position.rawValue)")
        }
    }
}

// UIView that hosts the AVCaptureVideoPreviewLayer.
final class CameraPreviewUIView: UIView {
    var previewLayer: AVCaptureVideoPreviewLayer? {
        didSet {
            oldValue?.removeFromSuperlayer()
            if let layer = previewLayer {
                layer.frame = bounds
                layer.videoGravity = AVLayerVideoGravity.resizeAspectFill
                self.layer.addSublayer(layer)
            }
        }
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        previewLayer?.frame = bounds
    }
}

// SwiftUI wrapper for the preview UIView.
struct CameraPreviewView: UIViewRepresentable {
    @ObservedObject var controller: CameraController

    func makeUIView(context: Context) -> CameraPreviewUIView {
        let view = CameraPreviewUIView()
        view.backgroundColor = .black
        view.previewLayer = controller.previewLayer
        return view
    }

    func updateUIView(_ uiView: CameraPreviewUIView, context: Context) {
        uiView.previewLayer = controller.previewLayer
    }
}

struct ContentView: View {
    @StateObject private var controller = CameraController()

    var body: some View {
        VStack {
            Text("Rust Camera Test")
                .font(.title)
                .padding(.top)

            CameraPreviewView(controller: controller)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Color.black)
                .onAppear {
                    // Only ask for permission; don't auto-start camera.
                    AVCaptureDevice.requestAccess(for: .video) { granted in
                        if !granted {
                            print("Camera permission not granted")
                        }
                    }
                }

            HStack {
                Button("Start Front") {
                    controller.startFront()
                }
                .padding()

                Button("Start Back") {
                    controller.startBack()
                }
                .padding()

                Button("Stop") {
                    controller.stop()
                }
                .padding()
            }
        }
    }
}

#Preview {
    ContentView()
}
