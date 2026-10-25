"Time each leg of adhara_hybrid with the benchmark's json filter, on the benchmark store of `n` rows kept in `dir`."
import sys, re, numpy as np
sys.path.insert(0, 'bench')
from bench_hybrid import run, W
from common import encoder, ms
from litesearch import database
from litesearch.data import pre

n, d = int(sys.argv[1]), sys.argv[2]
emb = encoder()
run(n, emb, d)   # builds the store once; later runs reuse it
db = database(f'{d}/s{n}.db'); import adhara; adhara.load(db.conn.raw)
words = sorted({w for (c,) in db.conn.execute('select content from store limit 2000') for w in re.findall(r'[A-Za-z]{5,}', c)})
rng = np.random.default_rng(0); qs = [' '.join(rng.choice(words, 3)) for _ in range(20)]; es = [emb([q])[0].tobytes() for q in qs]
S = 'select id from adhara_hybrid(?, ?, ?, 10, ?)'
it = iter(range(10**9)); nx = lambda: next(it) % 20
for name, f in [('fts leg', lambda i: db.q(S, ['store', pre(qs[i]), None, W])), ('vec leg', lambda i: db.q(S, ['store', None, es[i], W])),
                ('both', lambda i: db.q(S, ['store', pre(qs[i]), es[i], W])), ('filter alone', lambda i: db.q(f'select rowid from store where {W}'))]:
    f(0); print(f'{name:12} {ms(lambda: f(nx()), 40):.2f} ms')