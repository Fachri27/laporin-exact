//! Tipe batas data, dibangkitkan saat build dari shape di `../app.contract` (lihat `build.rs`).

#![allow(missing_docs, dead_code, clippy::all)]

include!(concat!(env!("OUT_DIR"), "/shapes.rs"));
