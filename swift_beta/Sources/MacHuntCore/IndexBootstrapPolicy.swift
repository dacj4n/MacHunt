import Foundation

public enum IndexBootstrapPolicy {
    public static func shouldBuildIndex(itemCount: Int, indexIsComplete _: Bool) -> Bool {
        itemCount == 0
    }

    public static func defaultRoots(fileManager: FileManager = .default) -> [URL] {
        var roots = [URL(fileURLWithPath: "/", isDirectory: true)]
        let volumesURL = URL(fileURLWithPath: "/Volumes", isDirectory: true)
        let volumeKeys: Set<URLResourceKey> = [
            .isDirectoryKey, .isSymbolicLinkKey, .volumeIsLocalKey, .volumeIsBrowsableKey,
        ]
        let volumes = (try? fileManager.contentsOfDirectory(
            at: volumesURL,
            includingPropertiesForKeys: Array(volumeKeys),
            options: [.skipsHiddenFiles]
        )) ?? []

        for volume in volumes {
            guard volume.lastPathComponent != "Macintosh HD",
                  let values = try? volume.resourceValues(forKeys: volumeKeys),
                  values.isDirectory == true,
                  values.isSymbolicLink != true,
                  values.volumeIsLocal == true,
                  values.volumeIsBrowsable != false else { continue }
            roots.append(volume)
        }
        return roots
    }
}
