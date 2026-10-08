// Resolve only the capture child's window; never capture the desktop or another app.
import CoreGraphics
import Foundation

if CommandLine.arguments.count == 2 && CommandLine.arguments[1] == "--check-session" {
    let session = CGSessionCopyCurrentDictionary() as? [String: Any]
    let locked = session?["CGSSessionScreenIsLocked"] as? Bool ?? false
    if locked || CGDisplayIsAsleep(CGMainDisplayID()) != 0 {
        FileHandle.standardError.write(Data("Unlock the Mac and wake the display before capturing Ropy.\n".utf8))
        exit(1)
    }
    exit(0)
}

guard CommandLine.arguments.count == 2, let pid = Int32(CommandLine.arguments[1]), pid > 0 else {
    exit(2)
}
let windows = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as? [[String: Any]] ?? []
let matches = windows.filter { window in
    guard window[kCGWindowOwnerPID as String] as? Int32 == pid,
          let bounds = window[kCGWindowBounds as String] as? [String: Any],
          let width = bounds["Width"] as? Double,
          let height = bounds["Height"] as? Double else { return false }
    return width >= 400 && height >= 550
}
if matches.count == 1, let id = matches[0][kCGWindowNumber as String] as? Int {
    print(id)
}
