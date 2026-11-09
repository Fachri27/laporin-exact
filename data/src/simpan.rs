//! Salinan laporan yang terkirim dari HP ini, di `app:/data/laporan-saya.json`, untuk Profil (tamu tidak
//! punya akun untuk membaca `/api/mobile/saya/laporan`). Host Exact menjalankan baca/tulis berkasnya.

use exact_runner::{Outcome, Request};
use serde_json::{json, Value as Json};

use crate::shapes::LaporanSaya;
use crate::teks;

pub const BERKAS: &str = "app:/data/laporan-saya.json";

pub fn minta_baca() -> Request {
    exact_data::storage::request("fs.readFile", json!({ "path": BERKAS }))
}

pub fn minta_tulis(semua: &[Json]) -> Request {
    exact_data::storage::request("fs.atomicWriteFile", json!({ "path": BERKAS, "text": Json::Array(semua.to_vec()).to_string() }))
}

/// Galat baca karena berkas belum ada: belum pernah mengirim, jadi daftarnya kosong.
fn belum_ada(pesan: &str) -> bool {
    let p = pesan.to_ascii_lowercase();
    ["enoent", "no such file", "not found", "notfound", "does not exist"].iter().any(|k| p.contains(k))
}

/// Balasan penyimpanan dari sebuah `fs.readFile` atau `fs.atomicWriteFile`.
pub enum Balasan {
    /// Isi berkas (larik JSON), kosong bila berkas belum ada.
    Isi(Vec<Json>),
    /// Tulis selesai.
    Tertulis,
}

pub fn baca_balasan(outcome: Outcome) -> Result<Balasan, String> {
    match exact_data::storage::response(outcome) {
        Ok(Json::Null) => Ok(Balasan::Tertulis),
        Ok(v) => {
            let b64 = v.get("base64").and_then(Json::as_str).ok_or("balasan baca tanpa base64")?;
            let bita = teks::base64(b64).ok_or("base64 rusak")?;
            if bita.iter().all(u8::is_ascii_whitespace) {
                return Ok(Balasan::Isi(Vec::new()));
            }
            match serde_json::from_slice::<Json>(&bita).map_err(|e| format!("isi berkas: {e}"))? {
                Json::Array(a) => Ok(Balasan::Isi(a)),
                _ => Err("isi berkas bukan larik".into()),
            }
        }
        Err(e) if belum_ada(&e) => Ok(Balasan::Isi(Vec::new())),
        Err(e) => Err(e),
    }
}

/// Catatan baru di depan daftar (terbaru dulu). `id` dari waktu kirim, unik per milidetik.
/// `anonim` dibedakan dari nama yang dikosongkan: keduanya tanpa `pengirim`, tetapi hanya anonim yang
/// meminta server tidak mengaitkan laporan ke siapa pun.
pub fn catatan(judul: &str, deskripsi: &str, nama: &str, anonim: bool, epoch_ms: f64, selisih_menit: f64) -> Json {
    json!({
        "id": format!("{}", epoch_ms.round() as i64),
        "judul": judul,
        "deskripsi": deskripsi,
        "pengirim": nama,
        "anonim": anonim,
        "status": "menunggu",
        "dibuat": teks::waktu_singkat(epoch_ms, selisih_menit),
    })
}

pub fn ke_laporan(semua: &[Json]) -> Vec<LaporanSaya> {
    let teks_di = |j: &Json, k: &str| j.get(k).and_then(Json::as_str).unwrap_or("").to_string();
    semua
        .iter()
        .map(|j| LaporanSaya {
            id: teks_di(j, "id"),
            judul: teks_di(j, "judul"),
            deskripsi: teks_di(j, "deskripsi"),
            pengirim: teks_di(j, "pengirim"),
            status: teks_di(j, "status"),
            dibuat: teks_di(j, "dibuat"),
            lampiran: j.get("lampiran").and_then(Json::as_f64).unwrap_or(0.0),
            anonim: j.get("anonim").and_then(Json::as_bool).unwrap_or(false),
        })
        .filter(|l| !l.id.is_empty())
        .collect()
}
