import AppKit
import CoreServices
import Foundation

final class FileSystemMonitor: @unchecked Sendable {
    struct Batch: Sendable {
        let paths: [String]
        let lastEventID: UInt64
        let requiresFullRescan: Bool
    }

    private final class CallbackBox: @unchecked Sendable {
        let handler: @Sendable (Batch) -> Void
        init(handler: @escaping @Sendable (Batch) -> Void) { self.handler = handler }
    }

    private let lock = NSLock()
    private var resources: [(FSEventStreamRef, UnsafeMutableRawPointer)] = []
    private var generation = 0

    func start(
        paths: [String],
        since eventID: UInt64?,
        handler: @escaping @Sendable (Batch) -> Void
    ) -> Bool {
        stop()
        guard !paths.isEmpty else { return false }
        let currentGeneration = lock.withLock { generation += 1; return generation }
        for path in paths {
            DispatchQueue.global(qos: .utility).async { [weak self] in
                self?.startOne(
                    path: path,
                    since: eventID,
                    generation: currentGeneration,
                    handler: handler
                )
            }
        }
        return true
    }

    private func startOne(
        path: String,
        since eventID: UInt64?,
        generation expectedGeneration: Int,
        handler: @escaping @Sendable (Batch) -> Void
    ) {
        let box = CallbackBox(handler: handler)
        let pointer = Unmanaged.passRetained(box).toOpaque()
        var context = FSEventStreamContext(
            version: 0,
            info: pointer,
            retain: nil,
            release: nil,
            copyDescription: nil
        )
        let callback: FSEventStreamCallback = { _, info, count, rawPaths, eventFlags, eventIDs in
            guard let info else { return }
            let box = Unmanaged<CallbackBox>.fromOpaque(info).takeUnretainedValue()
            let array = Unmanaged<CFArray>.fromOpaque(rawPaths).takeUnretainedValue() as NSArray
            var paths: [String] = []
            var requiresFullRescan = false
            paths.reserveCapacity(count)
            for index in 0..<count {
                let flags = eventFlags[index]
                if flags & UInt32(kFSEventStreamEventFlagHistoryDone) != 0 { continue }
                let recoveryFlags = UInt32(
                    kFSEventStreamEventFlagMustScanSubDirs
                        | kFSEventStreamEventFlagUserDropped
                        | kFSEventStreamEventFlagKernelDropped
                        | kFSEventStreamEventFlagEventIdsWrapped
                )
                if flags & recoveryFlags != 0 { requiresFullRescan = true }
                if let path = array[index] as? String { paths.append(path) }
            }
            guard !paths.isEmpty else { return }
            box.handler(Batch(
                paths: paths,
                lastEventID: eventIDs[count - 1],
                requiresFullRescan: requiresFullRescan
            ))
        }
        let flags = FSEventStreamCreateFlags(
            kFSEventStreamCreateFlagUseCFTypes
                | kFSEventStreamCreateFlagFileEvents
                | kFSEventStreamCreateFlagWatchRoot
                | kFSEventStreamCreateFlagNoDefer
        )
        guard let created = FSEventStreamCreate(
            nil,
            callback,
            &context,
            [path] as CFArray,
            eventID ?? UInt64(kFSEventStreamEventIdSinceNow),
            0.5,
            flags
        ) else {
            Unmanaged<CallbackBox>.fromOpaque(pointer).release()
            return
        }
        FSEventStreamSetDispatchQueue(created, DispatchQueue(label: "com.dacj4n.machunt.fsevents", qos: .utility))
        guard FSEventStreamStart(created) else {
            FSEventStreamInvalidate(created)
            FSEventStreamRelease(created)
            Unmanaged<CallbackBox>.fromOpaque(pointer).release()
            return
        }
        let shouldKeep = lock.withLock { () -> Bool in
            guard generation == expectedGeneration else { return false }
            resources.append((created, pointer))
            return true
        }
        if !shouldKeep {
            FSEventStreamStop(created)
            FSEventStreamInvalidate(created)
            FSEventStreamRelease(created)
            Unmanaged<CallbackBox>.fromOpaque(pointer).release()
        }
    }

    func stop() {
        let stoppedResources = lock.withLock { () -> [(FSEventStreamRef, UnsafeMutableRawPointer)] in
            generation += 1
            let stopped = resources
            resources.removeAll()
            return stopped
        }
        for (stream, pointer) in stoppedResources {
            FSEventStreamStop(stream)
            FSEventStreamInvalidate(stream)
            FSEventStreamRelease(stream)
            Unmanaged<CallbackBox>.fromOpaque(pointer).release()
        }
    }

    deinit { stop() }
}

@MainActor
final class VolumeMonitor {
    var onMount: ((URL) -> Void)?
    var onUnmount: ((URL) -> Void)?
    private var observers: [NSObjectProtocol] = []

    func start() {
        stop()
        let center = NSWorkspace.shared.notificationCenter
        observers.append(center.addObserver(
            forName: NSWorkspace.didMountNotification,
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let url = notification.userInfo?[NSWorkspace.volumeURLUserInfoKey] as? URL else { return }
            MainActor.assumeIsolated { self?.onMount?(url) }
        })
        observers.append(center.addObserver(
            forName: NSWorkspace.didUnmountNotification,
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let url = notification.userInfo?[NSWorkspace.volumeURLUserInfoKey] as? URL else { return }
            MainActor.assumeIsolated { self?.onUnmount?(url) }
        })
    }

    func stop() {
        let center = NSWorkspace.shared.notificationCenter
        observers.forEach(center.removeObserver)
        observers.removeAll()
    }
}
