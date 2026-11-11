//! Siaran langsung (LiveKit) lewat server laporan (`LAPORIN_API`), seperti laporin-apps (`docs/LIVE.md`):
//! token untuk menyiarkan (`POST /api/mobile/live/token`), token untuk menonton siaran publik
//! (`POST /api/mobile/live/tonton`) dan daftar siaran publik (`GET /api/mobile/live`).
//!
//! Secret LiveKit tinggal di server: aplikasi hanya menerima alamat `wss://` dan token sekali pakai, yang Contract
//! serahkan ke halaman siaran (`assets/live/`) di WebView. Halaman itu yang tersambung ke LiveKit; WebView menolak
//! `http`/`ws` polos, jadi token tidak diminta dari halaman.

use exact_runner::{Outcome, Request};
use serde_json::{json, Value as Json};

use crate::akun;
use crate::kirim::API;
use crate::shapes::{SiaranPublik, TokenLive};
use crate::teks::dua;

fn dengan_sesi(minta: Request, sesi: Option<&str>) -> Request {
    let minta = minta.header("accept", "application/json").timeout(15_000);
    match sesi {
        Some(t) => minta.header("authorization", &format!("Bearer {t}")),
        None => minta,
    }
}

/// Isian "Mulai live". Siaran publik (tampil di tab Live warga lain) butuh akun dengan KTP sah dan tidak anonim;
/// tanpa akun siarannya hanya untuk petugas, atas nama "Warga".
pub struct Isian<'a> {
    pub judul: &'a str,
    pub publik: bool,
    pub lokasi: Option<(f64, f64)>,
}

pub fn minta_token(sesi: Option<&str>, isian: &Isian) -> Request {
    let (lat, lng) = isian.lokasi.map_or((Json::Null, Json::Null), |(a, b)| (json!(a), json!(b)));
    let body = json!({
        "judul": isian.judul,
        "anonim": false,
        "publik": isian.publik,
        "lat": lat,
        "lng": lng,
    });
    dengan_sesi(Request::post_json(&format!("{API}/api/mobile/live/token"), &body.to_string()), sesi)
}

pub fn minta_tonton(sesi: Option<&str>, ruang: &str) -> Request {
    let body = json!({ "ruang": ruang }).to_string();
    dengan_sesi(Request::post_json(&format!("{API}/api/mobile/live/tonton"), &body), sesi)
}

/// Token pratinjau: server membuat peserta tersembunyi (tidak terhitung penonton, tidak bisa berkomentar) untuk video
/// bisu di kartu tab Live dan sorotan Beranda, seperti `pratinjau` di lib/live.ts laporin-apps.
pub fn minta_pratinjau(ruang: &str) -> Request {
    let body = json!({ "ruang": ruang, "pratinjau": true }).to_string();
    dengan_sesi(Request::post_json(&format!("{API}/api/mobile/live/tonton"), &body), None)
}

pub fn minta_daftar() -> Request {
    dengan_sesi(Request::get(&format!("{API}/api/mobile/live")), None)
}

/// Token siaran atau tonton; galat server (`{ galat }`) atau jaringan masuk ke `galat`, token kosong.
pub fn token(outcome: &Outcome, ruang: &str) -> TokenLive {
    match outcome {
        Outcome::Response(r) if (200..300).contains(&r.status) => match serde_json::from_slice::<Json>(&r.body) {
            Ok(j) if j["token"].as_str().is_some_and(|t| !t.is_empty()) => TokenLive {
                url: j["url"].as_str().unwrap_or("").to_string(),
                token: j["token"].as_str().unwrap_or("").to_string(),
                ruang: j["ruang"].as_str().unwrap_or(ruang).to_string(),
                galat: String::new(),
            },
            _ => TokenLive {
                galat: dua!("Server siaran tidak memberi token.", "The live server didn't return a token.").into(),
                ..TokenLive::default()
            },
        },
        lain => TokenLive { galat: akun::galat(lain), ..TokenLive::default() },
    }
}

/// Siaran publik yang sedang berjalan, terbaru dulu (`[{ ruang, judul, nama, mulai, penonton }]`).
pub fn daftar(outcome: &Outcome) -> Result<Vec<SiaranPublik>, String> {
    let Outcome::Response(r) = outcome else { return Err(akun::galat(outcome)) };
    if !(200..300).contains(&r.status) {
        return Err(akun::galat(outcome));
    }
    let j: Json = serde_json::from_slice(&r.body).map_err(|e| dua!(format!("JSON siaran: {e}"), format!("Live list JSON: {e}")))?;
    let butir = j.as_array().ok_or(dua!("daftar siaran bukan larik", "live list is not an array"))?;
    let bawaan = dua!("Warga", "Citizen");
    Ok(butir
        .iter()
        .map(|s| SiaranPublik {
            ruang: s["ruang"].as_str().unwrap_or("").to_string(),
            judul: s["judul"].as_str().unwrap_or("").to_string(),
            nama: s["nama"].as_str().unwrap_or(bawaan).to_string(),
            mulai: s["mulai"].as_f64().unwrap_or(0.0),
            penonton: s["penonton"].as_f64().unwrap_or(0.0),
            pratinjau_url: String::new(),
            pratinjau_token: String::new(),
        })
        .filter(|s| !s.ruang.is_empty())
        .collect())
}

#[cfg(test)]
mod uji {
    use super::*;
    use exact_runner::Response;

    fn balasan(status: u16, body: &str) -> Outcome {
        Outcome::Response(Response { status, headers: Vec::new(), body: body.as_bytes().to_vec() })
    }

    #[test]
    fn token_dari_server_dan_galatnya() {
        let t = token(&balasan(200, r#"{"url":"wss://x.livekit.cloud","token":"abc","ruang":"siaran-1"}"#), "");
        assert_eq!((t.url.as_str(), t.token.as_str(), t.ruang.as_str(), t.galat.as_str()), ("wss://x.livekit.cloud", "abc", "siaran-1", ""));
        let t = token(&balasan(200, r#"{"url":"wss://x","token":"abc"}"#), "siaran-2");
        assert_eq!(t.ruang, "siaran-2");
        let t = token(&balasan(403, r#"{"galat":"Verifikasi KTP dulu untuk siaran langsung."}"#), "");
        assert_eq!((t.token.as_str(), t.galat.as_str()), ("", "Verifikasi KTP dulu untuk siaran langsung."));
    }

    #[test]
    fn daftar_siaran_publik() {
        let d = daftar(&balasan(200, r#"[{"ruang":"siaran-a","judul":"Asap","nama":"Budi","mulai":5,"penonton":2},{"judul":"tanpa ruang"}]"#)).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!((d[0].nama.as_str(), d[0].penonton), ("Budi", 2.0));
        assert!(daftar(&balasan(502, r#"{"galat":"Server siaran tidak bisa dihubungi."}"#)).is_err());
    }
}
