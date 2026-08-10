import Darwin
import Foundation
import MacHuntCore

@main
enum MacHuntCLI {
    static func main() async {
        do {
            try await run(Array(CommandLine.arguments.dropFirst()))
        } catch {
            FileHandle.standardError.write(Data("machunt: \(error.localizedDescription)\n".utf8))
            exit(EXIT_FAILURE)
        }
    }

    private static func run(_ arguments: [String]) async throws {
        guard let command = arguments.first else {
            printHelp()
            return
        }
        switch command {
        case "search":
            let database = try SearchDatabase(url: SearchDatabase.defaultURL())
            try search(database: database, arguments: Array(arguments.dropFirst()))
        case "build", "rebuild":
            let database = try SearchDatabase(url: SearchDatabase.defaultURL())
            let path = option("--path", in: arguments) ?? "/"
            let configuration = IndexConfiguration(roots: [path])
            let progressTask = Task {
                var lastProgress: String?
                while !Task.isCancelled {
                    try? await Task.sleep(for: .seconds(1))
                    guard !Task.isCancelled,
                          let progress = try? database.rebuildProgress() else { continue }
                    let message = switch progress.phase {
                    case .scanning: "Scanning files — \(progress.itemCount) items"
                    case .buildingSearchIndex: "Building search index — \(progress.itemCount) items"
                    case .optimizing: "Optimizing database — \(progress.itemCount) items"
                    case .complete: "Finishing index — \(progress.itemCount) items"
                    }
                    guard message != lastProgress else { continue }
                    lastProgress = message
                    print(message)
                }
            }
            defer { progressTask.cancel() }
            let summary = try await database.rebuildIndex(configuration: configuration)
            print("Indexed \(summary.itemCount) items")
        case "optimize":
            let database = try SearchDatabase(url: SearchDatabase.defaultURL())
            try await database.optimizeForSearching()
            print("Search index optimized")
        case "status":
            let database = try SearchDatabase(url: SearchDatabase.defaultURL())
            print("Indexed \(try database.indexedItemCount()) items")
        case "help", "--help", "-h":
            printHelp()
        default:
            throw CLIError("Unknown command: \(command)")
        }
    }

    private static func search(database: SearchDatabase, arguments: [String]) throws {
        let query = arguments.first(where: { !$0.hasPrefix("-") && !isOptionValue($0, in: arguments) }) ?? ""
        let mode: SearchMode = arguments.contains("--pattern") || arguments.contains("-p")
            ? .pattern
            : arguments.contains("--fuzzy") || arguments.contains("-F") ? .fuzzy : .substring
        let category: ResultCategory = arguments.contains("--dirs") || arguments.contains("-d")
            ? .folders
            : arguments.contains("--files") || arguments.contains("-f") ? .files : .all
        let request = SearchRequest(
            query: query,
            mode: mode,
            caseSensitive: arguments.contains("--case-sensitive") || arguments.contains("-c"),
            pathPrefix: option("--path", short: "-P", in: arguments),
            category: category,
            limit: Int(option("--limit", short: "-n", in: arguments) ?? "100") ?? 100
        )
        let results = try database.search(request)
        if arguments.contains("--json") {
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            FileHandle.standardOutput.write(try encoder.encode(results))
            FileHandle.standardOutput.write(Data("\n".utf8))
        } else {
            results.forEach { print($0.path) }
        }
    }

    private static func option(_ name: String, short: String? = nil, in arguments: [String]) -> String? {
        for key in [name, short].compactMap({ $0 }) {
            if let index = arguments.firstIndex(of: key), arguments.indices.contains(index + 1) {
                return arguments[index + 1]
            }
            if let value = arguments.first(where: { $0.hasPrefix(key + "=") }) {
                return String(value.dropFirst(key.count + 1))
            }
        }
        return nil
    }

    private static func isOptionValue(_ value: String, in arguments: [String]) -> Bool {
        guard let index = arguments.firstIndex(of: value), index > 0 else { return false }
        return ["--path", "-P", "--limit", "-n"].contains(arguments[index - 1])
    }

    private static func printHelp() {
        print("""
        MacHunt — native Swift file search

        machunt search [options] <query>
          -p, --pattern           wildcard/regular-expression search
          -F, --fuzzy             fuzzy subsequence search
          -c, --case-sensitive    preserve case
          -P, --path <path>       limit to a path prefix
          -f, --files             files only
          -d, --dirs              folders only
          -n, --limit <count>     maximum results
              --json              JSON output

        machunt build [--path <path>]
        machunt optimize
        machunt status
        """)
    }
}

private struct CLIError: LocalizedError {
    let message: String
    init(_ message: String) { self.message = message }
    var errorDescription: String? { message }
}
