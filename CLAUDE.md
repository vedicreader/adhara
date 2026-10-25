# Working in this repo

The Rust code in `src/` is the source. The Python package (`python/adhara`) and the Swift package (`swift/`, `Package.swift`) are thin layers over it. `scripts/gen_tables.py` writes `src/tables.rs`. Never edit `src/tables.rs` by hand.

## The one rule

adhara gives the results of litesearch. A faster method that changes a result is a bug. The tests in `tests/` compare adhara with litesearch directly.

- The folds equal `litesearch/sanskrit.py` on every word of the test corpus.
- The tokens equal the litesearch chain. One apsw quirk is the exception. `RESULTS.md` describes it.
- `adhara_hybrid` returns the rows of litesearch `search` in the same order. `ann` selects the HNSW search or the exact search, as `ann` does in litesearch.

## Three parts

- `fold.rs` is the port of `sanskrit.py`. It does not use SQLite.
- `tok.rs` is the FTS5 tokenizer `adhara`. Porter goes outside it (`porter adhara`), as litesearch puts porter outside its chain.
- `hybrid.rs` is the table function `adhara_hybrid`. It reads the litesearch schema: `<store>`, `<store>_fts` and `usearch_indices`.

## Builds

- `--features python` builds the Python module. The same file is the SQLite extension (`sqlite3_adhara_init`).
- `--features static` builds SQLite into the library, for Swift (`adhara_register`). This build also registers `simplify` for the litesearch chain. The extension does not, because apsw registers its own `simplify` on a Python connection.
- A change in tokens needs a new tokenizer name. Do not change the behaviour of an existing name. FTS5 tokenizes old content again on delete, so a changed token corrupts an index.

## Numbers

Each speed claim has a row in `RESULTS.md` and a script in `bench/`. Change the number before you change the claim.

## Licence

MIT.
