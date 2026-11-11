//! Verifikasi KTP, syarat siaran langsung (laporin-apps `src/lib/verifikasi.ts`, `docs/VERIFIKASI.md`): akun
//! mengirim NIK dan foto KTP sekali saja, petugas meninjaunya di CMS situs, lalu fotonya dihapus server.
//!
//! - `muatVerifikasi(email)`: GET /api/mobile/saya/verifikasi dengan sesi → `StatusKtp` (`status` kosong = belum masuk
//!   atau tidak termuat).
//! - `ajukanVerifikasi(nik, foto)`: foto diperkecil `fs.compressImage` (JPEG ≤ 2048 px, ≤ 1,5 MB; EXIF dan lokasi
//!   ikut terbuang), dibaca, lalu POST multipart `{ nik, ktp }` dengan sesi → `{ ok, galat }`.

use exact_runner::{FailureKind, Outcome, Request};
use serde_json::{json, Value as Json};

use crate::kirim::{multipart_bernama, API};
use crate::shapes::StatusKtp;
use crate::teks::{self, dua};

pub const PANJANG_NIK: usize = 16;
const SISI_MAKS: u32 = 2048;
const BITA_MAKS: u32 = 1_500_000;
const SEMENTARA: &str = "app:/tmp/laporin-ktp.jpg";

fn hari_dalam_bulan(bulan: u32, tahun2: u32) -> u32 {
    // Abad tidak tertulis di NIK: 29 Februari sah bila tahunnya kabisat di salah satu abad.
    match bulan {
        2 if tahun2 % 4 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Bentuk NIK, sama dengan `periksaNik` situs (lib/nik.ts): PP KK CC DDMMYY NNNN — provinsi 11–96, kabupaten,
/// kecamatan dan nomor urut bukan nol, tanggal lahir yang ada (perempuan: tanggal + 40). `None` = masuk akal.
pub fn periksa_nik(nik: &str) -> Option<&'static str> {
    if nik.len() != PANJANG_NIK || !nik.bytes().all(|b| b.is_ascii_digit()) {
        return Some(dua!("NIK harus 16 angka.", "The NIK must be 16 digits."));
    }
    let dua = |i: usize| nik[i..i + 2].parse::<u32>().unwrap_or(0);
    let tanggal = dua(6);
    let hari = if tanggal > 40 { tanggal - 40 } else { tanggal };
    let bulan = dua(8);
    let sah = (11..=96).contains(&dua(0))
        && dua(2) > 0
        && dua(4) > 0
        && nik[12..].parse::<u32>().unwrap_or(0) > 0
        && (1..=12).contains(&bulan)
        && hari >= 1
        && hari <= hari_dalam_bulan(bulan, dua(10));
    if sah { None } else { Some(dua!("NIK tidak valid. Periksa lagi angkanya.", "Invalid NIK. Check the digits again.")) }
}

pub fn minta_status(token: &str) -> Request {
    Request::get(&format!("{API}/api/mobile/saya/verifikasi"))
        .header("authorization", &format!("Bearer {token}"))
        .header("accept", "application/json")
        .timeout(15_000)
}

/// Status dari balasan GET. Galat jaringan/server = `Err` (resource gagal, Contract menawarkan coba lagi).
pub fn status(outcome: &Outcome) -> Result<StatusKtp, String> {
    match outcome {
        Outcome::Response(r) if (200..300).contains(&r.status) => {
            let j: Json = serde_json::from_slice(&r.body).map_err(|_| {
                dua!("Balasan verifikasi tidak terbaca.", "Couldn't read the verification response.").to_string()
            })?;
            let status = j["status"].as_str().unwrap_or("");
            if !matches!(status, "belum" | "menunggu" | "disetujui" | "ditolak") {
                return Err(dua!("Status verifikasi tidak dikenal.", "Unknown verification status.").into());
            }
            Ok(StatusKtp {
                status: status.into(),
                alasan: j["alasan"].as_str().unwrap_or("").into(),
                diajukan: j["diajukan"].as_str().unwrap_or("").into(),
            })
        }
        Outcome::Response(r) => Err(dua!(
            format!("Status verifikasi tidak bisa dimuat (HTTP {}).", r.status),
            format!("Couldn't load the verification status (HTTP {}).", r.status)
        )),
        _ => Err(dua!("Server tidak terjangkau.", "Can't reach the server.").into()),
    }
}

#[derive(Debug)]
enum Tahap {
    Perkecil,
    Baca,
    Unggah,
}

/// Satu pengajuan yang sedang berjalan di antara `answer` dan `parse`.
#[derive(Debug)]
pub struct Ajuan {
    nik: String,
    token: String,
    tahap: Tahap,
}

/// Langkah berikutnya: permintaan untuk host, atau balasan akhir `(ok, galat)`.
pub enum Langkah {
    Minta(Request),
    Selesai(bool, String),
}

impl Ajuan {
    /// Pengajuan baru dan permintaan pertamanya (perkecil foto), atau penolakan isian sebelum jaringan.
    pub fn mulai(nik: &str, foto: &str, token: Option<String>) -> Result<(Ajuan, Request), String> {
        let token = token.ok_or(dua!("Masuk dulu untuk verifikasi.", "Sign in to verify your ID."))?;
        if let Some(salah) = periksa_nik(nik) {
            return Err(salah.into());
        }
        if foto.is_empty() {
            return Err(dua!("Potret KTP dulu.", "Take a photo of your ID card first.").into());
        }
        let minta = exact_data::storage::request(
            "fs.compressImage",
            json!({ "path": foto, "destination": SEMENTARA, "maxDimension": SISI_MAKS, "maxBytes": BITA_MAKS }),
        );
        Ok((Ajuan { nik: nik.into(), token, tahap: Tahap::Perkecil }, minta))
    }

    pub fn lanjut(&mut self, outcome: Outcome) -> Langkah {
        match self.tahap {
            Tahap::Perkecil => match exact_data::storage::response(outcome) {
                Ok(hasil) => {
                    let jalur = hasil["path"].as_str().unwrap_or(SEMENTARA).to_string();
                    self.tahap = Tahap::Baca;
                    Langkah::Minta(exact_data::storage::request("fs.readFile", json!({ "path": jalur })))
                }
                Err(e) => Langkah::Selesai(
                    false,
                    dua!(format!("Foto KTP tidak bisa diproses: {e}"), format!("The ID card photo couldn't be processed: {e}")),
                ),
            },
            Tahap::Baca => {
                let isi = exact_data::storage::response(outcome).ok().and_then(|v| v["base64"].as_str().and_then(teks::base64));
                let Some(bita) = isi else {
                    return Langkah::Selesai(false, dua!("Foto KTP tidak bisa dibaca.", "The ID card photo couldn't be read.").into());
                };
                self.tahap = Tahap::Unggah;
                let (jenis, body) = multipart_bernama(&[("nik", self.nik.as_str())], "ktp", &[bita]);
                let minta = Request { method: "POST".into(), body, ..Request::get(&format!("{API}/api/mobile/saya/verifikasi")) }
                    .header("content-type", &jenis)
                    .header("accept", "application/json")
                    .header("authorization", &format!("Bearer {}", self.token))
                    .timeout(90_000);
                Langkah::Minta(minta)
            }
            Tahap::Unggah => match outcome {
                Outcome::Response(r) if (200..300).contains(&r.status) => Langkah::Selesai(true, String::new()),
                Outcome::Response(r) => Langkah::Selesai(
                    false,
                    serde_json::from_slice::<Json>(&r.body)
                        .ok()
                        .and_then(|j| j["galat"].as_str().map(str::to_string))
                        .unwrap_or_else(|| dua!(format!("Gagal mengirim (HTTP {}).", r.status), format!("Sending failed (HTTP {}).", r.status))),
                ),
                Outcome::Failed { kind: FailureKind::Timeout, .. } => {
                    Langkah::Selesai(
                        false,
                        dua!(
                            "Unggahan terlalu lama. Periksa koneksi lalu coba lagi.",
                            "The upload took too long. Check your connection and try again."
                        )
                        .into(),
                    )
                }
                _ => Langkah::Selesai(
                    false,
                    dua!(
                        "Tidak bisa menghubungi server. Periksa koneksi lalu coba lagi.",
                        "Can't reach the server. Check your connection and try again."
                    )
                    .into(),
                ),
            },
        }
    }
}

#[cfg(test)]
mod uji {
    use super::*;

    #[test]
    fn nik_diperiksa_seperti_situs() {
        assert_eq!(periksa_nik("123"), Some("NIK harus 16 angka."));
        assert_eq!(periksa_nik("62710a0101900001"), Some("NIK harus 16 angka."));
        // Kalteng 62, Palangka Raya 71, kecamatan 01, lahir 1 Jan 1990, urut 0001.
        assert_eq!(periksa_nik("6271010101900001"), None);
        // Perempuan: tanggal + 40.
        assert_eq!(periksa_nik("6271014101900001"), None);
        // Provinsi 10 tidak ada; 31 Februari; nomor urut nol.
        assert!(periksa_nik("1071010101900001").is_some());
        assert!(periksa_nik("6271013102900001").is_some());
        assert!(periksa_nik("6271010101900000").is_some());
        // 29 Februari hanya di tahun (dua angka) kabisat.
        assert_eq!(periksa_nik("6271012902000001"), None);
        assert!(periksa_nik("6271012902010001").is_some());
    }

    #[test]
    fn ajuan_ditolak_sebelum_jaringan() {
        assert_eq!(Ajuan::mulai("6271010101900001", "app:/tmp/camera/1.jpg", None).err().unwrap(), "Masuk dulu untuk verifikasi.");
        assert_eq!(Ajuan::mulai("6271010101900001", "", Some("t".into())).err().unwrap(), "Potret KTP dulu.");
        assert_eq!(Ajuan::mulai("12", "app:/tmp/camera/1.jpg", Some("t".into())).err().unwrap(), "NIK harus 16 angka.");
        assert!(Ajuan::mulai("6271010101900001", "app:/tmp/camera/1.jpg", Some("t".into())).is_ok());
    }

    #[test]
    fn status_dari_server() {
        let balas = |body: &[u8]| Outcome::Response(exact_runner::Response { status: 200, headers: Vec::new(), body: body.to_vec() });
        let ok = balas(br#"{"status":"ditolak","alasan":"Foto buram.","diajukan":"2026-11-11T10:00:00Z","diputuskan":null}"#);
        let s = status(&ok).unwrap();
        assert_eq!((s.status.as_str(), s.alasan.as_str()), ("ditolak", "Foto buram."));
        assert!(status(&balas(br#"{"status":"x"}"#)).is_err());
    }
}
