"Shared bench helpers: texts, an encoder, a timer."
import time, statistics as st, numpy as np

def texts(n):
    "`n` distinct ~500-character chunks cut from laghu's offline corpus, each tagged so none repeats."
    import laghu
    s = '\n'.join(laghu.corpus(4000)); cs = [s[i:i + 500] for i in range(0, len(s) - 500, 500)]
    return [f'{cs[i % len(cs)]} r{i}' for i in range(n)]

def encoder(name='minishlab/potion-base-8M'):
    "f16 vectors from a model2vec model through laghu."
    import laghu
    m = laghu.load(name)
    return lambda xs: m.encode(xs).astype(np.float16)

def ms(f, n=100):
    "Median milliseconds of `f` over `n` calls."
    ts = []
    for _ in range(n): t = time.perf_counter(); f(); ts.append(time.perf_counter() - t)
    return st.median(ts) * 1e3
