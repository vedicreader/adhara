"`adhara_hybrid` returns litesearch's `search` rows in litesearch's order, leg by leg and fused."
import re, numpy as np, pytest, adhara
ls = pytest.importorskip('litesearch')
from litesearch.utils import hash_embed
from litesearch.data import pre

W = 'json_extract(metadata, "$.g") = 3'
RARE = 'rowid % 97 = 0'   # so few rows pass that adhara reads every passing rowid at once
SQL = 'select id, dist from adhara_hybrid(?, ?, ?, ?, ?, ?, ?)'

def mk(path, name, ann, dtype, texts):
    "A litesearch store of `texts` with hash vectors in `dtype`, and adhara loaded on its connection."
    db = ls.database(str(path)); s = db.get_store(name, hash=True, ann=ann, ndim=256, dtype=dtype)
    E = hash_embed(texts, dtype=dtype)
    ls.process_content(s, [dict(content=t, embedding=e.tobytes(), metadata=f'{{"g": {i % 5}}}') for i, (t, e) in enumerate(zip(texts, E))], embed=False)
    if ann: s.rebuild_index(dtype=dtype)
    adhara.load(db.conn.raw)
    return db

@pytest.fixture(scope='module')
def docs(texts): return sorted({t for t in texts if len(t) >= 20})

@pytest.fixture(scope='module')
def queries(docs):
    rng = np.random.default_rng(0); ws = sorted({w for t in docs for w in re.findall(r'[A-Za-z]{4,}', t)})
    return [' '.join(rng.choice(ws, 2)) for _ in range(25)]

@pytest.fixture(scope='module')
def stores(tmp_path_factory, docs):
    p = tmp_path_factory.mktemp('db')
    return {(ann, dt): mk(p/f'{ann}{dt.__name__}.db', 'store', ann, dt, docs) for ann in (False, True) for dt in (np.float16, np.float32)}

def ours(db, q, e, k, where=None, dtype='f16', metric='cosine'):
    return [r['id'] for r in db.q(SQL, ['store', q, e, k, where, dtype, metric])]

def emb(q, dt): return hash_embed([q], dtype=dt)[0].tobytes()

def vec_leg(db, e, k, where, sfx, metric='cosine'): return [(r['id'], r['dist']) for r in db.q(SQL, ['store', None, e, k, where, sfx, metric])]

def same_up_to_ties(want, got, eps=1e-5):
    "Same distance at every rank; ids may differ only where two kernels break a tie differently."
    return len(want) == len(got) and all(abs(a[1] - b[1]) < eps for a, b in zip(want, got))

@pytest.mark.parametrize('ann', [False, True])
@pytest.mark.parametrize('dt', [np.float16, np.float32])
@pytest.mark.parametrize('where', [None, W, RARE])
def test_legs_and_fused(stores, queries, ann, dt, where):
    db, sfx, ties = stores[(ann, dt)], 'f16' if dt == np.float16 else 'f32', 0
    for q in queries:
        e = emb(q, dt)
        legs = db.search(q, e, columns=['rowid'], limit=10, ann=ann, where=where, dtype=dt, rrf=False)
        assert ours(db, pre(q), None, 10, where, sfx) == [r['rowid'] for r in legs['fts']]
        want, got = [(r['rowid'], r['_dist']) for r in legs['vec']], vec_leg(db, e, 10, where, sfx)
        assert same_up_to_ties(want, got)
        if want != [(i, pytest.approx(d, abs=1e-5)) for i, d in got]: ties += 1; continue
        fused = db.search(q, e, columns=['rowid'], limit=10, ann=ann, where=where, dtype=dt)
        assert ours(db, pre(q), e, 10, where, sfx) == [r['rowid'] for r in fused]
    assert ties <= len(queries) // 5

@pytest.mark.parametrize('metric', ['cosine', 'inner', 'sqeuclidean'])
def test_metrics(stores, queries, metric):
    db = stores[(False, np.float32)]
    for q in queries:
        e = emb(q, np.float32)
        want = [(r['rowid'], r['_dist']) for r in db.t.store.vec_search(e, ['rowid'], emb_metric=metric, dtype=np.float32, limit=10)]
        assert same_up_to_ties(want, vec_leg(db, e, 10, None, 'f32', metric))

def test_distances(stores, queries):
    db = stores[(False, np.float16)]; e = emb(queries[0], np.float16)
    want = {r['rowid']: r['_dist'] for r in db.t.store.vec_search(e, ['rowid'], limit=10)}
    got = {r['id']: r['dist'] for r in db.q(SQL, ['store', None, e, 10, None, 'f16', 'cosine'])}
    assert want.keys() == got.keys() and all(abs(want[k] - got[k]) < 1e-3 for k in want)

def test_sees_writes(tmp_path, docs):
    db = mk(tmp_path/'w.db', 'store', False, np.float16, docs[:200])
    t = 'zyzzyva quokka unique words'; e = emb(t, np.float16)
    assert t not in [r['content'] for r in db.q('select content from store where rowid in (select id from adhara_hybrid(?, ?, ?, 5))', ['store', pre(t), e])]
    ls.process_content(db.t.store, [dict(content=t, embedding=e)], embed=False)
    top = db.q('select s.content from adhara_hybrid(?, ?, ?, 1) h cross join store s on s.rowid = h.id', ['store', pre(t), e])
    assert top[0]['content'] == t

def test_ann_switch(tmp_path, docs):
    "`ann=0` scans exactly, as litesearch's `ann=False`, and finds a row the HNSW sidecar does not hold."
    db = mk(tmp_path/'a.db', 'store', True, np.float16, docs[:500])
    t = 'zyzzyva quokka unique words'; e = emb(t, np.float16)
    db.t.store.insert(dict(content=t, embedding=e))   # written after the index was built, so the sidecar misses it
    leg = lambda ann: [r['rowid'] for r in db.search(t, e, columns=['rowid'], limit=5, ann=ann, rrf=False)['vec']]
    ann_sql = 'select id from adhara_hybrid(?, ?, ?, ?, ?, ?, ?, ?)'
    ours = lambda ann: [r['id'] for r in db.q(ann_sql, ['store', None, e, 5, None, 'f16', 'cosine', ann])]
    assert ours(0) == leg(False) and ours(0)[0] == db.q('select max(rowid) m from store')[0]['m']
    assert ours(1) == leg(True) == [r['id'] for r in db.q('select id from adhara_hybrid(?, ?, ?, 5)', ['store', None, e])]

def test_ann_skips_deleted(tmp_path, docs):
    "The HNSW search drops a key whose row is gone and fills its place, as litesearch's `ann_search` does."
    db = mk(tmp_path/'d.db', 'store', True, np.float16, docs[:500])
    e = db.q('select embedding from store where rowid=7')[0]['embedding']
    db.execute('delete from store where rowid=7')   # the sidecar still holds key 7
    want = [r['rowid'] for r in db.search('x', e, columns=['rowid'], limit=5, ann=True, rrf=False)['vec']]
    assert [r['id'] for r in db.q('select id from adhara_hybrid(?, ?, ?, 5)', ['store', None, e])] == want and 7 not in want

def test_errors(stores):
    db = stores[(False, np.float16)]
    with pytest.raises(Exception, match='bytes'): db.q(SQL, ['store', None, b'\x00' * 6, 5, None, 'f16', 'cosine'])
    with pytest.raises(Exception, match='dtype'): db.q(SQL, ['store', None, b'\x00' * 512, 5, None, 'f8', 'cosine'])
    with pytest.raises(Exception, match='metric'): db.q(SQL, ['store', None, b'\x00' * 512, 5, None, 'f16', 'hamming'])

def test_stdlib_sqlite3(tmp_path):
    "The same file loads into Python's own sqlite3 in a fresh process."
    import subprocess, sys, sqlite3
    if sqlite3.sqlite_version_info < (3, 45, 3): pytest.skip(f'stdlib SQLite {sqlite3.sqlite_version} is older than 3.45.3')
    code = f'''import sqlite3, adhara
c = adhara.load(sqlite3.connect(":memory:"))
c.execute("create virtual table f using fts5(t, tokenize='{adhara.TOKENIZE}')")
c.execute("insert into f values ('भीष्म spoke')")
print(c.execute("select count(*) from f where f match 'Bheeshma'").fetchone()[0])'''
    r = subprocess.run([sys.executable, '-c', code], capture_output=True, text=True)
    if 'enable_load_extension' in r.stderr: pytest.skip("this Python's sqlite3 cannot load extensions")
    assert r.stdout.strip() == '1', r.stderr

def test_old_sqlite_refused():
    "An SQLite older than 3.45.3 gets an error, not a crash."
    import sqlite3
    if sqlite3.sqlite_version_info >= (3, 45, 3): pytest.skip('stdlib SQLite is new enough')
    c = sqlite3.connect(':memory:'); c.enable_load_extension(True)
    with pytest.raises(sqlite3.OperationalError, match='3.45.3'): c.load_extension(adhara.path())
