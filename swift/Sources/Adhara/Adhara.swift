import AdharaCore
import Foundation

/// An SQLite error: the result code and SQLite's message.
public struct AdharaError: Error, CustomStringConvertible {
    public let code: Int32
    public let message: String
    public var description: String { "SQLite \(code): \(message)" }
}

/// A value bound to or read from SQLite.
public enum Value: Equatable, Sendable {
    case null, int(Int64), real(Double), text(String), blob(Data)
}

/// One `adhara_hybrid` row: the rowid, its RRF score, its rank in each leg and its vector distance.
public struct Hit: Equatable, Sendable {
    public let id: Int64, score: Double, ftsRank: Int?, vecRank: Int?, dist: Double?
}

nonisolated(unsafe) private let transient = unsafeBitCast(-1, to: sqlite3_destructor_type.self)

/// A connection to the SQLite built into libadhara, with the `adhara` tokenizer and `adhara_hybrid` registered.
public final class Database {
    public let handle: OpaquePointer

    public init(path: String, readonly: Bool = false) throws {
        var db: OpaquePointer?
        let flags = (readonly ? SQLITE_OPEN_READONLY : SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE) | SQLITE_OPEN_FULLMUTEX
        let rc = sqlite3_open_v2(path, &db, flags, nil)
        guard rc == SQLITE_OK, let db else {
            let msg = db.map { String(cString: sqlite3_errmsg($0)) } ?? "open failed"
            sqlite3_close_v2(db)
            throw AdharaError(code: rc, message: msg)
        }
        handle = db
        try check(adhara_register(db))
    }

    deinit { sqlite3_close_v2(handle) }

    /// The version of the SQLite built into libadhara.
    public static var sqliteVersion: String { String(cString: sqlite3_libversion()) }

    private func check(_ rc: Int32) throws {
        if rc != SQLITE_OK && rc != SQLITE_ROW && rc != SQLITE_DONE { throw AdharaError(code: rc, message: String(cString: sqlite3_errmsg(handle))) }
    }

    /// Run one or more statements that return no rows.
    public func exec(_ sql: String) throws { try check(sqlite3_exec(handle, sql, nil, nil, nil)) }

    private func bind(_ st: OpaquePointer, _ args: [Value]) throws {
        for (i, v) in args.enumerated() {
            let n = Int32(i + 1)
            switch v {
            case .null: try check(sqlite3_bind_null(st, n))
            case .int(let x): try check(sqlite3_bind_int64(st, n, x))
            case .real(let x): try check(sqlite3_bind_double(st, n, x))
            case .text(let s): try check(sqlite3_bind_text(st, n, s, -1, transient))
            case .blob(let d): try d.withUnsafeBytes { try check(sqlite3_bind_blob(st, n, $0.baseAddress, Int32($0.count), transient)) }
            }
        }
    }

    private func column(_ st: OpaquePointer, _ i: Int32) -> Value {
        switch sqlite3_column_type(st, i) {
        case SQLITE_INTEGER: return .int(sqlite3_column_int64(st, i))
        case SQLITE_FLOAT: return .real(sqlite3_column_double(st, i))
        case SQLITE_TEXT: return .text(sqlite3_column_text(st, i).map { String(cString: $0) } ?? "")
        case SQLITE_BLOB:
            let n = Int(sqlite3_column_bytes(st, i))
            return .blob(n == 0 ? Data() : Data(bytes: sqlite3_column_blob(st, i), count: n))
        default: return .null
        }
    }

    /// Run a query and return each row as column name to value.
    public func query(_ sql: String, _ args: [Value] = []) throws -> [[String: Value]] {
        var st: OpaquePointer?
        try check(sqlite3_prepare_v2(handle, sql, -1, &st, nil))
        guard let st else { return [] }
        defer { sqlite3_finalize(st) }
        try bind(st, args)
        let names = (0..<sqlite3_column_count(st)).map { String(cString: sqlite3_column_name(st, $0)) }
        var rows: [[String: Value]] = []
        while true {
            let rc = sqlite3_step(st)
            if rc == SQLITE_DONE { break }
            try check(rc)
            rows.append(Dictionary(uniqueKeysWithValues: names.enumerated().map { ($0.element, column(st, Int32($0.offset))) }))
        }
        return rows
    }

    /// Hybrid search on a litesearch store: the FTS5 leg for `fts`, the vector leg for `vector`, fused by RRF.
    /// `ann: false` scans every vector exactly instead of using the HNSW sidecar.
    public func search(store: String = "store", fts: String?, vector: Data?, k: Int = 10, filter: String? = nil,
                       dtype: String = "f16", metric: String = "cosine", ann: Bool = true) throws -> [Hit] {
        let args: [Value] = [.text(store), fts.map(Value.text) ?? .null, vector.map(Value.blob) ?? .null, .int(Int64(k)),
                             filter.map(Value.text) ?? .null, .text(dtype), .text(metric), .int(ann ? 1 : 0)]
        return try query("select id, score, fts_rank, vec_rank, dist from adhara_hybrid(?, ?, ?, ?, ?, ?, ?, ?)", args).map { r in
            func int(_ k: String) -> Int? { if case .int(let x) = r[k] { return Int(x) }; return nil }
            func real(_ k: String) -> Double? { if case .real(let x) = r[k] { return x }; return nil }
            return Hit(id: Int64(int("id") ?? 0), score: real("score") ?? 0, ftsRank: int("fts_rank"), vecRank: int("vec_rank"), dist: real("dist"))
        }
    }
}

/// The FTS5 tokenizer chain for new stores, as litesearch names it.
public let tokenize = "porter adhara"

/// An FTS5 query matching any of `words`, each quoted and matched as a prefix, as litesearch's `pre` without its stopwords.
public func anyOf(_ words: [String]) -> String? {
    var seen = Set<String>()
    let ws = words.map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty && seen.insert($0).inserted }
    return ws.isEmpty ? nil : ws.map { "\"\($0.replacingOccurrences(of: "\"", with: "\"\""))\"*" }.joined(separator: " OR ")
}
