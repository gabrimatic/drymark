#!/usr/bin/env swift

import AppKit
import CoreGraphics
import Foundation

private enum SmokeFailure: Error {
    case appNotRunning
    case clipboardSnapshot
    case clipboardWrite
    case shortcutDispatch
    case preserveResult
    case richFormatsRemain
    case emptyFormattingRemains
    case nonTextClipboardChanged
    case visualFeedbackMissing
    case clipboardRestore
}

private let pasteboard = NSPasteboard.general
private let customType = NSPasteboard.PasteboardType("info.gabrimatic.drymark.synthetic-rich")

private func snapshotClipboard() throws -> [NSPasteboardItem] {
    guard let sourceItems = pasteboard.pasteboardItems else {
        return []
    }

    return try sourceItems.map { source in
        let copy = NSPasteboardItem()
        for type in source.types {
            guard let data = source.data(forType: type) else {
                throw SmokeFailure.clipboardSnapshot
            }
            copy.setData(data, forType: type)
        }
        return copy
    }
}

private func replaceClipboard(with item: NSPasteboardItem) throws -> Int {
    pasteboard.clearContents()
    guard pasteboard.writeObjects([item]) else {
        throw SmokeFailure.clipboardWrite
    }
    return pasteboard.changeCount
}

private func restoreClipboard(_ items: [NSPasteboardItem]) throws {
    pasteboard.clearContents()
    if !items.isEmpty && !pasteboard.writeObjects(items) {
        throw SmokeFailure.clipboardRestore
    }
}

private func triggerDefaultShortcut() throws {
    let source = """
    tell application "System Events"
      key code 9 using {option down, shift down}
    end tell
    """
    var details: NSDictionary?
    guard let script = NSAppleScript(source: source),
          script.executeAndReturnError(&details).descriptorType != 0 else {
        throw SmokeFailure.shortcutDispatch
    }
}

private func waitForClipboard(
    after initialChangeCount: Int,
    timeout: TimeInterval = 5,
    predicate: () -> Bool
) -> Bool {
    let deadline = Date().addingTimeInterval(timeout)
    while Date() < deadline {
        if pasteboard.changeCount != initialChangeCount && predicate() {
            return true
        }
        RunLoop.current.run(until: Date().addingTimeInterval(0.025))
    }
    return false
}

private func waitWithoutClipboardChange(from initialChangeCount: Int) -> Bool {
    let deadline = Date().addingTimeInterval(0.8)
    while Date() < deadline {
        if pasteboard.changeCount != initialChangeCount {
            return false
        }
        RunLoop.current.run(until: Date().addingTimeInterval(0.025))
    }
    return true
}

private func hasVisibleResultWindow(processIdentifier: pid_t) -> Bool {
    guard let windows = CGWindowListCopyWindowInfo(
        [.optionOnScreenOnly, .excludeDesktopElements],
        kCGNullWindowID
    ) as? [[String: Any]] else {
        return false
    }

    return windows.contains { window in
        guard let ownerPID = window[kCGWindowOwnerPID as String] as? NSNumber,
              ownerPID.int32Value == processIdentifier,
              window[kCGWindowName as String] as? String == "DryMark Result",
              window[kCGWindowIsOnscreen as String] as? Bool == true,
              let bounds = window[kCGWindowBounds as String] as? NSDictionary,
              let frame = CGRect(dictionaryRepresentation: bounds) else {
            return false
        }
        return frame.width == 360 && frame.height == 84
    }
}

private func waitForVisualFeedback(
    processIdentifier: pid_t,
    timeout: TimeInterval = 2
) -> Bool {
    let deadline = Date().addingTimeInterval(timeout)
    while Date() < deadline {
        if hasVisibleResultWindow(processIdentifier: processIdentifier) {
            return true
        }
        RunLoop.current.run(until: Date().addingTimeInterval(0.025))
    }
    return false
}

private func runSmokeTest() throws {
    guard let application = NSRunningApplication.runningApplications(
        withBundleIdentifier: "info.gabrimatic.drymark"
    ).first else {
        throw SmokeFailure.appNotRunning
    }

    let input = "same\u{200B} words 👩‍💻 می\u{200C}روم safe\u{202E}txt\u{202C}"
    let expected = "same words 👩‍💻 می\u{200C}روم safetxt"
    let richItem = NSPasteboardItem()
    richItem.setString(input, forType: .string)
    richItem.setString("<b>synthetic fixture</b>", forType: .html)
    richItem.setData(Data([0x54, 0x53]), forType: customType)
    let richChangeCount = try replaceClipboard(with: richItem)

    try triggerDefaultShortcut()
    guard waitForClipboard(after: richChangeCount, predicate: {
        pasteboard.string(forType: .string) == expected
    }) else {
        throw SmokeFailure.preserveResult
    }
    guard waitForVisualFeedback(processIdentifier: application.processIdentifier) else {
        throw SmokeFailure.visualFeedbackMissing
    }

    let resultingTypes = Set(pasteboard.pasteboardItems?.flatMap(\.types) ?? [])
    guard !resultingTypes.contains(.html),
          !resultingTypes.contains(.rtf),
          !resultingTypes.contains(.rtfd),
          !resultingTypes.contains(customType) else {
        throw SmokeFailure.richFormatsRemain
    }

    RunLoop.current.run(until: Date().addingTimeInterval(0.15))
    let emptyItem = NSPasteboardItem()
    emptyItem.setString("", forType: .string)
    emptyItem.setString("<i>synthetic empty fixture</i>", forType: .html)
    emptyItem.setData(Data([0x45, 0x4D]), forType: customType)
    let emptyChangeCount = try replaceClipboard(with: emptyItem)
    try triggerDefaultShortcut()
    guard waitForClipboard(after: emptyChangeCount, predicate: {
        pasteboard.string(forType: .string) == ""
    }) else {
        throw SmokeFailure.emptyFormattingRemains
    }
    let emptyResultTypes = Set(pasteboard.pasteboardItems?.flatMap(\.types) ?? [])
    guard !emptyResultTypes.contains(.html),
          !emptyResultTypes.contains(.rtf),
          !emptyResultTypes.contains(.rtfd),
          !emptyResultTypes.contains(customType) else {
        throw SmokeFailure.emptyFormattingRemains
    }

    let nonTextItem = NSPasteboardItem()
    nonTextItem.setData(
        Data(base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=") ?? Data(),
        forType: .png
    )
    let nonTextChangeCount = try replaceClipboard(with: nonTextItem)
    try triggerDefaultShortcut()
    guard waitWithoutClipboardChange(from: nonTextChangeCount) else {
        throw SmokeFailure.nonTextClipboardChanged
    }
}

var result = 0
let originalClipboard: [NSPasteboardItem]
do {
    originalClipboard = try snapshotClipboard()
} catch {
    fputs("DryMark runtime smoke: original clipboard could not be snapshotted; nothing changed.\n", stderr)
    exit(2)
}

do {
    try runSmokeTest()
    print("DryMark runtime smoke: packaged shortcut, Preserve policy, visible feedback, rich-format clearing, empty rich text, and non-text checks passed.")
} catch {
    fputs("DryMark runtime smoke: a synthetic packaged-app check failed.\n", stderr)
    result = 1
}

do {
    try restoreClipboard(originalClipboard)
} catch {
    fputs("DryMark runtime smoke: original clipboard restoration failed.\n", stderr)
    result = 1
}

exit(Int32(result))
