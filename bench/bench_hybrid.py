"`adhara_hybrid` against litesearch's `search` on one on-disk store: HNSW, and a `where` filter that forces litesearch's exact scan."
import sys, os, re, tempfile, numpy as np, adhara
from litesearch import database, process_content
from litesearch.data import pre
from common import texts, encoder, ms

W = "json_extract(metadata,'$.g')=3"
SQL = 'select s.rowid as rowid, s.content, h.score from adhara_hybrid(?,?,?,?,?) h cross join store s on s.rowid=h.id order by h.score desc'

def run(n, emb, d):
    p = f'{d}/s{n}.db'; fresh = not os.path.exists(p)
    db = database(p); s = db.get_store(hash=True, ann=True, ndim=256)
    if fresh:
        ch = texts(n)
        process_content(s, [dict(content=x, embedding=e.tobytes(), metadata=f'{{"g":{i % 10}}}') for i, (x, e) in enumerate(zip(ch, emb(ch)))], embed=False)
        s.rebuild_index()
    adhara.load(db.conn.raw)
    words = sorted({w for (c,) in db.conn.execute('select content from store limit 2000') for w in re.findall(r'[A-Za-z]{5,}', c)})
    rng = np.random.default_rng(0); qs = [' '.join(rng.choice(words, 3)) for _ in range(20)]; es = [emb([q])[0].tobytes() for q in qs]
    lit = lambda i, w=None: [r['rowid'] for r in db.search(qs[i], es[i], columns=['content'], limit=10, ann=True, where=w)]
    ada = lambda i, w=None: [r['rowid'] for r in db.q(SQL, ['store', pre(qs[i]), es[i], 10, w])]
    same = lambda w: sum(lit(i, w) == ada(i, w) for i in range(20))
    it = iter(range(10**9)); nx = lambda: next(it) % 20
    ada(0); ada(0, W)
    return dict(n=n, same=same(None), same_w=same(W), lit=ms(lambda: lit(nx()), 200), ada=ms(lambda: ada(nx()), 200),
                lit_w=ms(lambda: lit(nx(), W), 40), ada_w=ms(lambda: ada(nx(), W), 40))

if __name__ == '__main__':
    emb, d = encoder(), tempfile.mkdtemp()
    print('| rows | search | litesearch ms | adhara ms | speedup | same top 10 |\n|---|---|---|---|---|---|')
    for n in map(int, sys.argv[1:] or [2000, 20000, 200000]):
        r = run(n, emb, d)
        print(f"| {n:,} | HNSW | {r['lit']:.2f} | {r['ada']:.2f} | {r['lit']/r['ada']:.1f}x | {r['same']}/20 |")
        print(f"| {n:,} | `where` | {r['lit_w']:.2f} | {r['ada_w']:.2f} | {r['lit_w']/r['ada_w']:.1f}x | {r['same_w']}/20 |")
