//! Tipe di batas data dibangkitkan dari shape di `../app.contract` (`contract rust`), jadi field
//! yang ditambah di sana menjadi galat kompilasi di sini, bukan record yang bergeser satu posisi.

fn main() {
    let path = std::path::Path::new("../app.contract");
    println!("cargo:rerun-if-changed={}", path.display());
    let plan = contract::compile_path(path).unwrap_or_else(|e| panic!("../app.contract: {e}"));
    let rust = contract::rust(&plan).expect("contract rust");
    let out = std::path::Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("shapes.rs");
    std::fs::write(out, rust).expect("tulis shapes.rs");
}
