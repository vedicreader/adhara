//! FTS5 tokenizer `adhara`: litesearch's `simplify casefold 1 sanskrit unicodewords` in one native pass.
//! Wrap it in porter (`porter adhara`) as litesearch wraps its chain, so the fold happens before the stem.
use crate::fold::{fold_token, roman_from};
use crate::tables::ext_pict;
use rusqlite::{Connection, Result, ffi};
use std::{cell::RefCell, collections::HashMap, ffi::{CStr, c_char, c_int, c_void}, ptr, rc::Rc};
use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};
use unicode_segmentation::UnicodeSegmentation;

/// A UAX #29 segment is a word when it holds a letter, a number, an emoji or a regional indicator, as apsw's `unicodewords`.
fn wordy(seg: &str) -> bool {
    seg.chars().any(|c| matches!(c.general_category_group(), GeneralCategoryGroup::Letter | GeneralCategoryGroup::Number)
        || ext_pict(c) || ('\u{1f1e6}'..='\u{1f1ff}').contains(&c))
}

/// The tokens one word indexes under, first one primary: as `sanskrit` then `simplify casefold 1` in litesearch.
pub fn forms(w: &str) -> Vec<String> {
    let mut l = vec![w.to_string()];
    let f = fold_token(w);
    let r = (f.chars().count() >= 4).then(|| roman_from(&f, false, false, false));
    if !f.is_empty() && !l.contains(&f) { l.push(f) }
    if let Some(r) = r { if !l.contains(&r) { l.push(r) } }
    let mut out: Vec<String> = Vec::with_capacity(l.len());
    for t in l {
        let c = if t.is_ascii() { t.to_ascii_lowercase() } else { caseless::default_case_fold_str(&t) };
        if !c.is_empty() && !out.contains(&c) { out.push(c) }
    }
    out
}

const CACHE_CAP: usize = 1 << 16;
thread_local! { static CACHE: RefCell<HashMap<Box<str>, Rc<[String]>>> = RefCell::new(HashMap::with_capacity(1024)) }

/// `forms`, remembered per thread for the last 65,536 distinct words, as litesearch's lru_cache on the folds.
fn cached(w: &str) -> Rc<[String]> {
    CACHE.with_borrow_mut(|m| {
        if let Some(f) = m.get(w) { return f.clone() }
        if m.len() >= CACHE_CAP { m.clear() }
        let f: Rc<[String]> = forms(w).into();
        m.insert(w.into(), f.clone());
        f
    })
}

/// Every word of `s` as (byte start, byte end, forms), offsets shifted by `at`.
pub fn tokens(s: &str, at: usize) -> impl Iterator<Item = (usize, usize, Vec<String>)> + '_ {
    s.split_word_bound_indices().filter(|(_, w)| wordy(w)).map(move |(i, w)| (at + i, at + i + w.len(), forms(w)))
}

type XToken = unsafe extern "C" fn(*mut c_void, c_int, *const c_char, c_int, c_int, c_int) -> c_int;

unsafe extern "C" fn x_create(_: *mut c_void, _: *mut *const c_char, _: c_int, out: *mut *mut ffi::Fts5Tokenizer) -> c_int {
    // stateless: any non-null pointer will do, and it is never dereferenced
    unsafe { *out = ptr::NonNull::<ffi::Fts5Tokenizer>::dangling().as_ptr() };
    ffi::SQLITE_OK
}

/// The words after `simplify` in litesearch's chain, `porter simplify casefold 1 sanskrit unicodewords`.
#[cfg(feature = "static")]
const CHAIN: [&str; 4] = ["casefold", "1", "sanskrit", "unicodewords"];

/// `simplify` as litesearch's chain names it: the same tokens as `adhara`, so a Swift reader opens a store Python built.
/// Other arguments are refused, since adhara does not reproduce them.
#[cfg(feature = "static")]
unsafe extern "C" fn x_create_chain(p: *mut c_void, az: *mut *const c_char, n: c_int, out: *mut *mut ffi::Fts5Tokenizer) -> c_int {
    let args = (0..n.max(0) as usize).map(|i| unsafe { CStr::from_ptr(*az.add(i)) }.to_bytes());
    if args.eq(CHAIN.iter().map(|s| s.as_bytes())) { unsafe { x_create(p, az, n, out) } } else { ffi::SQLITE_ERROR }
}

unsafe extern "C" fn x_delete(_: *mut ffi::Fts5Tokenizer) {}

unsafe extern "C" fn x_tokenize(_: *mut ffi::Fts5Tokenizer, ctx: *mut c_void, _flags: c_int, text: *const c_char, n: c_int,
                                x_token: Option<XToken>) -> c_int {
    let (Some(emit), false) = (x_token, text.is_null() || n <= 0) else { return ffi::SQLITE_OK };
    let b = unsafe { std::slice::from_raw_parts(text as *const u8, n as usize) };
    let mut at = 0;
    for chunk in b.utf8_chunks() {
        for (i, w) in chunk.valid().split_word_bound_indices().filter(|(_, w)| wordy(w)) {
            let (s, e, fs) = (at + i, at + i + w.len(), cached(w));
            for (j, t) in fs.iter().enumerate() {
                let fl = if j == 0 { 0 } else { ffi::FTS5_TOKEN_COLOCATED };
                let rc = unsafe { emit(ctx, fl, t.as_ptr() as *const c_char, t.len() as c_int, s as c_int, e as c_int) };
                if rc != ffi::SQLITE_OK { return rc }
            }
        }
        at += chunk.valid().len() + chunk.invalid().len();
    }
    ffi::SQLITE_OK
}

/// The connection's FTS5 API, fetched with `select fts5(?1)` as the FTS5 docs describe.
unsafe fn fts5_api(db: *mut ffi::sqlite3) -> Result<*mut ffi::fts5_api> {
    let (mut api, mut st): (*mut ffi::fts5_api, *mut ffi::sqlite3_stmt) = (ptr::null_mut(), ptr::null_mut());
    unsafe {
        if ffi::sqlite3_prepare_v2(db, c"select fts5(?1)".as_ptr(), -1, &mut st, ptr::null_mut()) != ffi::SQLITE_OK {
            return Err(rusqlite::Error::ModuleError("FTS5 is not compiled into this SQLite".into()));
        }
        ffi::sqlite3_bind_pointer(st, 1, &mut api as *mut _ as *mut c_void, c"fts5_api_ptr".as_ptr(), None);
        ffi::sqlite3_step(st);
        ffi::sqlite3_finalize(st);
    }
    if api.is_null() { Err(rusqlite::Error::ModuleError("no fts5_api on this connection".into())) } else { Ok(api) }
}

type XCreate = unsafe extern "C" fn(*mut c_void, *mut *const c_char, c_int, *mut *mut ffi::Fts5Tokenizer) -> c_int;

fn create(api: *mut ffi::fts5_api, name: &CStr, x: XCreate) -> Result<()> {
    let mut t = ffi::fts5_tokenizer { xCreate: Some(x), xDelete: Some(x_delete), xTokenize: Some(x_tokenize) };
    let rc = unsafe { ((*api).xCreateTokenizer.unwrap())(api, name.as_ptr(), ptr::null_mut(), &mut t, None) };
    if rc == ffi::SQLITE_OK { Ok(()) } else { Err(rusqlite::Error::SqliteFailure(ffi::Error::new(rc), Some("fts5 xCreateTokenizer".into()))) }
}

/// Register the `adhara` tokenizer on `db`. The static build also registers `simplify` for litesearch's chain;
/// the extension does not, since apsw registers its own `simplify` on a Python connection.
pub fn register(db: &Connection) -> Result<()> {
    let api = unsafe { fts5_api(db.handle())? };
    create(api, c"adhara", x_create)?;
    #[cfg(feature = "static")]
    create(api, c"simplify", x_create_chain)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forms_match_chain() {
        assert_eq!(forms("Bhīṣma"), ["bhīṣma", "bhisma", "bisma"]);
        assert_eq!(forms("STRASSE"), ["strasse"]);
        assert_eq!(forms("Straße"), ["strasse"]);
        let ws: Vec<_> = tokens("don't stop 🙂 ॐ", 0).map(|t| t.2[0].clone()).collect();
        assert_eq!(ws, ["don't", "stop", "🙂", "ॐ"]);
    }
}
