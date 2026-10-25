# adhara

adhara is the SQLite core under litesearch. The name is Sanskrit for "foundation". It is one Rust library that loads into SQLite and gives two things:

- `porter adhara`, an FTS5 tokenizer. It gives litesearch's tokens (`porter simplify casefold 1 sanskrit unicodewords`) 4.6 times faster. A query in Devanagari, IAST or plain English spelling finds the same word: `Bheeshma`, `Bhīṣma` and `भीष्म` all match.
- `adhara_hybrid`, one SQL call that runs the FTS5 leg, the vector leg and Reciprocal Rank Fusion on a litesearch store. It returns litesearch's rows in litesearch's order, 1.2 to 9.5 times faster.

The same code runs in Python (as a loadable extension for apsw or sqlite3) and in Swift (as a static library with SQLite built in). A Swift app can open a vault that Python wrote.

`RESULTS.md` has the numbers and the method.

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

- `adhara.load(conn)` loads the extension into an apsw or sqlite3 connection. Use one SQLite library for each process.
- `adhara.path()` is the file to give to `load_extension` yourself.
- `fold_token`, `roman_fold`, `skeleton`, `forms` and `tokens` give the folds and tokens without SQLite.

## SQL

```sql
select id, score, fts_rank, vec_rank, dist
from adhara_hybrid(store, q, qvec, k, filter, dtype, metric)
```

| argument | meaning |
|---|---|
| `store` | a litesearch store: the table `store`, its FTS5 table `store_fts`, an `embedding` column |
| `q` | an FTS5 query, or NULL for the vector leg only |
| `qvec` | the query vector as bytes, or NULL for the FTS leg only |
| `k` | rows from each leg and rows returned |
| `filter` | an SQL condition on the store's rows, or NULL |
| `dtype` | `f16` (default), `f32` or `i8` |
| `metric` | `cosine` (default), `inner` or `sqeuclidean` |

Without a filter, the vector leg uses litesearch's HNSW sidecar when it is fresh, else an exact scan. With a filter, the scan is exact. Join on `id` for the row:

```sql
select s.content, h.score from adhara_hybrid('store', :q, :vec, 10) h cross join store s on s.rowid = h.id order by h.score desc
```

## Swift

```swift
import Adhara
let db = try Database(path: vaultPath, readonly: true)
let hits = try db.search(fts: anyOf(["krishna"]), vector: queryVector, k: 10)
```

Build the binary target first with `scripts/xcframework.sh` (macOS, Rust). It makes `swift/AdharaCore.xcframework` for Apple silicon and Intel. Then `swift test`.

## Build and test

```sh
cargo test --release
maturin develop --release
pytest tests            # compares with litesearch, laghu's corpus and apsw
python bench/bench_tok.py; python bench/bench_hybrid.py
```

## Licence

MIT.
