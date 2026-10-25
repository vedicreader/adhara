"Native FTS5 tokenizer and hybrid FTS + vector search for SQLite, as a loadable extension."
from ._adhara import TOKENIZE, fold_token, roman_fold, skeleton, strip_vedic, deva2ascii, forms, tokens
from . import _adhara

__all__ = ['TOKENIZE', 'ENTRY', 'path', 'load', 'fold_token', 'roman_fold', 'skeleton', 'strip_vedic', 'deva2ascii', 'forms', 'tokens']
ENTRY = 'sqlite3_adhara_init'

def path() -> str:
    "The extension's file; the Python module and the SQLite extension are one file."
    return _adhara.__file__

def load(conn):
    "Load adhara into an apsw or sqlite3 connection and return it. Load it into one SQLite library per process."
    en = getattr(conn, 'enableloadextension', None) or conn.enable_load_extension
    ld = getattr(conn, 'loadextension', None) or conn.load_extension
    en(True)
    try: ld(path())   # SQLite derives sqlite3_adhara_init from the file name `_adhara.*`
    finally: en(False)
    return conn
