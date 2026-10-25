# Results

This file has the measurements behind the claims in the README, and the method for each.

| setting | value |
|---|---|
| machine | one 4-core x86_64 Linux machine |
| Python | 3.13 |
| apsw | 3.53.4 |
| litesearch | 0.1.61 |
| laghu | 0.3.1 |

## Tokenizer: `porter adhara` against the litesearch chain

Run `python bench/bench_tok.py`. The texts are chunks of about 500 characters, cut from `laghu.corpus(4000)`. The insert test writes all chunks into a bare FTS5 table in one transaction. The query test runs two-word OR queries and reads the top 10 by bm25. The `process_content` test embeds with potion-base-8M through laghu and writes a litesearch store. Each number is the median of 3 runs.

| test | size |
|---|---|
| chunks | 3,000 |
| queries | 2,000 |

| chain | insert rows/s | query /s | process_content chunks/s |
|---|---|---|---|
| `porter simplify casefold 1 sanskrit unicodewords` | 6,301 | 5,202 | 3,254 |
| `porter adhara` | 28,691 | 5,622 | 6,268 |
| `porter unicode61 remove_diacritics 2` | 40,983 | 5,864 | 7,315 |

| `porter adhara` against the litesearch chain | speedup |
|---|---|
| insert | 4.6x |
| `process_content`, end to end | 1.9x |

`unicode61` is the upper limit. It is the C tokenizer of SQLite. It does no folding and writes one token for each word. adhara writes up to three tokens for each word.

The tokens are the tokens of litesearch. `tests/test_tok.py` runs both chains over `laghu.corpus()` and over Sanskrit in both scripts. The corpus has English, Devanagari, IAST, noise and unusual Unicode.

| texts | count | identical tokens |
|---|---|---|
| `laghu.corpus()` | 3,226 | 3,226 |
| chunks of the notebooks and docs of the repos | 2,827 | 2,826 |

The one difference comes from apsw. apsw puts the space after a ZWJ emoji sequence into the word. UAX #29 does not. Both sides use Unicode 17. The emoji test (`Extended_Pictographic`) is generated from apsw.

## Folds: Rust against `sanskrit.py`

`tests/test_fold.py` runs `fold_token`, `strip_vedic`, `deva2ascii`, `skeleton` and `roman_fold` on every word of the corpus. It runs `roman_fold` under all 8 flag settings. The Rust and Python results have no difference.

There are two known gaps:

- `roman_fold('²', numbers=True)` raises an error in Python, because `'²'.isdigit()` is true and `int('²')` fails. adhara returns `'²'`.
- `sanskrit.py` has eight nukta consonant keys of two characters. They never match, because `deva2ascii` reads one character at a time. adhara does not include them.

## Hybrid search: `adhara_hybrid` against litesearch `search`

Run `python bench/bench_hybrid.py`. litesearch builds a store on disk with an HNSW index file. The store has `n` chunks from `laghu.corpus(4000)`, each tagged to be distinct. The vectors are potion-base-8M in f16. The metadata is `{"g": i % 10}`. The queries have three words each and go through `pre`. Each query reads the top 10. The `where` rows use `json_extract(metadata,'$.g')=3`. With that filter, litesearch does an exact search. Both sides return the content of each row.

| setting | value |
|---|---|
| queries | 20 |
| runs, no filter (median) | 200 |
| runs, `where` (median) | 40 |

| corpus | rows | search | litesearch ms | adhara ms | speedup | same top 10 |
|---|---|---|---|---|---|---|
| laghu | 2,000 | HNSW | 0.97 | 0.38 | 2.6x | 20/20 |
| laghu | 2,000 | `where` | 2.47 | 1.10 | 2.2x | 20/20 |
| laghu | 20,000 | HNSW | 3.24 | 2.43 | 1.3x | 20/20 |
| laghu | 20,000 | `where` | 20.89 | 4.23 | 4.9x | 20/20 |
| laghu | 200,000 | HNSW | 26.15 | 22.69 | 1.2x | 20/20 |
| laghu | 200,000 | `where` | 208.87 | 45.89 | 4.6x | 20/20 |
| repos' own text | 200,000 | HNSW | 7.33 | 6.21 | 1.2x | not recorded |
| repos' own text | 200,000 | `where` | 142.41 | 14.92 | 9.5x | not recorded |

Without a filter, the time is mostly bm25 over every FTS5 match. That part is the same on both sides. At 200,000 rows the laghu corpus repeats each chunk about 130 times. Each query matches many rows. The text of the repos has fewer repeats.

With a filter, litesearch scans every row in SQL. adhara ranks every vector in memory on all cores, with f32 dot products in 16 lanes. Then it checks the filter on one window of candidates at a time, in distance order, until it has `k` rows. The first window holds `2k` candidates, at least 16. Each later window is four times larger. One SQL statement checks a window: the candidates are a `VALUES` table, and `CROSS JOIN` makes SQLite look them up by rowid, so a window costs its own size. When a window would hold more than 1/128 of the rows (at least 1,024), adhara reads every passing rowid once and checks the rest in memory. It reads the FTS5 matches in rank order the same way.

The table shows one query on a snapshot of a vishalakshi vault: 181,290 chunks of 256-dimension f16 vectors, k = 30, on a 14-core arm64 Mac. The filters are vishalakshi's document-kind filters and a page filter. Each number is the mean of 10 runs after one warm-up run, from three repeats. `bench/legs.py` times each search with the json filter of `bench_hybrid.py`.

| filter | rows that pass | filter alone | litesearch scan | adhara |
|---|---|---|---|---|
| one document kind | 6,727 | 1.3 ms | 3.9 ms | 7.3 to 7.7 ms |
| three document kinds | 78,710 | 13.6 ms | 44.8 ms | 3.0 to 3.3 ms |
| `page <= 2` | 125,107 | 70.0 ms | 82.5 ms | 3.0 to 3.1 ms |

A filter that few rows pass is slower in adhara. SQLite reads only those rows through an index. adhara learns that the filter is selective only when its windows come back almost empty.

adhara reads the HNSW index file the first time a search uses it, not each time it reads the vectors.

`tests/test_hybrid.py` compares each search and the fused list with litesearch. It covers f16 and f32, HNSW and exact search, with and without `where`, and the metrics cosine, inner and sqeuclidean. The full-text results are identical. The HNSW results are identical. The exact results have the same distance at every rank. Two rows can tie within 1e-5. Then the two kernels can put them in a different order.

adhara uses plain f32 arithmetic. A first version used the f16 kernels of SimSIMD. Those kernels changed the order of rows 1e-4 apart.

| kernel | queries with an identical top 10 |
|---|---|
| SimSIMD f16 | 13 of 20 |

## Costs

| cost | value |
|---|---|
| memory for each connection, 200,000 f16 rows of 256 dimensions | about 100 MB, beside the HNSW index |
| oldest SQLite | 3.45.3 |
| system SQLite of Ubuntu 24.04 | 3.45.1 (apsw bundles a newer SQLite) |

Each connection keeps its own copy of the vectors of a store. A write on the connection, or a commit by another connection, makes the next search read the vectors again.
