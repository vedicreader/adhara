/* adhara: register the `adhara` FTS5 tokenizer and `adhara_hybrid` on a connection from the SQLite built into libadhara. */
#ifndef ADHARA_H
#define ADHARA_H
#include "sqlite3.h"

/* SQLITE_OK, or an SQLite error code. `db` must come from this library's sqlite3_open_v2. */
int adhara_register(sqlite3 *db);

#endif
