//! Akun warga di server laporan (`LAPORIN_API`): Masuk uji, masuk/daftar dengan email, keluar, profil,
//! "Laporan saya" dan suka.
//!
//! Sesi disimpan di `Store` (rahasia aplikasi, grant `secret.keep`) sebagai JSON `{token, nama, email}`, jadi
//! Profil tetap menyebut nama pengguna ketika server tidak terjangkau. Balasan 401 berarti sesi sudah tidak
//! berlaku di server: sesinya dihapus dan aplikasi kembali sebagai tamu.
//!
//! Host Android Exact (pre-1.0) tidak bisa menulis rahasia ke disk ("lock acquisition failed due to I/O error"
//! di jurnal): Store hanya memegangnya selama aplikasi hidup. Jadi sesi juga ditulis ke `app:/data/sesi.json`
//! dan dipulihkan dari sana ke Store saat `muatSesi()` pertama setelah aplikasi dibuka.

use exact_runner::{Outcome, Request, Store};
use serde_json::{json, Value as Json};

use crate::kirim::API;
use crate::shapes::{LaporanAkun, Sesi};
use crate::teks;

pub const KUNCI_SESI: &str = "sesi-laporin";
pub const BERKAS_SESI: &str = "app:/data/sesi.json";

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

pub fn minta_baca_sesi() -> Request {
    exact_data::storage::request("fs.readFile", json!({ "path": BERKAS_SESI }))
}

/// Tulis salinan sesi (`"{}"` = keluar).
pub fn minta_tulis_sesi(isi: &str) -> Request {
    exact_data::storage::request("fs.atomicWriteFile", json!({ "path": BERKAS_SESI, "text": isi }))
}

/// Sesi dari isi `app:/data/sesi.json`, dimasukkan lagi ke Store. Berkas yang belum ada, kosong atau rusak = tamu.
pub fn pulihkan(store: &mut Store, outcome: Outcome) -> Option<Tersimpan> {
    let v = exact_data::storage::response(outcome).ok()?;
    let bita = teks::base64(v.get("base64")?.as_str()?)?;
    let isi = String::from_utf8(bita).ok()?;
    let j: Json = serde_json::from_str(&isi).ok()?;
    let token = j["token"].as_str().filter(|t| !t.is_empty())?.to_string();
    let _ = store.set(KUNCI_SESI, &isi);
    Some(Tersimpan { token, nama: j["nama"].as_str().unwrap_or("").into(), email: j["email"].as_str().unwrap_or("").into() })
}

fn dengan_token(minta: Request, token: &str) -> Request {
    minta.header("authorization", &format!("Bearer {token}")).header("accept", "application/json").timeout(15_000)
}

pub fn minta_masuk_uji(nama: &str) -> Request {
    Request::post_json(&format!("{API}/api/mobile/masuk/uji"), &json!({ "nama": nama }).to_string())
        .header("accept", "application/json")
        .timeout(15_000)
}

/// `POST /masuk/email { email, sandi }`: salah sandi dan akun tanpa sandi sama-sama 401 `{ galat }`.
pub fn minta_masuk_email(email: &str, sandi: &str) -> Request {
    Request::post_json(&format!("{API}/api/mobile/masuk/email"), &json!({ "email": email, "sandi": sandi }).to_string())
        .header("accept", "application/json")
        .timeout(15_000)
}

/// `POST /daftar { nama, email, sandi }` → 201 dengan sesi, seperti masuk. Email terpakai = 409 `{ galat }`.
pub fn minta_daftar(nama: &str, email: &str, sandi: &str) -> Request {
    let body = json!({ "nama": nama, "email": email, "sandi": sandi }).to_string();
    Request::post_json(&format!("{API}/api/mobile/daftar"), &body).header("accept", "application/json").timeout(15_000)
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

pub fn minta_komentar(token: &str, id: &str, isi: &str) -> Request {
    let url = format!("{API}/api/mobile/laporan/{}/komentar", teks::enkode_uri(id));
    dengan_token(Request::post_json(&url, &json!({ "isi": isi }).to_string()), token)
}

pub fn minta_disukai(token: &str) -> Request {
    dengan_token(Request::get(&format!("{API}/api/mobile/saya/disukai")), token)
}

/// `POST` (suka) atau `DELETE` (batal) `/laporan/{id}/suka`.
pub fn minta_suka(token: &str, id: &str, suka: bool) -> Request {
    let mut minta = Request::post_json(&format!("{API}/api/mobile/laporan/{}/suka", teks::enkode_uri(id)), "{}");
    if !suka {
        minta.method = "DELETE".into();
        minta.body.clear();
    }
    dengan_token(minta, token)
}

/// Laporkan konten tidak butuh akun; token dikirim bila ada, seperti laporin-apps.
pub fn minta_laporkan(token: Option<String>, jenis: &str, id: &str, alasan: &str, catatan: &str) -> Request {
    let body = json!({ "jenis": jenis, "id": id, "alasan": alasan, "catatan": catatan }).to_string();
    let minta = Request::post_json(&format!("{API}/api/mobile/laporkan"), &body).header("accept", "application/json").timeout(15_000);
    match token {
        Some(t) => minta.header("authorization", &format!("Bearer {t}")),
        None => minta,
    }
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

/// Balasan masuk → sesi tersimpan di Store (JSON-nya dikembalikan untuk `app:/data/sesi.json`), atau pesan galatnya.
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
    let _ = nama;
    store.set(KUNCI_SESI, &sesi.to_string()).map_err(|e| format!("Sesi tidak bisa disimpan: {e:?}"))?;
    Ok(sesi.to_string())
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

/// Id laporan yang disukai akun ini (`GET /saya/disukai` → `{ disukai: [173, …] }`), sebagai teks seperti id umpan.
pub fn disukai(store: &mut Store, outcome: &Outcome) -> Result<Vec<String>, String> {
    match outcome {
        Outcome::Response(r) if r.status == 401 => {
            lupakan(store);
            Ok(Vec::new())
        }
        Outcome::Response(r) if (200..300).contains(&r.status) => {
            let j: Json = serde_json::from_slice(&r.body).map_err(|e| format!("JSON disukai: {e}"))?;
            let butir = j["disukai"].as_array().ok_or("disukai bukan larik")?;
            Ok(butir
                .iter()
                .filter_map(|v| v.as_i64().map(|n| n.to_string()).or_else(|| v.as_str().map(str::to_string)))
                .collect())
        }
        lain => Err(galat(lain)),
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
