// SPDX-License-Identifier: Apache-2.0
//
// Make the given app the default handler for .torrent files and magnet links.
// Usage: swift set-default-handler.swift /path/to/Pouletb.app

import AppKit
import UniformTypeIdentifiers

let args = CommandLine.arguments
guard args.count == 2 else {
    FileHandle.standardError.write(Data("usage: set-default-handler.swift <App.app>\n".utf8))
    exit(2)
}
let app = URL(fileURLWithPath: args[1])
var failed = false

@MainActor
func report(_ label: String, _ change: () async throws -> Void) async {
    do {
        try await change()
        print("default for \(label): \(app.lastPathComponent)")
    } catch {
        FileHandle.standardError.write(Data("\(label): \(error.localizedDescription)\n".utf8))
        failed = true
    }
}

let torrent = UTType(filenameExtension: "torrent", conformingTo: .data)!
await report(".torrent (\(torrent.identifier))") {
    try await NSWorkspace.shared.setDefaultApplication(at: app, toOpen: torrent)
}
// macOS asks the user to confirm a URL-scheme change; declining leaves the old handler.
await report("magnet: links") {
    try await NSWorkspace.shared.setDefaultApplication(at: app, toOpenURLsWithScheme: "magnet")
}

exit(failed ? 1 : 0)
