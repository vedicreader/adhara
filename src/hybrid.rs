//! `adhara_hybrid(store, q, qvec, k, filter, dtype, metric)`: FTS5 leg, vector leg and RRF in one SQLite call.
//! Same rows and order as litesearch's `search` on a litesearch store: `<store>`, `<store>_fts`, `usearch_indices`.
use half::{f16, slice::HalfFloatSliceExt};
use rusqlite::vtab::{Context, Filters, IndexConstraintOp, IndexInfo, Module, VTab, VTabConnection, VTabCursor};
use rusqlite::{Connection, OptionalExtension, Result, ffi};
use std::{borrow::Cow, cell::RefCell, collections::HashMap, ffi::{CStr, c_int}, rc::Rc};
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

fn err(e: impl ToString) -> rusqlite::Error { rusqlite::Error::ModuleError(e.to_string()) }
fn q(s: &str) -> String { format!("\"{}\"", s.replace('"', "\"\"")) }

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Dtype { F16, F32, I8 }
impl Dtype {
    fn parse(s: &str) -> Result<Self> { Ok(match s { "f16" => Self::F16, "f32" => Self::F32, "i8" => Self::I8, _ => return Err(err(format!("dtype {s:?}: use f16, f32 or i8"))) }) }
    fn size(self) -> usize { match self { Self::F16 => 2, Self::F32 => 4, Self::I8 => 1 } }
    fn scalar(self) -> ScalarKind { match self { Self::F16 => ScalarKind::F16, Self::F32 => ScalarKind::F32, Self::I8 => ScalarKind::I8 } }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Metric { Cos, Ip, L2sq }
impl Metric {
    fn parse(s: &str) -> Result<Self> {
        Ok(match s { "cosine" | "cos" => Self::Cos, "inner" | "ip" => Self::Ip, "sqeuclidean" | "l2sq" => Self::L2sq, _ => return Err(err(format!("metric {s:?}: use cosine, inner or sqeuclidean"))) })
    }
    fn kind(self) -> MetricKind { match self { Self::Cos => MetricKind::Cos, Self::Ip => MetricKind::IP, Self::L2sq => MetricKind::L2sq } }
}

/// Raw embedding bytes as f32.
pub fn to_f32(b: &[u8], dt: Dtype, out: &mut [f32]) {
    match dt {
        Dtype::F16 => { let h: Vec<f16> = b.chunks_exact(2).map(|x| f16::from_le_bytes([x[0], x[1]])).collect(); h.convert_to_f32_slice(out) }
        Dtype::F32 => for (o, x) in out.iter_mut().zip(b.chunks_exact(4)) { *o = f32::from_le_bytes([x[0], x[1], x[2], x[3]]) },
        Dtype::I8 => for (o, &x) in out.iter_mut().zip(b) { *o = x as i8 as f32 },
    }
}

/// f32 dot product in 16 lanes, so it vectorizes.
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    let mut acc = [0f32; 16];
    for (x, y) in a.chunks_exact(16).zip(b.chunks_exact(16)) { for j in 0..16 { acc[j] += x[j] * y[j] } }
    let t = a.len() / 16 * 16;
    acc.iter().sum::<f32>() + a[t..].iter().zip(&b[t..]).map(|(x, y)| x * y).sum::<f32>()
}

fn l2sq(a: &[f32], b: &[f32]) -> f32 {
    let mut acc = [0f32; 16];
    for (x, y) in a.chunks_exact(16).zip(b.chunks_exact(16)) { for j in 0..16 { let d = x[j] - y[j]; acc[j] += d * d } }
    let t = a.len() / 16 * 16;
    acc.iter().sum::<f32>() + a[t..].iter().zip(&b[t..]).map(|(x, y)| (x - y) * (x - y)).sum::<f32>()
}

/// A store's vectors, raw, with norms, plus the HNSW sidecar when litesearch saved a fresh one.
pub struct Vecs { ids: Vec<i64>, raw: Vec<u8>, norms: Vec<f32>, dim: usize, dt: Dtype, ann: Option<Index>, stamp: (u64, i64) }

impl Vecs {
    fn row(&self, i: usize, buf: &mut [f32]) { let w = self.dim * self.dt.size(); to_f32(&self.raw[i * w..(i + 1) * w], self.dt, buf) }

    /// Distance from `qv` to every row, as (distance, row position), on all cores; usearch's definitions.
    fn dists(&self, qv: &[f32], m: Metric) -> Vec<(f32, u32)> {
        let (n, qn) = (self.ids.len(), dot(qv, qv).sqrt());
        let t = std::thread::available_parallelism().map_or(1, |x| x.get()).min(n / 16_384 + 1);
        let step = n.div_ceil(t).max(1);
        let mut out = vec![(0f32, 0u32); n];
        std::thread::scope(|sc| for (j, o) in out.chunks_mut(step).enumerate() {
            sc.spawn(move || {
                let mut buf = vec![0f32; self.dim];
                for (i, d) in o.iter_mut().enumerate() {
                    let at = j * step + i;
                    self.row(at, &mut buf);
                    let v = match m {
                        Metric::Cos => { let nn = self.norms[at] * qn; if nn > 0. { 1. - dot(&buf, qv) / nn } else { 1. } }
                        Metric::Ip => 1. - dot(&buf, qv),
                        Metric::L2sq => l2sq(&buf, qv),
                    };
                    *d = (v, at as u32);
                }
            });
        });
        out
    }

    /// Exact `k` nearest rowids that pass `keep`, widening the candidate pool until `k` pass; ties by rowid.
    fn exact(&self, qv: &[f32], m: Metric, k: usize, mut keep: impl FnMut(i64) -> Result<bool>) -> Result<Vec<(i64, f32)>> {
        let mut d = self.dists(qv, m);
        let cmp = |a: &(f32, u32), b: &(f32, u32)| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1));
        let (mut out, mut done, mut pool) = (vec![], 0, (k * 8).max(256));
        while out.len() < k && done < d.len() {
            let n = pool.min(d.len());
            if n < d.len() { d[done..].select_nth_unstable_by(n - done - 1, cmp); }
            d[done..n].sort_unstable_by(cmp);
            for &(x, i) in &d[done..n] { let id = self.ids[i as usize]; if keep(id)? { out.push((id, x)); if out.len() == k { break } } }
            (done, pool) = (n, pool * 4);
        }
        Ok(out)
    }
}

fn stamp(c: &Connection) -> Result<(u64, i64)> { Ok((c.total_changes(), c.query_row("pragma data_version", [], |r| r.get(0))?)) }

/// The fresh HNSW sidecar named in `usearch_indices`, if any, and only when it matches `dt` and `m`.
fn sidecar(c: &Connection, store: &str, dt: Dtype, m: Metric) -> Result<Option<Index>> {
    if c.query_row("select 1 from sqlite_master where name='usearch_indices'", [], |_| Ok(())).optional()?.is_none() { return Ok(None) }
    let row: Option<(Option<String>, Option<i64>, Option<String>, Option<String>)> = c.query_row(
        "select path, ndim, metric, dtype from usearch_indices where name=?1", [store], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional()?;
    let Some((Some(p), Some(nd), mt, dts)) = row else { return Ok(None) };
    if Dtype::parse(dts.as_deref().unwrap_or("f16")).ok() != Some(dt) || Metric::parse(mt.as_deref().unwrap_or("cosine")).ok() != Some(m) { return Ok(None) }
    if !std::path::Path::new(&p).exists() || std::path::Path::new(&format!("{p}.stale")).exists() { return Ok(None) }
    let ix = Index::new(&IndexOptions { dimensions: nd as usize, metric: m.kind(), quantization: dt.scalar(), ..Default::default() }).map_err(err)?;
    Ok(ix.load(&p).ok().map(|_| ix))
}

/// Read every embedding of `store` once.
fn load(c: &Connection, store: &str, col: &str, dt: Dtype, m: Metric) -> Result<Vecs> {
    let stamp = stamp(c)?;
    let mut s = c.prepare(&format!("select rowid, {} from {} where {0} is not null order by rowid", q(col), q(store)))?;
    let (mut ids, mut raw, mut w) = (vec![], vec![], 0);
    let mut rows = s.query([])?;
    while let Some(r) = rows.next()? {
        let b = r.get_ref(1)?.as_blob()?;
        if w == 0 { w = b.len() }
        if b.len() != w { return Err(err(format!("{store}: vectors have different lengths ({w} and {} bytes)", b.len()))) }
        ids.push(r.get(0)?);
        raw.extend_from_slice(b);
    }
    let dim = w / dt.size();
    let mut v = Vecs { ids, raw, norms: vec![], dim, dt, ann: None, stamp };
    let mut buf = vec![0f32; dim];
    v.norms = (0..v.ids.len()).map(|i| { v.row(i, &mut buf); dot(&buf, &buf).sqrt() }).collect();
    v.ann = sidecar(c, store, dt, m)?;
    Ok(v)
}

/// Reciprocal Rank Fusion as litesearch's `rrf_all`: 1/(k + rank), ties in first-seen order.
pub fn rrf(legs: [&[i64]; 2], k: f64, lim: usize) -> Vec<(i64, f64, [i64; 2])> {
    let (mut at, mut out): (HashMap<i64, usize>, Vec<(i64, f64, [i64; 2])>) = Default::default();
    for (l, ids) in legs.iter().enumerate() {
        for (r, &id) in ids.iter().enumerate() {
            let i = *at.entry(id).or_insert_with(|| { out.push((id, 0., [-1, -1])); out.len() - 1 });
            out[i].1 += 1. / (k + r as f64);
            out[i].2[l] = r as i64;
        }
    }
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out.truncate(lim);
    out
}

/// One output row: rowid, RRF score, rank in each leg, vector distance.
type Hit = (i64, f64, [i64; 2], Option<f32>);

#[repr(C)]
pub struct Hybrid { base: ffi::sqlite3_vtab, db: Connection, cache: RefCell<HashMap<(String, Dtype, Metric), Rc<Vecs>>> }

impl Hybrid {
    /// This connection's vectors for `store`, read again whenever the database changed.
    fn vecs(&self, store: &str, dt: Dtype, m: Metric) -> Result<Rc<Vecs>> {
        let key = (store.to_string(), dt, m);
        if let Some(v) = self.cache.borrow().get(&key) { if v.stamp == stamp(&self.db)? { return Ok(v.clone()) } }
        let v = Rc::new(load(&self.db, store, "embedding", dt, m)?);
        self.cache.borrow_mut().insert(key, v.clone());
        Ok(v)
    }

    fn run(&self, a: &Args) -> Result<Vec<Hit>> {
        let (c, store, k) = (&self.db, a.store.as_str(), a.k);
        let mut keep = a.filter.as_ref().map(|w| c.prepare_cached(&format!("select 1 from {} where rowid=?1 and ({w})", q(store)))).transpose()?;
        let mut fts = vec![];
        if let Some(fq) = a.q.as_ref().filter(|s| !s.trim().is_empty()) {
            let f = q(&format!("{store}_fts"));
            let mut st = c.prepare_cached(&format!("select rowid from {f} where {f} match ?1 order by rank limit ?2"))?;
            let mut rows = st.query((fq, if keep.is_some() { -1 } else { k as i64 }))?;
            while fts.len() < k {
                let Some(r) = rows.next()? else { break };
                let id: i64 = r.get(0)?;
                if keep.as_mut().map_or(Ok(true), |s| s.exists([id]))? { fts.push(id) }
            }
        }
        let mut vec: Vec<(i64, f32)> = vec![];
        if let Some(b) = &a.qvec {
            let v = self.vecs(store, a.dtype, a.metric)?;
            if !v.ids.is_empty() {
                if b.len() != v.dim * a.dtype.size() { return Err(err(format!("query has {} bytes, {store} rows have {}", b.len(), v.dim * a.dtype.size()))) }
                let mut qv = vec![0f32; v.dim];
                to_f32(b, a.dtype, &mut qv);
                vec = match (&keep, &v.ann) {
                    (None, Some(ix)) => { let m = ix.search(&qv, k).map_err(err)?; m.keys.iter().zip(&m.distances).map(|(&x, &d)| (x as i64, d)).collect() }
                    (None, None) => v.exact(&qv, a.metric, k, |_| Ok(true))?,
                    (Some(_), _) => { let s = keep.as_mut().unwrap(); v.exact(&qv, a.metric, k, |id| s.exists([id]))? }
                };
            }
        }
        let dist: HashMap<i64, f32> = vec.iter().copied().collect();
        let vids: Vec<i64> = vec.iter().map(|x| x.0).collect();
        Ok(rrf([&fts, &vids], 60., k).into_iter().map(|(id, s, r)| (id, s, r, dist.get(&id).copied())).collect())
    }
}

const COLS: usize = 5;
const ARGS: [&str; 7] = ["store", "q", "qvec", "k", "filter", "dtype", "metric"];

struct Args { store: String, q: Option<String>, qvec: Option<Vec<u8>>, k: usize, filter: Option<String>, dtype: Dtype, metric: Metric }

unsafe impl<'vtab> VTab<'vtab> for Hybrid {
    type Aux = ();
    type Cursor = Cur<'vtab>;
    fn connect(db: &mut VTabConnection, _: Option<&()>, _: &[u8], _: &[u8], _: &[u8], _: &[&[u8]]) -> Result<(Cow<'static, CStr>, Self)> {
        let c = unsafe { Connection::from_handle(db.handle())? };
        Ok((Cow::Borrowed(c"create table x(id, score, fts_rank, vec_rank, dist, store hidden, q hidden, qvec hidden, k hidden, filter hidden, dtype hidden, metric hidden)"),
            Hybrid { base: Default::default(), db: c, cache: Default::default() }))
    }
    fn best_index(&self, info: &mut IndexInfo) -> Result<bool> {
        let mut at = [None; ARGS.len()];
        for (i, c) in info.constraints().enumerate() {
            let col = c.column() - COLS as c_int;
            if (0..ARGS.len() as c_int).contains(&col) && c.is_usable() && c.operator() == IndexConstraintOp::SQLITE_INDEX_CONSTRAINT_EQ { at[col as usize] = Some(i) }
        }
        if at[0].is_none() || at[3].is_none() { return Ok(false) }
        let (mut n, mut mask) = (0, 0);
        for (j, i) in at.iter().enumerate() {
            if let Some(i) = *i { n += 1; mask |= 1 << j; let mut u = info.constraint_usage(i); u.set_argv_index(n); u.set_omit(true); }
        }
        info.set_idx_num(mask);
        info.set_estimated_cost(1.);
        Ok(true)
    }
    fn open(&'vtab mut self) -> Result<Cur<'vtab>> { Ok(Cur { base: Default::default(), tab: self, rows: vec![], i: 0 }) }
}

#[repr(C)]
pub struct Cur<'vtab> { base: ffi::sqlite3_vtab_cursor, tab: &'vtab Hybrid, rows: Vec<Hit>, i: usize }

unsafe impl VTabCursor for Cur<'_> {
    fn filter(&mut self, mask: c_int, _: Option<&str>, a: &Filters<'_>) -> Result<()> {
        let mut n = 0;
        let ix: Vec<Option<usize>> = (0..ARGS.len()).map(|j| (mask & (1 << j) != 0).then(|| { n += 1; n - 1 })).collect();
        let s = |j: usize| -> Result<Option<String>> { ix[j].map(|i| a.get::<Option<String>>(i)).transpose().map(Option::flatten) };
        let args = Args {
            store: s(0)?.ok_or_else(|| err("store is required"))?,
            q: s(1)?,
            qvec: ix[2].map(|i| a.get::<Option<Vec<u8>>>(i)).transpose()?.flatten(),
            k: a.get::<i64>(ix[3].unwrap())?.max(0) as usize,
            filter: s(4)?,
            dtype: Dtype::parse(s(5)?.as_deref().unwrap_or("f16"))?,
            metric: Metric::parse(s(6)?.as_deref().unwrap_or("cosine"))?,
        };
        self.rows = self.tab.run(&args)?;
        self.i = 0;
        Ok(())
    }
    fn next(&mut self) -> Result<()> { self.i += 1; Ok(()) }
    fn eof(&self) -> bool { self.i >= self.rows.len() }
    fn column(&self, ctx: &mut Context, col: c_int) -> Result<()> {
        let (id, s, r, d) = self.rows[self.i];
        let rank = |x: i64| (x >= 0).then_some(x);
        match col { 0 => ctx.set_result(id), 1 => ctx.set_result(s), 2 => ctx.set_result(rank(r[0])), 3 => ctx.set_result(rank(r[1])), 4 => ctx.set_result(d.map(f64::from)), _ => ctx.set_result(None::<i64>) }
    }
    fn rowid(&self) -> Result<i64> { Ok(self.i as i64) }
}

/// Register `adhara_hybrid` on `db`.
pub fn register(db: &Connection) -> Result<()> {
    const M: Module<Hybrid> = Module::eponymous_only_module();
    db.create_module(c"adhara_hybrid", &M, None)
}
