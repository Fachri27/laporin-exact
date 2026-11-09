//! Tipe batas data, dibangkitkan saat build dari shape di `../app.contract` (lihat `build.rs`).

#![allow(missing_docs, dead_code, non_camel_case_types, clippy::all)]

include!(concat!(env!("OUT_DIR"), "/shapes.rs"));
