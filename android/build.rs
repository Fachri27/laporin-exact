//! Kompilasi dan panggang `app.contract` ke `OUT_DIR/app.plan` saat build, bersama id
//! kompatibilitas, pemasangan hatch dan daftar pemakaian plan, untuk `exact_linux::canvas_jni!` dan
//! build APK.

use contract::DataSource;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // Di luar Android pustakanya kosong (src/lib.rs): tidak ada yang dipanggang.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android") {
        return;
    }
    contract::rerun_if_changed(std::path::Path::new("../app.contract"));
    let plan = contract::compile_path(std::path::Path::new("../app.contract"))
        .unwrap_or_else(|e| panic!("app.contract:{e}"));
    let baked = contract::bake(plan, laporin_data::Laporin).unwrap_or_else(|e| panic!("bake: {e:?}"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out_dir.join("app.plan"), baked.encode()).unwrap();
    println!("cargo:rerun-if-changed=../app.json");
    println!("cargo:rerun-if-changed=../app.tamber");
    println!("cargo:rerun-if-changed=../data");
    println!("cargo:rerun-if-changed=../Cargo.lock");
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let target = std::env::var("TARGET").unwrap_or_default();
    let manifest = contract::Manifest::read(&app_dir).unwrap_or_else(|e| panic!("app.json: {e}"));
    let sumber = laporin_data::Laporin;
    let grants = sumber.grants();
    let compat = exact_bake::compatibility_id(&app_dir, "linux", &target, &manifest, Some(grants))
        .unwrap_or_else(|e| panic!("compatibility id: {e}"));
    std::fs::write(out_dir.join("compat.json"), compat.to_json()).unwrap();
    contract::native::android_entry(&app_dir, &manifest, &baked, &out_dir)
        .unwrap_or_else(|e| panic!("android: {e}"));
}
