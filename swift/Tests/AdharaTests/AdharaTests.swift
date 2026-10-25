import Adhara
import Foundation
import XCTest

/// Little-endian f32 bytes, as litesearch stores `np.float32` vectors.
func f32(_ xs: [Float]) -> Data { xs.withUnsafeBufferPointer { Data(buffer: $0) } }

final class AdharaTests: XCTestCase {
    func makeStore() throws -> Database {
        let db = try Database(path: ":memory:")
        try db.exec("""
            create table store(id integer primary key, content text, embedding blob, metadata text);
            create virtual table store_fts using fts5(content, metadata, tokenize='\(tokenize)', content=store);
            """)
        let rows: [(String, [Float], String)] = [("भीष्म spoke to the king", [1, 0], #"{"g": 1}"#),
                                                ("Krishna smiled", [0, 1], #"{"g": 2}"#),
                                                ("nothing here", [1, 1], #"{"g": 1}"#)]
        for (t, v, m) in rows { _ = try db.query("insert into store(content, embedding, metadata) values (?, ?, ?)", [.text(t), .blob(f32(v)), .text(m)]) }
        try db.exec("insert into store_fts(store_fts) values ('rebuild')")
        return db
    }

    func testSQLiteVersion() { XCTAssertGreaterThanOrEqual(Database.sqliteVersion, "3.45.3") }

    func testMatchAcrossScripts() throws {
        let db = try makeStore()
        let ids = { (q: String) in try db.query("select rowid from store_fts where store_fts match ? order by rowid", [.text(q)]).compactMap { r -> Int64? in if case .int(let x) = r["rowid"] { return x }; return nil } }
        XCTAssertEqual(try ids("Bheeshma"), [1])
        XCTAssertEqual(try ids("कृष्ण"), [2])
    }

    func testHybrid() throws {
        let db = try makeStore()
        let hits = try db.search(fts: anyOf(["Bheeshma"]), vector: f32([1, 0]), k: 3, dtype: "f32")
        XCTAssertEqual(hits.map(\.id), [1, 3, 2])
        XCTAssertEqual(hits[0].ftsRank, 0)
        XCTAssertEqual(hits[0].vecRank, 0)
        XCTAssertEqual(hits[1].ftsRank, nil)
        XCTAssertEqual(hits[1].dist ?? 0, 1 - 1 / 2.squareRoot(), accuracy: 1e-4)
    }

    func testFilter() throws {
        let db = try makeStore()
        let hits = try db.search(fts: nil, vector: f32([0, 1]), k: 3, filter: #"json_extract(metadata, '$.g') = 1"#, dtype: "f32")
        XCTAssertEqual(hits.map(\.id), [3, 1])
    }

    func testErrors() throws {
        let db = try makeStore()
        XCTAssertThrowsError(try db.search(fts: nil, vector: Data([0, 0, 0]), k: 3, dtype: "f32"))
        XCTAssertThrowsError(try db.exec("select * from nowhere"))
    }

    func testAnyOf() {
        XCTAssertEqual(anyOf(["a", "b \"c\"", "a", " "]), #""a"* OR "b ""c"""*"#)
        XCTAssertNil(anyOf([]))
    }
}
