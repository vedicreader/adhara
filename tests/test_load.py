"`adhara.load` finds the extension's file when the import system gives the module no `__file__`, as py2app's bundles do."
import apsw, adhara

def test_load_without_dunder_file(monkeypatch):
    monkeypatch.delattr(adhara._adhara, '__file__')
    assert adhara.path() == adhara._adhara.__spec__.origin
    c = adhara.load(apsw.Connection(':memory:'))
    c.execute("create virtual table t using fts5(x, tokenize='porter adhara')")
    c.execute("insert into t values ('Bhīṣma')")
    assert c.execute("select count(*) from t where t match 'Bheeshma'").fetchone()[0] == 1
