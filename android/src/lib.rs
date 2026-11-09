//! Laporin di host Canvas Android: JNI host (`exact_linux::canvas_jni!`) di atas inti data, dengan
//! penyimpanan aplikasi (`exact_data_host::Storage`) untuk laporan tersimpan, plan yang dipanggang
//! dan hatch-nya. Pembaca (exact/host/android) memuatnya sebagai `libexactcanvas.so`.

#[cfg(target_os = "android")]
include!(concat!(env!("OUT_DIR"), "/hatches.rs"));

#[cfg(target_os = "android")]
exact_linux::canvas_jni!(exact_data_host::Storage<laporin_data::Laporin>, hatches = crate::exact_hatches());
