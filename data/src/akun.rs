//! Akun warga di server laporan (`LAPORIN_API`): Masuk uji, keluar, profil dan "Laporan saya".
//!
//! Sesi disimpan di `Store` (rahasia aplikasi, grant `secret.keep`) sebagai JSON `{token, nama, email}`, jadi
//! Profil tetap menyebut nama pengguna ketika server tidak terjangkau. Balasan 401 berarti sesi sudah tidak
//! berlaku di server: sesinya dihapus dan aplikasi kembali sebagai tamu.

use exact_runner::{Outcome, Request, Store};
use serde_json::{json, Value as Json};

use crate::kirim::API;
use crate::shapes::{LaporanAkun, Sesi};
use crate::teks;

pub const KUNCI_SESI: &str = "sesi-laporin";

/// Sesi yang tersimpan: token Bearer dan identitas untuk tampilan luring.
pub struct Tersimpan {
    pub token: String,
    pub nama: String,
    pub email: String,
}

pub fn tersimpan(store: &Store) -> Option<Tersimpan> {
    let j: Json = serde_json::from_str(store.get(KUNCI_SESI)?).ok()?;
    let teks_di = |k: &str| j[k].as_str().unwrap_or("").to_string();
    let token = teks_di("token");
    (!token.is_empty()).then(|| Tersimpan { token, nama: teks_di("nama"), email: teks_di("email") })
}

pub fn lupakan(store: &mut Store) {
    let _ = store.forget(KUNCI_SESI);
}

fn dengan_token(minta: Request, token: &str) -> Request {
    minta.header("authorization", &format!("Bearer {token}")).header("accept", "application/json").timeout(15_000)
}

pub fn minta_masuk_uji(nama: &str) -> Request {
    Request::post_json(&format!("{API}/api/mobile/masuk/uji"), &json!({ "nama": nama }).to_string())
        .header("accept", "application/json")
        .timeout(15_000)
}

pub fn minta_keluar(token: &str) -> Request {
    dengan_token(Request::post_json(&format!("{API}/api/mobile/keluar"), "{}"), token)
}

pub fn minta_profil(token: &str) -> Request {
    dengan_token(Request::get(&format!("{API}/api/mobile/saya")), token)
}

pub fn minta_laporan(token: &str) -> Request {
    dengan_token(Request::get(&format!("{API}/api/mobile/saya/laporan")), token)
}

/// Galat dari server (`{ galat }`) atau keterangan jaringan, untuk balasan `{ ok: false, galat }`.
pub fn galat(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Response(r) => serde_json::from_slice::<Json>(&r.body)
            .ok()
            .and_then(|j| j["galat"].as_str().map(str::to_string))
            .unwrap_or_else(|| format!("Server menolak (HTTP {}).", r.status)),
        _ => format!("Server akun tidak terjangkau ({API}). Jalankan `npm run mock` di laporin-apps."),
    }
}

/// Balasan `masuk/uji` → sesi tersimpan, atau pesan galatnya.
pub fn simpan_masuk(store: &mut Store, outcome: &Outcome) -> Result<String, String> {
    let Outcome::Response(r) = outcome else { return Err(galat(outcome)) };
    if !(200..300).contains(&r.status) {
        return Err(galat(outcome));
    }
    let j: Json = serde_json::from_slice(&r.body).map_err(|e| format!("Balasan masuk rusak: {e}"))?;
    let token = j["token"].as_str().filter(|t| !t.is_empty()).ok_or("Server tidak memberi token.")?;
    let p = &j["pengguna"];
    let nama = p["nama"].as_str().unwrap_or("").to_string();
    let sesi = json!({ "token": token, "nama": nama, "email": p["email"].as_str().unwrap_or("") });
    store.set(KUNCI_SESI, &sesi.to_string()).map_err(|e| format!("Sesi tidak bisa disimpan: {e:?}"))?;
    Ok(nama)
}

/// Sesi untuk Profil dari balasan `GET /saya`. Tanpa jaringan, sesi tersimpan tanpa statistik; 401 = keluar.
pub fn sesi(store: &mut Store, outcome: &Outcome) -> Sesi {
    let Some(simpanan) = tersimpan(store) else { return Sesi::default() };
    let luring = Sesi {
        masuk: true,
        inisial: teks::inisial(&simpanan.nama),
        nama: simpanan.nama.clone(),
        email: simpanan.email.clone(),
        ..Sesi::default()
    };
    match outcome {
        Outcome::Response(r) if r.status == 401 => {
            lupakan(store);
            Sesi::default()
        }
        Outcome::Response(r) if (200..300).contains(&r.status) => {
            let Ok(j) = serde_json::from_slice::<Json>(&r.body) else { return luring };
            let (p, s) = (&j["pengguna"], &j["statistik"]);
            let nama = p["nama"].as_str().map_or(simpanan.nama, str::to_string);
            Sesi {
                masuk: true,
                inisial: teks::inisial(&nama),
                nama,
                email: p["email"].as_str().map_or(simpanan.email, str::to_string),
                laporan: s["laporan"].as_f64().unwrap_or(0.0),
                tayang: s["tayang"].as_f64().unwrap_or(0.0),
                disukai: s["disukai"].as_f64().unwrap_or(0.0),
            }
        }
        _ => luring,
    }
}

/// "Laporan saya" dari `GET /saya/laporan` (terbaru dulu). 401 menghapus sesi; galat lain = daftar gagal.
pub fn laporan(store: &mut Store, outcome: &Outcome) -> Result<Vec<LaporanAkun>, String> {
    match outcome {
        Outcome::Response(r) if r.status == 401 => {
            lupakan(store);
            Ok(Vec::new())
        }
        Outcome::Response(r) if (200..300).contains(&r.status) => {
            let j: Json = serde_json::from_slice(&r.body).map_err(|e| format!("JSON laporan saya: {e}"))?;
            let butir = j["laporan"].as_array().ok_or("laporan bukan larik")?;
            Ok(butir
                .iter()
                .map(|l| LaporanAkun {
                    id: l["id"].as_str().unwrap_or("").to_string(),
                    judul: l["judul"].as_str().unwrap_or("").to_string(),
                    status: l["status"].as_str().unwrap_or("menunggu").to_string(),
                    dibuat: teks::waktu_iso_wib(l["dibuat"].as_str().unwrap_or("")),
                    sampul: l["sampul"].as_str().unwrap_or("").to_string(),
                    berkas: l["jumlahBerkas"].as_f64().unwrap_or(0.0),
                })
                .filter(|l| !l.id.is_empty())
                .collect())
        }
        lain => Err(galat(lain)),
    }
}
