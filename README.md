# adhara

adhara is the SQLite core of litesearch. The name is Sanskrit for "foundation". adhara is one Rust library. It loads into SQLite and adds an FTS5 tokenizer and a hybrid search function. FTS5 is the full-text search module of SQLite.

- `porter adhara` is the tokenizer. It gives the same tokens as the tokenizer chain of litesearch, `porter simplify casefold 1 sanskrit unicodewords`. It inserts text 4.6 times faster. A query in Devanagari, IAST or plain English spelling finds the same word. `Bheeshma`, `Bhīṣma` and `भीष्म` all match.
- `adhara_hybrid` is the hybrid search function. One SQL call runs a full-text search, a vector search and Reciprocal Rank Fusion (RRF) on a litesearch store. RRF merges the two result lists by rank. The function returns the rows of litesearch's `search` in the same order. It is 1.2 to 9.5 times faster.

Python loads adhara as an SQLite extension. Swift links adhara as a static library with SQLite built in. A Swift app can search a store that Python wrote.

`RESULTS.md` has the measurements and the method. `DEV.md` has the build and release steps.

## Python

```sh
pip install adhara
```

```python
import apsw, adhara
db = adhara.load(apsw.Connection('vault.db'))
db.execute(f"create virtual table notes using fts5(body, tokenize='{adhara.TOKENIZE}')")
rows = db.execute("select id, score from adhara_hybrid('store', :q, :vec, 10)", dict(q='"krishna"*', vec=qvec)).fetchall()
```

- `adhara.load(conn)` loads the extension into an apsw or sqlite3 connection and returns the connection. Use one SQLite library in each process.
- `adhara.path()` returns the path of the extension file, for your own `load_extension` call.
- `fold_token`, `roman_fold`, `skeleton`, `forms` and `tokens` give the folds and the tokens without SQLite.

## SQL

```sql
select id, score, fts_rank, vec_rank, dist
from adhara_hybrid(store, q, qvec, k, filter, dtype, metric, ann)
```

| argument | meaning |
|---|---|
| `store` | a litesearch store: the table `store`, its FTS5 table `store_fts`, and an `embedding` column |
| `q` | an FTS5 query, or NULL for the vector search only |
| `qvec` | the query vector as bytes, or NULL for the full-text search only |
| `k` | the number of rows from each search, and the number of rows returned |
| `filter` | an SQL condition on the rows of the store, or NULL |
| `dtype` | `f16` (default), `f32` or `i8` |
| `metric` | `cosine` (default), `inner` or `sqeuclidean` |
| `ann` | 1 (default) to use the HNSW index of litesearch, 0 to compare the query with every vector |

HNSW is an approximate nearest-neighbour index. litesearch keeps it in a file beside the database. The vector search uses the HNSW index when `ann` is 1, `filter` is NULL and the index file is current. Otherwise the vector search compares the query with every vector. An HNSW search can miss a row that the exact search finds. litesearch's `search` uses the exact search by default (`ann=False`). Pass `ann` as 0 to get the same rows.

Join on `id` to get the row:

```sql
select s.content, h.score from adhara_hybrid('store', :q, :vec, 10) h cross join store s on s.rowid = h.id order by h.score desc
```

## Swift

Add adhara to the dependencies of your package:

```swift
.package(url: "https://github.com/vedicreader/adhara", from: "0.1.0")
```

```swift
import Adhara
let db = try Database(path: vaultPath, readonly: true)
let hits = try db.search(fts: anyOf(["krishna"]), vector: queryVector, k: 10)
```

The package downloads the compiled library from the GitHub release. It runs on macOS 13 and later, on Apple silicon and Intel.

The Swift library also registers the tokenizer name `simplify`. litesearch stores declare `porter simplify casefold 1 sanskrit unicodewords`. With this name, Swift reads those stores with the `adhara` tokenizer. The `adhara` tokens are the same as the tokens of that chain, except after a ZWJ emoji sequence (`RESULTS.md`). Open those stores read-only in Swift. A Swift write to such a store can leave a stale index entry for a row with that sequence. `simplify` with other arguments gives an error.

## Build and test

```sh
cargo test --release
maturin develop --release
pytest
scripts/xcframework.sh && swift test
python bench/bench_tok.py; python bench/bench_hybrid.py
```

`pytest` compares adhara with litesearch and apsw on the corpus of laghu. `scripts/xcframework.sh` builds `swift/AdharaCore.xcframework`. When that folder exists, the Swift package links it instead of the release download.

## Licence

MIT.
