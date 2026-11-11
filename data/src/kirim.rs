//! Kirim laporan ke server laporan (`POST {API}/api/mobile/laporan`, multipart), dengan aturan
//! mock-server laporin-apps yang meniru server sungguhan: judul, deskripsi, 1–6 berkas bukti, dan
//! tiket captcha sekali pakai untuk tamu atau anonim.
//!
//! Rantainya, satu permintaan host per langkah (`answer` lalu `parse` berulang):
//! tiket → (perkecil foto → baca foto) × n → unggah multipart → baca & tulis laporan-saya.json.
//! Pengguna yang masuk (tidak anonim) mengirim dengan Bearer token tanpa tiket, seperti server menerima akun
//! sebagai pengganti captcha; kiriman anonim tidak pernah membawa token.
//! Foto diperkecil `fs.compressImage` (JPEG, sisi terpanjang 2048 px, ≤ 1,5 MB), yang juga membuang
//! metadata EXIF dan GPS sebelum foto meninggalkan HP.

use exact_runner::{FailureKind, Outcome, Request};
use serde_json::{json, Value as Json};

use crate::teks::{self, dua};
use crate::simpan;

pub const API: &str = env!("LAPORIN_API");

/// Halaman Turnstile server untuk kiriman tanpa akun, atau "" bila server laporannya mock-server lewat http (WebView
/// Android menolak http polos, dan mock meloloskan `mock-captcha` tanpa halaman).
pub fn alamat_captcha() -> String {
    if API.starts_with("https://") { format!("{API}/api/mobile/captcha") } else { String::new() }
}

const SISI_MAKS: u32 = 2048;
const BITA_MAKS: u32 = 1_500_000;
const BATAS: char = '-';

/// Isian formulir yang dikirim, dari argumen `kirimLaporan(…)`.
#[derive(Debug, Clone)]
pub struct Isian {
    pub judul: String,
    pub deskripsi: String,
    pub nama: String,
    pub anonim: bool,
    pub epoch_ms: f64,
    pub selisih_menit: f64,
    pub lampiran: Vec<String>,
    /// Titik GPS saat formulir dibuka, bila ada.
    pub lokasi: Option<(f64, f64)>,
    /// Token Turnstile dari halaman `/api/mobile/captcha` (lembar Verifikasi singkat) untuk kiriman tanpa akun; kosong
    /// di mock-server, yang meloloskan token `mock-captcha`.
    pub captcha: String,
}

#[derive(Debug)]
enum Tahap {
    Tiket,
    Perkecil(usize),
    Baca(usize),
    Unggah,
    BacaSaya,
    TulisSaya,
}

/// Satu kiriman yang sedang berjalan. Formulir menahan kirim kedua selama `pending(terkirim)`.
#[derive(Debug)]
pub struct Kiriman {
    isian: Isian,
    tahap: Tahap,
    tiket: String,
    token: Option<String>,
    berkas: Vec<Vec<u8>>,
}

/// Langkah berikutnya: permintaan untuk host, atau balasan akhir `(ok, galat)`.
pub enum Langkah {
    Minta(Request),
    Selesai(bool, String),
}

/// Hasil perkecil, langsung di `app:/tmp`: `fs.compressImage` tidak membuat folder tujuan.
fn sementara(i: usize) -> String {
    format!("app:/tmp/laporin-unggah-{i}.jpg")
}

fn tak_terjangkau() -> Langkah {
    Langkah::Selesai(
        false,
        match (API.starts_with("https://"), teks::inggris()) {
            (true, false) => "Server laporan tidak terjangkau. Periksa koneksi lalu coba lagi.".into(),
            (true, true) => "Can't reach the report server. Check your connection and try again.".into(),
            (false, false) => format!(
                "Server laporan tidak terjangkau ({API}). Jalankan `npm run mock` di laporin-apps, lalu `adb reverse tcp:4000 tcp:4000`."
            ),
            (false, true) => format!(
                "Can't reach the report server ({API}). Run `npm run mock` in laporin-apps, then `adb reverse tcp:4000 tcp:4000`."
            ),
        },
    )
}

/// Pesan `galat` dari balasan JSON server, atau keterangan status HTTP-nya.
fn galat_server(status: u16, body: &[u8]) -> String {
    serde_json::from_slice::<Json>(body)
        .ok()
        .and_then(|j| j.get("galat").and_then(Json::as_str).map(str::to_string))
        .unwrap_or_else(|| dua!(format!("Server menolak laporan (HTTP {status})."), format!("The server rejected the report (HTTP {status}).")))
}

/// Body multipart/form-data (RFC 7578) dengan batas yang tidak muncul di isi mana pun; berkas di bagian `berkas`.
pub fn multipart(kolom: &[(&str, &str)], berkas: &[Vec<u8>]) -> (String, Vec<u8>) {
    multipart_bernama(kolom, "berkas", berkas)
}

/// Seperti [`multipart`], dengan nama bagian berkas sendiri (verifikasi KTP: `ktp`, berkas `ktp.jpg`).
pub fn multipart_bernama(kolom: &[(&str, &str)], nama_berkas: &str, berkas: &[Vec<u8>]) -> (String, Vec<u8>) {
    let mut batas = String::from("laporin-exact-batas-7f3a");
    while kolom.iter().any(|(_, v)| v.contains(&batas)) || berkas.iter().any(|b| b.windows(batas.len()).any(|w| w == batas.as_bytes())) {
        batas.push(BATAS);
        batas.push('x');
    }
    let mut body = Vec::new();
    for (nama, nilai) in kolom {
        body.extend_from_slice(format!("--{batas}\r\nContent-Disposition: form-data; name=\"{nama}\"\r\n\r\n{nilai}\r\n").as_bytes());
    }
    for (i, isi) in berkas.iter().enumerate() {
        body.extend_from_slice(
            format!(
                "--{batas}\r\nContent-Disposition: form-data; name=\"{nama_berkas}\"; filename=\"{}\"\r\nContent-Type: image/jpeg\r\n\r\n",
                if nama_berkas == "berkas" { format!("foto-{}.jpg", i + 1) } else { format!("{nama_berkas}.jpg") }
            )
            .as_bytes(),
        );
        body.extend_from_slice(isi);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{batas}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={batas}"), body)
}

impl Kiriman {
    /// Kiriman baru dan permintaan pertamanya (tiket captcha), atau penolakan isian langsung.
    pub fn mulai(isian: Isian, token: Option<String>) -> Result<(Kiriman, Request), String> {
        if isian.judul.trim().is_empty() {
            return Err(dua!("Isi judul singkat dulu.", "Add a short title first.").into());
        }
        if isian.deskripsi.trim().is_empty() {
            return Err(dua!("Ceritakan kondisinya sedikit.", "Briefly describe the situation.").into());
        }
        if isian.lampiran.is_empty() {
            return Err(dua!("Sertakan minimal satu foto sebagai bukti.", "Attach at least one photo as evidence.").into());
        }
        if isian.lampiran.len() > 6 {
            return Err(dua!("Maksimal 6 foto per laporan.", "Up to 6 photos per report.").into());
        }
        let token = token.filter(|_| !isian.anonim);
        let mut kiriman = Kiriman { isian, tahap: Tahap::Tiket, tiket: String::new(), token, berkas: Vec::new() };
        let minta = if kiriman.token.is_some() {
            kiriman.minta_perkecil(0)
        } else {
            // Token Turnstile ditukar dengan tiket sekali pakai. Mock-server meloloskan `mock-captcha` (laporin-apps
            // mock-server/server.mjs), jadi tanpa token dari lembar verifikasi itu yang dikirim.
            let captcha = if kiriman.isian.captcha.is_empty() { "mock-captcha" } else { kiriman.isian.captcha.as_str() };
            Request::post_json(&format!("{API}/api/mobile/tiket"), &json!({ "token": captcha }).to_string()).timeout(20_000)
        };
        Ok((kiriman, minta))
    }

    fn minta_perkecil(&mut self, i: usize) -> Request {
        self.tahap = Tahap::Perkecil(i);
        exact_data::storage::request(
            "fs.compressImage",
            json!({ "path": self.isian.lampiran[i], "destination": sementara(i), "maxDimension": SISI_MAKS, "maxBytes": BITA_MAKS }),
        )
    }

    fn perkecil(&mut self, i: usize) -> Langkah {
        Langkah::Minta(self.minta_perkecil(i))
    }

    fn unggah(&mut self) -> Langkah {
        self.tahap = Tahap::Unggah;
        let i = &self.isian;
        let mut kolom = vec![("judul", i.judul.as_str()), ("deskripsi", i.deskripsi.as_str())];
        if self.token.is_none() {
            kolom.push(("tiket", self.tiket.as_str()));
        }
        if i.anonim {
            kolom.push(("anonim", "1"));
        } else if !i.nama.is_empty() {
            kolom.push(("nama", i.nama.as_str()));
        }
        // Server meminta lat & lng berpasangan, tujuh desimal (laporin-apps src/lib/lapor.ts).
        let titik = i.lokasi.map(|(lat, lng)| (format!("{lat:.7}"), format!("{lng:.7}")));
        if let Some((lat, lng)) = &titik {
            kolom.push(("lat", lat.as_str()));
            kolom.push(("lng", lng.as_str()));
        }
        let (jenis, body) = multipart(&kolom, &self.berkas);
        let minta = Request { method: "POST".into(), body, ..Request::get(&format!("{API}/api/mobile/laporan")) }
            .header("content-type", &jenis)
            .header("accept", "application/json")
            .timeout(60_000);
        Langkah::Minta(match &self.token {
            Some(token) => minta.header("authorization", &format!("Bearer {token}")),
            None => minta.header("x-tiket-lapor", &self.tiket),
        })
    }

    /// Olah balasan langkah yang sedang berjalan dan tentukan langkah berikutnya.
    pub fn lanjut(&mut self, outcome: Outcome) -> Langkah {
        match self.tahap {
            Tahap::Tiket => match outcome {
                Outcome::Response(r) if (200..300).contains(&r.status) => {
                    let tiket = serde_json::from_slice::<Json>(&r.body).ok().and_then(|j| j["tiket"].as_str().map(str::to_string));
                    match tiket {
                        Some(t) => {
                            self.tiket = t;
                            self.perkecil(0)
                        }
                        None => Langkah::Selesai(
                            false,
                            dua!("Server tidak memberi tiket captcha.", "The server didn't issue a captcha ticket.").into(),
                        ),
                    }
                }
                Outcome::Response(r) => Langkah::Selesai(false, galat_server(r.status, &r.body)),
                _ => tak_terjangkau(),
            },
            Tahap::Perkecil(i) => match exact_data::storage::response(outcome) {
                Ok(hasil) => {
                    let jalur = hasil["path"].as_str().map(str::to_string).unwrap_or_else(|| sementara(i));
                    self.tahap = Tahap::Baca(i);
                    Langkah::Minta(exact_data::storage::request("fs.readFile", json!({ "path": jalur })))
                }
                Err(e) => Langkah::Selesai(
                    false,
                    dua!(
                        format!("Foto {} tidak bisa diproses ({} → {}): {e}", i + 1, self.isian.lampiran[i], sementara(i)),
                        format!("Photo {} couldn't be processed ({} → {}): {e}", i + 1, self.isian.lampiran[i], sementara(i))
                    ),
                ),
            },
            Tahap::Baca(i) => {
                let isi = exact_data::storage::response(outcome)
                    .ok()
                    .and_then(|v| v["base64"].as_str().and_then(teks::base64));
                match isi {
                    Some(bita) => {
                        self.berkas.push(bita);
                        if i + 1 < self.isian.lampiran.len() {
                            self.perkecil(i + 1)
                        } else {
                            self.unggah()
                        }
                    }
                    None => Langkah::Selesai(
                        false,
                        dua!(format!("Foto {} tidak bisa dibaca.", i + 1), format!("Photo {} couldn't be read.", i + 1)),
                    ),
                }
            }
            Tahap::Unggah => match outcome {
                Outcome::Response(r) if (200..300).contains(&r.status) => {
                    self.tahap = Tahap::BacaSaya;
                    Langkah::Minta(simpan::minta_baca())
                }
                Outcome::Response(r) => Langkah::Selesai(false, galat_server(r.status, &r.body)),
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
                _ => tak_terjangkau(),
            },
            // Terkirim; salinannya untuk Profil. Gagal menyimpan salinan tidak membatalkan kiriman.
            Tahap::BacaSaya => match simpan::baca_balasan(outcome) {
                Ok(simpan::Balasan::Isi(mut semua)) => {
                    let i = &self.isian;
                    let mut catatan = simpan::catatan(&i.judul, &i.deskripsi, &i.nama, i.anonim, i.epoch_ms, i.selisih_menit);
                    catatan["lampiran"] = json!(self.berkas.len());
                    semua.insert(0, catatan);
                    self.tahap = Tahap::TulisSaya;
                    Langkah::Minta(simpan::minta_tulis(&semua))
                }
                _ => Langkah::Selesai(true, String::new()),
            },
            Tahap::TulisSaya => Langkah::Selesai(true, String::new()),
        }
    }
}

#[cfg(test)]
mod uji {
    use super::*;

    #[test]
    fn multipart_berisi_kolom_dan_berkas() {
        let (jenis, body) = multipart(&[("judul", "Asap"), ("anonim", "1")], &[vec![0xFF, 0xD8, 0xFF, 0x00]]);
        assert_eq!(jenis, "multipart/form-data; boundary=laporin-exact-batas-7f3a");
        let teks = String::from_utf8_lossy(&body);
        assert!(teks.starts_with("--laporin-exact-batas-7f3a\r\nContent-Disposition: form-data; name=\"judul\"\r\n\r\nAsap\r\n"));
        assert!(teks.contains("name=\"berkas\"; filename=\"foto-1.jpg\"\r\nContent-Type: image/jpeg\r\n\r\n"));
        assert!(body.windows(4).any(|w| w == [0xFF, 0xD8, 0xFF, 0x00]));
        assert!(teks.ends_with("--laporin-exact-batas-7f3a--\r\n"));
    }

    #[test]
    fn batas_menghindari_isi() {
        let (jenis, _) = multipart(&[("deskripsi", "ada laporin-exact-batas-7f3a di sini")], &[]);
        assert_eq!(jenis, "multipart/form-data; boundary=laporin-exact-batas-7f3a-x");
    }

    fn isian(lampiran: usize) -> Isian {
        Isian {
            judul: "Asap".into(),
            deskripsi: "Tebal".into(),
            nama: String::new(),
            anonim: false,
            epoch_ms: 0.0,
            selisih_menit: 420.0,
            lampiran: (0..lampiran).map(|i| format!("app:/tmp/pilih/{i}")).collect(),
            lokasi: None,
            captcha: String::new(),
        }
    }

    #[test]
    fn isian_tanpa_foto_ditolak_sebelum_jaringan() {
        assert_eq!(Kiriman::mulai(isian(0), None).err().unwrap(), "Sertakan minimal satu foto sebagai bukti.");
        assert_eq!(Kiriman::mulai(isian(7), None).err().unwrap(), "Maksimal 6 foto per laporan.");
        let (_, minta) = Kiriman::mulai(isian(1), None).unwrap();
        assert_eq!(minta.url, format!("{API}/api/mobile/tiket"));
    }

    #[test]
    fn token_captcha_ditukar_jadi_tiket() {
        let mut i = isian(1);
        i.captcha = "0.turnstile-abc".into();
        let (_, minta) = Kiriman::mulai(i, None).unwrap();
        assert_eq!(String::from_utf8_lossy(&minta.body), r#"{"token":"0.turnstile-abc"}"#);
        let (_, minta) = Kiriman::mulai(isian(1), None).unwrap();
        assert_eq!(String::from_utf8_lossy(&minta.body), r#"{"token":"mock-captcha"}"#);
    }

    #[test]
    fn akun_melewati_tiket_kecuali_anonim() {
        let (_, minta) = Kiriman::mulai(isian(1), Some("abc".into())).unwrap();
        assert!(minta.storage.is_some(), "pengguna yang masuk langsung memperkecil foto");
        let mut anonim = isian(1);
        anonim.anonim = true;
        let (_, minta) = Kiriman::mulai(anonim, Some("abc".into())).unwrap();
        assert_eq!(minta.url, format!("{API}/api/mobile/tiket"));
    }
}
