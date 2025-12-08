//
//  ContentView.swift
//  RustCameraTest
//
//  Created by Alex on 12/8/25.
//

import SwiftUI

struct ContentView: View {
    var body: some View {
        VStack(spacing: 20) {
            Text("Rust Camera Test")
                .font(.title)

            Button("Start Camera") {
                asimov_camera_avf_test()
            }
            .padding()
        }
    }
}

#Preview {
    ContentView()
}
