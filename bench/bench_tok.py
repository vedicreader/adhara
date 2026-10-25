"Insert and query speed of litesearch's Python chain, `porter adhara` and SQLite's C `unicode61`, raw and through process_content."
import time, re, statistics as st, numpy as np, adhara
from litesearch import database, process_content
from litesearch.core import _FTS_TOKENIZE
from common import texts, encoder

def raw(ch, words, tok):
    "Rows inserted and queries run per second on a bare FTS5 table."
    db = adhara.load(database(':memory:').conn.raw)
    db.execute(f"create virtual table f using fts5(content, tokenize='{tok}')")
    t = time.perf_counter()
    with db: db.executemany('insert into f(content) values (?)', [(x,) for x in ch])
    ins = len(ch) / (time.perf_counter() - t)
    qs = [f'"{a}" OR "{b}"' for a, b in np.random.default_rng(0).choice(words, (2000, 2))]
    t = time.perf_counter()
    for q in qs: db.execute('select rowid from f where f match ? order by rank limit 10', (q,)).fetchall()
    return ins, len(qs) / (time.perf_counter() - t)

def e2e(ch, emb, tok):
    "Chunks per second through process_content, embedding included."
    db = database(':memory:'); adhara.load(db.conn.raw); s = db.get_store(hash=True, tokenize=tok)
    t = time.perf_counter(); process_content(s, [dict(content=x) for x in ch], emb_fn=emb)
    return len(ch) / (time.perf_counter() - t)

if __name__ == '__main__':
    ch, emb = texts(3000), encoder()
    words = sorted({w for x in ch for w in re.findall(r'[A-Za-z]{5,}', x)})
    print('| chain | insert rows/s | query /s | process_content chunks/s |\n|---|---|---|---|')
    for tok in [_FTS_TOKENIZE, adhara.TOKENIZE, 'porter unicode61 remove_diacritics 2']:
        r = [raw(ch, words, tok) for _ in range(3)]; e = st.median(e2e(ch, emb, tok) for _ in range(3))
        print(f'| `{tok}` | {st.median(x[0] for x in r):,.0f} | {st.median(x[1] for x in r):,.0f} | {e:,.0f} |')
