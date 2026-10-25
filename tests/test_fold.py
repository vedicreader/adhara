"The Rust folds equal litesearch's sanskrit.py on every word of the corpus."
import itertools, pytest, adhara
sk = pytest.importorskip('litesearch.sanskrit')

def safe(g):
    "`g`, or None where it raises: litesearch's `roman_fold('²', numbers=True)` raises, since '²'.isdigit() but int('²') fails."
    def h(x):
        try: return g(x)
        except ValueError: return None
    return h

def diffs(f, g, xs): return [(x, f(x), y) for x in xs if (y := safe(g)(x)) is not None and f(x) != y]

@pytest.mark.parametrize('name', ['fold_token', 'strip_vedic', 'deva2ascii'])
def test_same(words, name): assert not diffs(getattr(adhara, name), getattr(sk, name), words)

@pytest.mark.parametrize('flags', list(itertools.product([False, True], repeat=3)))
def test_roman_fold(words, flags):
    sk.roman_fold.cache_clear()
    assert not diffs(lambda w: adhara.roman_fold(w, *flags), lambda w: sk.roman_fold(w, *flags), words)

def test_skeleton(words):
    ks = [sk.fold_token(w) for w in words]
    assert not diffs(adhara.skeleton, sk.skeleton, ks)

def test_known():
    assert adhara.roman_fold('Bheeshma') == adhara.roman_fold('भीष्म') == 'bisma'
    assert adhara.skeleton('saundarya') == adhara.skeleton('soundarya') == 'sndr'
