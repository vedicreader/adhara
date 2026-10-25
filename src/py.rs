//! The Python module `adhara._adhara`; its file is also the SQLite extension.
use pyo3::prelude::*;

#[pyfunction]
fn fold_token(s: &str) -> String { crate::fold::fold_token(s) }

#[pyfunction]
#[pyo3(signature = (s, doubles=false, endings=false, numbers=false))]
fn roman_fold(s: &str, doubles: bool, endings: bool, numbers: bool) -> String { crate::fold::roman_fold(s, doubles, endings, numbers) }

#[pyfunction]
fn skeleton(k: &str) -> String { crate::fold::skeleton(k) }

#[pyfunction]
fn strip_vedic(s: &str) -> String { crate::fold::strip_vedic(s) }

#[pyfunction]
fn deva2ascii(s: &str) -> String { crate::fold::deva2ascii(s) }

/// The tokens `porter adhara` indexes a word under, before porter.
#[pyfunction]
fn forms(w: &str) -> Vec<String> { crate::tok::forms(w) }

/// (byte start, byte end, forms) for each word of `text`.
#[pyfunction]
fn tokens(text: &str) -> Vec<(usize, usize, Vec<String>)> { crate::tok::tokens(text, 0).collect() }

#[pymodule]
fn _adhara(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(fold_token, m)?)?;
    m.add_function(wrap_pyfunction!(roman_fold, m)?)?;
    m.add_function(wrap_pyfunction!(skeleton, m)?)?;
    m.add_function(wrap_pyfunction!(strip_vedic, m)?)?;
    m.add_function(wrap_pyfunction!(deva2ascii, m)?)?;
    m.add_function(wrap_pyfunction!(forms, m)?)?;
    m.add_function(wrap_pyfunction!(tokens, m)?)?;
    m.add("TOKENIZE", crate::TOKENIZE)?;
    Ok(())
}
