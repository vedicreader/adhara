# Results

Numbers behind the claims in the README, with the method. Run on one 4-core x86_64 Linux machine, Python 3.13, apsw 3.53.4, litesearch 0.1.61, laghu 0.3.1.

## Tokenizer: `porter adhara` against litesearch's chain

`python bench/bench_tok.py`. 3,000 chunks of about 500 characters cut from `laghu.corpus(4000)`. Insert is one transaction into a bare FTS5 table. Query is 2,000 two-word OR queries, top 10 by bm25. `process_content` embeds with potion-base-8M through laghu and writes a litesearch store. Median of 3 runs.

| chain | insert rows/s | query /s | process_content chunks/s |
|---|---|---|---|
| `porter simplify casefold 1 sanskrit unicodewords` | 6,301 | 5,202 | 3,254 |
| `porter adhara` | 28,691 | 5,622 | 6,268 |
| `porter unicode61 remove_diacritics 2` | 40,983 | 5,864 | 7,315 |

`porter adhara` inserts 4.6 times faster and ingests 1.9 times faster end to end. `unicode61` is the ceiling: SQLite's own C tokenizer with no folding and one token per word, where adhara writes up to three.

The tokens are litesearch's. `tests/test_tok.py` runs both chains over `laghu.corpus()` (3,226 texts: English, Devanagari, IAST, noise, odd Unicode) plus Sanskrit in both scripts: 3,226 identical. On the 2,827 chunks of the repos' notebooks and docs, 2,826 are identical. The one difference is apsw's: it puts the space after a ZWJ emoji sequence into the word, which UAX #29 does not. Both sides use Unicode 17. The emoji test (Extended_Pictographic) is generated from apsw.

## Folds: Rust against sanskrit.py

`tests/test_fold.py`: `fold_token`, `strip_vedic`, `deva2ascii`, `skeleton` and `roman_fold` under all 8 flag settings, on every word of the corpus. No difference. The Python side raises on `roman_fold('²', numbers=True)`, since `'²'.isdigit()` but `int('²')` fails; adhara returns `'²'`. sanskrit.py's eight nukta consonant keys are two characters and never match, since `deva2ascii` reads one character at a time; adhara leaves them out.

## Hybrid search: `adhara_hybrid` against litesearch's `search`

`python bench/bench_hybrid.py`. A litesearch store on disk with an HNSW sidecar, built by litesearch: `n` chunks from `laghu.corpus(4000)` tagged to be distinct, potion-base-8M vectors in f16, metadata `{"g": i % 10}`. 20 three-word queries through `pre`, top 10. `where` is `json_extract(metadata,'$.g')=3`, which makes litesearch scan exactly. Median of 200 runs, 40 with `where`. Both sides return the row content.

| rows | search | litesearch ms | adhara ms | speedup | same top 10 |
|---|---|---|---|---|---|
| 2,000 | HNSW | 0.97 | 0.38 | 2.6x | 20/20 |
| 2,000 | `where` | 2.47 | 1.10 | 2.2x | 20/20 |
| 20,000 | HNSW | 3.24 | 2.43 | 1.3x | 20/20 |
| 20,000 | `where` | 20.89 | 4.23 | 4.9x | 20/20 |
| 200,000 | HNSW | 26.15 | 22.69 | 1.2x | 20/20 |
| 200,000 | `where` | 208.87 | 45.89 | 4.6x | 20/20 |

Without a filter, FTS5's bm25 over every match is the floor and is the same on both sides; this corpus repeats each chunk about 130 times at 200,000 rows, so the match lists are long. On 200,000 rows of the repos' own text (fewer repeats) the same comparison gave 7.33 against 6.21 ms with HNSW and 142.41 against 14.92 ms (9.5x) with `where`.

With a filter, litesearch scans every row in SQL. adhara ranks every vector in memory on all cores (f32 dot products, 16 lanes), then checks the filter row by row in distance order until it has `k`, and walks the FTS matches in rank order the same way.

`tests/test_hybrid.py` checks each leg and the fused list against litesearch for f16 and f32, with and without HNSW, with and without `where`, for cosine, inner and sqeuclidean. The FTS leg and the HNSW leg are identical. The exact vector leg has the same distance at every rank; where two rows tie within 1e-5, the two kernels can order them differently. A first try with SimSIMD's f16 kernels swapped rows 1e-4 apart (13 of 20 queries identical), so adhara uses plain f32 arithmetic.

## Costs

- Memory: each connection keeps a copy of a store's vectors, about 100 MB for 200,000 f16 rows of 256 dims, beside the HNSW index.
- Freshness: any write on the connection, or a commit by another, makes the next search read the vectors again.
- SQLite 3.45.3 or later. Ubuntu 24.04's system SQLite is 3.45.1; apsw bundles a newer one.
