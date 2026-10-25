//! adhara: the SQLite core under litesearch and its Swift readers.
//! An FTS5 tokenizer (`porter adhara`) and `adhara_hybrid`, FTS5 + vector search fused by RRF in one call.
pub mod fold;
pub mod hybrid;
mod tables;
pub mod tok;
#[cfg(feature = "python")]
mod py;

use rusqlite::{Connection, Result};
#[cfg(any(feature = "ext", feature = "static"))]
use rusqlite::ffi;
use std::ffi::c_int;

/// The oldest SQLite adhara runs on.
pub const MIN_SQLITE: c_int = 3_045_003;

/// The tokenizer chain for new stores: porter outside, so the fold happens before the stem.
pub const TOKENIZE: &str = "porter adhara";

/// Register the tokenizer and `adhara_hybrid` on `db`.
pub fn register(db: &Connection) -> Result<()> {
    tok::register(db)?;
    hybrid::register(db)
}

/// # Safety
/// Called by SQLite's `load_extension`.
#[cfg(feature = "ext")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sqlite3_adhara_init(db: *mut ffi::sqlite3, err: *mut *mut std::ffi::c_char, api: *mut ffi::sqlite3_api_routines) -> c_int {
    // rusqlite's bindings need 3.45.3; on an older SQLite its init fails and then panics while reporting it
    unsafe {
        let Some(a) = api.as_ref() else { return ffi::SQLITE_ERROR };
        if a.libversion_number.map_or(0, |f| f()) < MIN_SQLITE {
            if let (false, Some(m)) = (err.is_null(), a.mprintf) { *err = m(c"adhara needs SQLite 3.45.3 or later".as_ptr()) }
            return ffi::SQLITE_ERROR;
        }
        Connection::extension_init2(db, err, api, |c| register(&c).map(|_| false))
    }
}

/// Register adhara on a connection from the SQLite built into this library; returns an SQLite result code.
/// # Safety
/// `db` must be an open connection from this library's SQLite.
#[cfg(feature = "static")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn adhara_register(db: *mut ffi::sqlite3) -> c_int {
    match unsafe { Connection::from_handle(db) }.and_then(|c| register(&c)) {
        Ok(()) => ffi::SQLITE_OK,
        Err(rusqlite::Error::SqliteFailure(e, _)) => e.extended_code,
        Err(_) => ffi::SQLITE_ERROR,
    }
}
