"`porter adhara` gives the tokens of litesearch's Python chain, apart from one apsw quirk."
import re, apsw, pytest, adhara
core = pytest.importorskip('litesearch.core')
from litesearch.sanskrit import register_sanskrit
from apsw.fts5 import register_tokenizers, map_tokenizers

ZWJ_SPACE = re.compile('‍\\S+ ')   # apsw puts the space after a ZWJ emoji sequence into the word; UAX #29 does not

@pytest.fixture(scope='module')
def conn():
    c = apsw.Connection(':memory:')
    register_tokenizers(c, map_tokenizers); register_sanskrit(c); adhara.load(c)
    return c

def toks(c, chain, t):
    p = chain.split()
    return list(c.fts5_tokenizer(p[0], p[1:])(t.encode(), apsw.FTS5_TOKENIZE_DOCUMENT, None))

def test_chain(conn, texts):
    bad = [t for t in texts if toks(conn, core._FTS_TOKENIZE, t) != toks(conn, adhara.TOKENIZE, t)]
    assert all(ZWJ_SPACE.search(t) for t in bad), bad[:3]
    assert len(bad) <= len(texts) // 1000

def test_query_mode(conn):
    q = lambda chain: list(conn.fts5_tokenizer(*(lambda p: (p[0], p[1:]))(chain.split()))('Bhīṣma Krishna'.encode(), apsw.FTS5_TOKENIZE_QUERY, None))
    assert q(core._FTS_TOKENIZE) == q(adhara.TOKENIZE)

def test_match_across_scripts(conn):
    conn.execute(f"create virtual table f using fts5(t, tokenize='{adhara.TOKENIZE}')")
    conn.executemany('insert into f(t) values (?)', [('भीष्म spoke',), ('Kṛṣṇa smiled',), ('nothing here',)])
    hit = lambda q: [r[0] for r in conn.execute('select rowid from f where f match ? order by rowid', (q,))]
    assert hit('Bheeshma') == [1] and hit('bhisma') == [1] and hit('Krishna') == [2] and hit('कृष्ण') == [2]

def test_bad_utf8(conn):
    out = list(conn.fts5_tokenizer('adhara', [])(b'ab \xff cd', apsw.FTS5_TOKENIZE_DOCUMENT, None))
    assert [o[2] for o in out] == ['ab', 'cd'] and out[1][:2] == (5, 7)
