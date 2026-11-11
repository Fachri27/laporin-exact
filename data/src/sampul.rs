//! Sampul Beranda di HP (`siapkanSampul(urls)`): setiap gambar sampul disimpan di `app:/data/sampul/`, supaya
//! Beranda yang dibuka dingin dari salinan umpan (`muatUmpanTersimpan()`) langsung menggambar sampulnya dari
//! berkas, bukan menunggu jaringan.
//!
//! Hasilnya sepanjang `urls`: jalur `app:/data/sampul/<hash url>.jpg` bila berkasnya sudah ada (`fs.stat`) atau
//! baru diunduh, selain itu "" (Contract lalu memakai URL-nya). Paling banyak [`BATAS`] URL pertama; sisanya "".
//!
//! Rantainya, satu permintaan host per langkah: buat folder → (stat → unduh bila belum ada) × n → daftar isi folder
//! → hapus berkas sampul yang tidak dipakai lagi satu per satu. Unduhan memakai `Request::body_to`: host menulis
//! badan 2xx langsung ke berkasnya (lewat berkas sementara yang baru menggantikan tujuannya setelah lengkap), jadi
//! bita gambar tidak melewati crate ini. Satu URL yang gagal hanya menjadi "" untuk URL itu.

use std::collections::HashSet;

use exact_runner::{Outcome, Request};
use serde_json::{json, Value as Json};

pub const FOLDER: &str = "app:/data/sampul";
/// URL yang diproses per panggilan.
pub const BATAS: usize = 24;
const BATAS_WAKTU_MS: u32 = 15_000;
/// Batas satu gambar sampul (`_next/image` w=640–1200 JPEG jauh di bawahnya).
const BATAS_BITA: u32 = 8 << 20;

/// FNV-1a 64-bit: hash yang sama di setiap proses dan versi (tanpa benih acak `HashMap`).
pub fn hash(url: &str) -> u64 {
    url.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3))
}

/// "0123456789abcdef.jpg": nama berkas sampul untuk `url`.
pub fn nama_berkas(url: &str) -> String {
    format!("{:016x}.jpg", hash(url))
}

/// `app:/data/sampul/<hash>.jpg`.
pub fn jalur(url: &str) -> String {
    format!("{FOLDER}/{}", nama_berkas(url))
}

/// Nama berkas yang dibuat rantai ini (16 hex + ".jpg"). Berkas lain di folder, termasuk berkas sementara host
/// (`.exact-….partial`, `.ibex-…`) milik unduhan yang mungkin sedang berjalan, tidak pernah dihapus.
pub fn berkas_sampul(nama: &str) -> bool {
    nama.len() == 20 && nama.ends_with(".jpg") && nama[..16].bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// URL yang bisa diunduh (selain itu hasilnya "").
fn bisa_diunduh(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// Nama berkas yang dipakai `urls` (dalam batas), untuk dipertahankan saat rantai lain membersihkan folder.
pub fn nama_dipakai(urls: &[String]) -> Vec<String> {
    urls.iter().take(BATAS).filter(|u| bisa_diunduh(u)).map(|u| nama_berkas(u)).collect()
}

fn fs(op: &str, args: Json) -> Request {
    exact_data::storage::request(op, args)
}

#[derive(Debug, Clone, PartialEq)]
enum Tahap {
    Folder,
    Periksa(usize),
    Unduh(usize),
    /// Host tanpa `body_to` mengembalikan badannya: ditulis dengan `fs.atomicWriteFile`.
    Tulis(usize),
    /// Membuang berkas kosong atau bukan gambar yang terlanjur ditulis.
    Buang(usize),
    Daftar,
    Hapus,
}

/// Langkah berikutnya: permintaan untuk host, atau daftar akhir.
#[derive(Debug)]
pub enum Langkah {
    Minta(Request),
    Selesai(Vec<String>),
}

/// Satu `siapkanSampul(urls)` yang sedang berjalan.
#[derive(Debug)]
pub struct Rantai {
    urls: Vec<String>,
    hasil: Vec<String>,
    tahap: Tahap,
    hapus: Vec<String>,
}

impl Rantai {
    /// Rantai baru dan langkah pertamanya. Tanpa URL yang bisa diunduh (umpan belum termuat) jawabannya langsung,
    /// dan folder tidak dibersihkan: daftar kosong di awal aplikasi tidak boleh menghapus sampul yang tersimpan.
    pub fn mulai(urls: Vec<String>) -> (Rantai, Langkah) {
        let hasil = vec![String::new(); urls.len()];
        let kosong = nama_dipakai(&urls).is_empty();
        let mut rantai = Rantai { urls, hasil, tahap: Tahap::Folder, hapus: Vec::new() };
        if kosong {
            let hasil = std::mem::take(&mut rantai.hasil);
            return (rantai, Langkah::Selesai(hasil));
        }
        // `fs.mkdir` membuat folder beserta induknya dan berhasil bila sudah ada.
        (rantai, Langkah::Minta(fs("fs.mkdir", json!({ "path": FOLDER }))))
    }

    /// Nama berkas yang dipakai rantai ini (lihat [`nama_dipakai`]).
    pub fn nama(&self) -> Vec<String> {
        nama_dipakai(&self.urls)
    }

    /// Jawaban tanpa I/O bila tidak ada yang bisa diunduh (saat bake): semua "".
    pub fn tanpa_unduhan(urls: &[String]) -> Option<Vec<String>> {
        nama_dipakai(urls).is_empty().then(|| vec![String::new(); urls.len()])
    }

    /// Periksa URL berikutnya mulai dari `i`, atau mulai membersihkan folder bila semua sudah.
    fn berikut(&mut self, mut i: usize) -> Langkah {
        while i < self.urls.len().min(BATAS) {
            let url = &self.urls[i];
            if bisa_diunduh(url) {
                // URL yang sama di depan sudah diperiksa: pakai hasilnya.
                match self.urls[..i].iter().position(|u| u == url) {
                    Some(j) => self.hasil[i] = self.hasil[j].clone(),
                    None => {
                        self.tahap = Tahap::Periksa(i);
                        return Langkah::Minta(fs("fs.stat", json!({ "path": jalur(url) })));
                    }
                }
            }
            i += 1;
        }
        self.tahap = Tahap::Daftar;
        Langkah::Minta(fs("fs.readdir", json!({ "path": FOLDER })))
    }

    fn unduh(&mut self, i: usize) -> Langkah {
        self.tahap = Tahap::Unduh(i);
        let url = &self.urls[i];
        // Tanpa accept-encoding: JPEG tidak dimampatkan lagi, dan berkasnya harus bita gambar apa adanya.
        Langkah::Minta(
            Request::get(url).timeout(BATAS_WAKTU_MS).independent_http(BATAS_BITA).body_to(jalur(url)),
        )
    }

    fn selesai(&mut self) -> Langkah {
        Langkah::Selesai(std::mem::take(&mut self.hasil))
    }

    /// Olah balasan langkah yang sedang berjalan dan tentukan langkah berikutnya. `lain` = nama berkas yang dipakai
    /// panggilan `siapkanSampul` lain (sasaran lain), yang tidak boleh terhapus saat folder dibersihkan.
    pub fn lanjut(&mut self, outcome: Outcome, lain: &HashSet<String>) -> Langkah {
        match self.tahap.clone() {
            Tahap::Folder => match exact_data::storage::response(outcome) {
                Ok(_) => self.berikut(0),
                // Tanpa folder tidak ada yang bisa disimpan: semua jatuh ke URL.
                Err(_) => self.selesai(),
            },
            Tahap::Periksa(i) => {
                let ada = exact_data::storage::response(outcome).is_ok_and(|v| {
                    v["isFile"].as_bool().unwrap_or(false) && v["size"].as_u64().unwrap_or(0) > 0
                });
                if ada {
                    self.hasil[i] = jalur(&self.urls[i]);
                    self.berikut(i + 1)
                } else {
                    self.unduh(i)
                }
            }
            Tahap::Unduh(i) => match outcome {
                Outcome::Response(r) if (200..300).contains(&r.status) => {
                    let gambar = r
                        .headers
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                        .is_none_or(|(_, v)| v.trim().to_ascii_lowercase().starts_with("image/"));
                    match r.body_written() {
                        Some(n) if n > 0 && gambar => {
                            self.hasil[i] = jalur(&self.urls[i]);
                            self.berikut(i + 1)
                        }
                        // Badan kosong atau bukan gambar sudah tertulis: buang lagi.
                        Some(_) => {
                            self.tahap = Tahap::Buang(i);
                            Langkah::Minta(fs("fs.rm", json!({ "path": jalur(&self.urls[i]) })))
                        }
                        None if gambar && !r.body.is_empty() => {
                            self.tahap = Tahap::Tulis(i);
                            let bita: Vec<Json> = r.body.iter().map(|&b| json!(b)).collect();
                            Langkah::Minta(fs("fs.atomicWriteFile", json!({ "path": jalur(&self.urls[i]), "bytes": bita })))
                        }
                        None => self.berikut(i + 1),
                    }
                }
                _ => self.berikut(i + 1),
            },
            Tahap::Tulis(i) => {
                if exact_data::storage::response(outcome).is_ok() {
                    self.hasil[i] = jalur(&self.urls[i]);
                }
                self.berikut(i + 1)
            }
            Tahap::Buang(i) => self.berikut(i + 1),
            Tahap::Daftar => {
                let nama: Vec<String> = match exact_data::storage::response(outcome) {
                    Ok(Json::Array(a)) => a.into_iter().filter_map(|n| n.as_str().map(str::to_string)).collect(),
                    _ => Vec::new(),
                };
                let milik: HashSet<String> = self.nama().into_iter().collect();
                self.hapus = nama.into_iter().filter(|n| berkas_sampul(n) && !milik.contains(n) && !lain.contains(n)).collect();
                self.hapus_berikut()
            }
            // Gagal menghapus satu berkas tidak menggagalkan sampul yang sudah siap.
            Tahap::Hapus => self.hapus_berikut(),
        }
    }

    fn hapus_berikut(&mut self) -> Langkah {
        match self.hapus.pop() {
            Some(nama) => {
                self.tahap = Tahap::Hapus;
                Langkah::Minta(fs("fs.rm", json!({ "path": format!("{FOLDER}/{nama}") })))
            }
            None => self.selesai(),
        }
    }
}

#[cfg(test)]
mod uji {
    use super::*;
    use exact_runner::{FailureKind, Response, BODY_WRITTEN};

    fn op(langkah: &Langkah) -> (String, Json) {
        let Langkah::Minta(r) = langkah else { panic!("bukan permintaan: {langkah:?}") };
        let j: Json = serde_json::from_slice(r.storage.as_ref().expect("permintaan berkas")).unwrap();
        (j["op"].as_str().unwrap().to_string(), j["args"].clone())
    }

    fn berkas(v: Json) -> Outcome {
        Outcome::Storage(serde_json::to_vec(&v).unwrap())
    }

    fn unduhan(status: u16, tertulis: Option<u64>, jenis: &str, body: &[u8]) -> Outcome {
        let mut headers = vec![("Content-Type".to_string(), jenis.to_string())];
        if let Some(n) = tertulis {
            headers.push((BODY_WRITTEN.to_string(), n.to_string()));
        }
        Outcome::Response(Response { status, headers, body: body.to_vec() })
    }

    const A: &str = "https://situs.test/_next/image?url=%2Fa.jpg&w=640&q=75";
    const B: &str = "https://situs.test/_next/image?url=%2Fb.jpg&w=640&q=75";
    const C: &str = "https://situs.test/_next/image?url=%2Fc.jpg&w=640&q=75";

    #[test]
    fn nama_berkas_stabil() {
        // FNV-1a 64-bit dari teks kosong dan "a" (vektor uji resminya).
        assert_eq!(hash(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(hash("a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(nama_berkas(A), nama_berkas(A));
        assert_ne!(nama_berkas(A), nama_berkas(B));
        assert!(jalur(A).starts_with("app:/data/sampul/") && jalur(A).ends_with(".jpg"));
        assert_eq!(jalur(A).len(), "app:/data/sampul/".len() + 20);
        assert!(berkas_sampul(&nama_berkas(A)));
        assert!(!berkas_sampul(".exact-0123456789abcdef0123456789abcdef.partial"));
        assert!(!berkas_sampul("0123456789ABCDEF.jpg"));
        assert!(!berkas_sampul("catatan.txt"));
    }

    #[test]
    fn tanpa_url_langsung_dijawab_tanpa_membersihkan() {
        let (_, l) = Rantai::mulai(vec![String::new(), "assets/x.png".into()]);
        assert!(matches!(l, Langkah::Selesai(ref h) if h == &["", ""]));
        let (_, l) = Rantai::mulai(Vec::new());
        assert!(matches!(l, Langkah::Selesai(ref h) if h.is_empty()));
        assert_eq!(Rantai::tanpa_unduhan(&[String::new()]), Some(vec![String::new()]));
        assert_eq!(Rantai::tanpa_unduhan(&[A.into()]), None);
    }

    #[test]
    fn rantai_lengkap() {
        let urls: Vec<String> = vec![A.into(), "".into(), B.into(), A.into(), C.into()];
        let lama = format!("{:016x}.jpg", 7u64);
        let milik_lain = format!("{:016x}.jpg", 9u64);
        let (mut r, l) = Rantai::mulai(urls);
        let lain: HashSet<String> = [milik_lain.clone()].into();
        assert_eq!(op(&l), ("fs.mkdir".into(), json!({ "path": FOLDER })));
        // A sudah ada di HP.
        let l = r.lanjut(berkas(Json::Null), &lain);
        assert_eq!(op(&l), ("fs.stat".into(), json!({ "path": jalur(A) })));
        let l = r.lanjut(berkas(json!({ "size": 1234, "isFile": true, "isDirectory": false })), &lain);
        // B belum ada: diunduh ke berkasnya, 15 detik, tanpa accept-encoding.
        assert_eq!(op(&l), ("fs.stat".into(), json!({ "path": jalur(B) })));
        let l = r.lanjut(berkas(json!({ "error": "ENOENT: no such file or directory" })), &lain);
        let Langkah::Minta(minta) = &l else { panic!() };
        assert_eq!((minta.method.as_str(), minta.url.as_str()), ("GET", B));
        assert_eq!(minta.body_to.as_deref(), Some(jalur(B).as_str()));
        assert_eq!(minta.timeout_ms, Some(15_000));
        assert!(!minta.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("accept-encoding")));
        assert_eq!(minta.body_to_refusal(), None);
        assert_eq!(minta.timeout_refusal(), None);
        assert_eq!(minta.ceiling_refusal(), None);
        let l = r.lanjut(unduhan(200, Some(5000), "image/jpeg", b""), &lain);
        // A kedua memakai hasil A pertama tanpa stat; C gagal diunduh (404).
        assert_eq!(op(&l), ("fs.stat".into(), json!({ "path": jalur(C) })));
        let l = r.lanjut(berkas(json!({ "error": "ENOENT" })), &lain);
        assert!(matches!(&l, Langkah::Minta(m) if m.url == C));
        let l = r.lanjut(unduhan(404, None, "text/html", b"<html>"), &lain);
        // Bersihkan: hanya berkas sampul yang tidak dipakai siapa pun.
        assert_eq!(op(&l), ("fs.readdir".into(), json!({ "path": FOLDER })));
        let isi = json!([nama_berkas(A), nama_berkas(B), lama, milik_lain, ".exact-ab.partial", "lain.txt"]);
        let l = r.lanjut(berkas(isi), &lain);
        assert_eq!(op(&l), ("fs.rm".into(), json!({ "path": format!("{FOLDER}/{lama}") })));
        let l = r.lanjut(berkas(json!({ "error": "EACCES" })), &lain);
        let Langkah::Selesai(hasil) = l else { panic!("selesai") };
        assert_eq!(hasil, [jalur(A), String::new(), jalur(B), jalur(A), String::new()]);
    }

    #[test]
    fn unduhan_kosong_bukan_gambar_atau_tanpa_body_to() {
        let lain: HashSet<String> = HashSet::new();
        let mulai = || {
            let (mut r, _) = Rantai::mulai(vec![A.into()]);
            r.lanjut(berkas(Json::Null), &lain);
            r.lanjut(berkas(json!({ "error": "ENOENT" })), &lain);
            r
        };
        // Badan kosong tertulis: dibuang, hasilnya "".
        let mut r = mulai();
        let l = r.lanjut(unduhan(200, Some(0), "image/jpeg", b""), &lain);
        assert_eq!(op(&l), ("fs.rm".into(), json!({ "path": jalur(A) })));
        assert_eq!(op(&r.lanjut(berkas(Json::Null), &lain)).0, "fs.readdir");
        let Langkah::Selesai(h) = r.lanjut(berkas(json!([])), &lain) else { panic!() };
        assert_eq!(h, [""]);
        // Halaman HTML 200 bukan sampul.
        let mut r = mulai();
        assert_eq!(op(&r.lanjut(unduhan(200, Some(300), "text/html; charset=utf-8", b""), &lain)).0, "fs.rm");
        // Host tanpa body_to: badannya ditulis sendiri sebagai bita.
        let mut r = mulai();
        let l = r.lanjut(unduhan(200, None, "image/jpeg", &[0xFF, 0xD8, 0xFF]), &lain);
        assert_eq!(op(&l), ("fs.atomicWriteFile".into(), json!({ "path": jalur(A), "bytes": [255, 216, 255] })));
        assert_eq!(op(&r.lanjut(berkas(Json::Null), &lain)).0, "fs.readdir");
        let Langkah::Selesai(h) = r.lanjut(berkas(json!([nama_berkas(A)])), &lain) else { panic!() };
        assert_eq!(h, [jalur(A)]);
        // Jaringan putus: "" dan lanjut.
        let mut r = mulai();
        let l = r.lanjut(Outcome::Failed { kind: FailureKind::Timeout, message: "deadline".into() }, &lain);
        assert_eq!(op(&l).0, "fs.readdir");
    }

    #[test]
    fn folder_gagal_dan_batas() {
        let lain: HashSet<String> = HashSet::new();
        let (mut r, _) = Rantai::mulai(vec![A.into()]);
        let Langkah::Selesai(h) = r.lanjut(berkas(json!({ "error": "outside the app's grants" })), &lain) else { panic!() };
        assert_eq!(h, [""]);
        // Lebih dari 24 URL: sisanya "" dan tidak diperiksa.
        let urls: Vec<String> = (0..30).map(|i| format!("https://situs.test/{i}.jpg")).collect();
        let (mut r, _) = Rantai::mulai(urls);
        let mut l = r.lanjut(berkas(Json::Null), &lain);
        let mut stat = 0;
        while let ("fs.stat", _) = (op(&l).0.as_str(), ()) {
            stat += 1;
            l = r.lanjut(berkas(json!({ "size": 10, "isFile": true })), &lain);
        }
        assert_eq!(stat, BATAS);
        assert_eq!(op(&l).0, "fs.readdir");
        let Langkah::Selesai(h) = r.lanjut(berkas(json!([])), &lain) else { panic!() };
        assert_eq!(h.len(), 30);
        assert!(h[..BATAS].iter().all(|p| p.starts_with("app:/data/sampul/")));
        assert!(h[BATAS..].iter().all(String::is_empty));
    }
}
