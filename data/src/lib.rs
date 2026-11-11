//! Inti Rust Laporin: menjawab setiap sumber data yang dipanggil `app.contract`.
//!
//! Runner Exact tidak pernah melakukan I/O sendiri: `answer` menyerahkan permintaan (HTTP atau
//! berkas) ke host, lalu `parse` mengolah balasannya menjadi shape yang dideklarasikan Contract.
//!
//! | Sumber | Permintaan | Olahan |
//! |---|---|---|
//! | `aturBahasa(b)` | — ("EN" = Inggris, selain itu Indonesia; berlaku global lewat `teks::atur_bahasa`) | balasan `{ ok: true, galat: "" }` |
//! | `muatUmpan()` | GET `LAPORIN_SITUS`/api/umpan?bahasa=id\|en (gzip), lalu tulis badannya ke app:/data/umpan.json | `umpan::olah_umpan` |
//! | `muatUmpanTersimpan()` | baca app:/data/umpan.json (tidak ada/rusak = umpan kosong) | `umpan::olah_umpan` |
//! | `siapkanSampul(urls)` | mkdir app:/data/sampul, (stat → GET ke berkas) per URL (≤ 24), readdir & rm sampul lama | `string[]`: jalur `app:/data/sampul/<hash>.jpg` atau "" (`sampul`) |
//! | `muatKomentar(id)` | GET `LAPORIN_SITUS`/api/laporan/{id}/komentar (gzip) | `umpan::olah_komentar` |
//! | `muatLaporanSaya()` | baca app:/data/laporan-saya.json | `simpan::ke_laporan` |
//! | `muatSesi()`, `muatLaporanAkun(email)` | (pulihkan app:/data/sesi.json,) GET /api/mobile/saya, /saya/laporan dengan sesi di `Store` (`akun`) |
//! | `masukGoogle(idToken)`, `masukEmail(email, sandi)`, `daftarEmail(nama, email, sandi)`, `keluar()` | POST /api/mobile/masuk/google, /masuk/email, /daftar, /keluar | balasan `{ ok, galat }` |
//! | `hapusAkun()` | DELETE /api/mobile/saya dengan sesi (batas 60 detik), lalu sesi di HP dikosongkan seperti keluar | balasan `{ ok, galat }` |
//! | `alamatCaptcha()` | — (`{API}/api/mobile/captcha` bila server https, "" untuk mock) | teks |
//! | `alamatKabar()` | — (`{SITUS}/api/umpan/halaman`, `{API}/api/mobile/live`, `SITUS`: yang dicek paket notifikasi di latar) | `AlamatKabar` |
//! | `klienGoogle()` | — (client ID web Google dari `LAPORIN_GOOGLE_KLIEN` saat build) | teks |
//! | `muatVerifikasi(email)`, `ajukanVerifikasi(nik, foto)` | GET /api/mobile/saya/verifikasi; perkecil & baca foto KTP, POST multipart | `verifikasi` |
//! | `muatDisukai(email)`, `aturSuka(id, suka)` | GET /api/mobile/saya/disukai, POST/DELETE /laporan/{id}/suka | id teks, balasan `{ ok, galat }` |
//! | `muatSiaranLangsung()` | GET /api/mobile/live | `siaran::daftar` |
//! | `mulaiSiaran(judul, publik, adaLokasi, lat, lng)`, `tontonSiaran(ruang)` | POST /api/mobile/live/token, /live/tonton (dengan sesi bila ada) | `siaran::token`: `{ url, token, ruang, galat }` |
//! | `kirimLaporan(…)` | tiket, perkecil & baca foto, POST multipart ke `LAPORIN_API` (`kirim`) | balasan `{ ok, galat }` |
//!
//! Teks yang dibuat crate ini (tanggal, koordinat, label bawaan, galat buatan aplikasi) mengikuti `aturBahasa`;
//! galat yang dikirim server diteruskan apa adanya. Sumber yang keluarannya bergantung pada bahasa: `muatUmpan`,
//! `muatUmpanTersimpan`, `muatKomentar`, `muatLaporanSaya`, `muatLaporanAkun`, `muatSiaranLangsung` (dan pesan galat
//! `muatVerifikasi`).

pub mod akun;
pub mod kirim;
pub mod sampul;
pub mod siaran;
mod shapes;
pub mod simpan;
pub mod teks;
pub mod umpan;
pub mod verifikasi;

use std::collections::HashMap;

use exact_runner::failure::FailureCode;
use exact_runner::{Answer, DataError, DataSource, FailureKind, Outcome, Request, Store, Target, Value};

use shapes::{AlamatKabar, Balasan, ContractValue, Komentar, LaporanAkun, LaporanSaya, Sesi, StatusKtp, TokenLive, Umpan};
use teks::dua;

/// Batas balasan HTTP: umpan ±160 laporan muat dengan longgar.
const BATAS_BALASAN: u32 = 16 << 20;
const BATAS_WAKTU_MS: u32 = 20_000;

#[derive(Debug, Default)]
pub struct Laporin {
    /// Kiriman yang sedang berjalan di antara `answer` dan `parse` (satu per waktu).
    kiriman: Option<kirim::Kiriman>,
    /// Pengajuan verifikasi KTP yang sedang berjalan (perkecil → baca → unggah).
    ajuan: Option<verifikasi::Ajuan>,
    /// `app:/data/sesi.json` sudah dibaca di proses ini (sekali saja: sesi yang dihapus tidak dipulihkan lagi).
    sesi_dibaca: bool,
    /// Langkah `muatSesi` yang sedang berjalan.
    tahap_sesi: TahapSesi,
    /// Masuk/keluar sedang menulis `app:/data/sesi.json` setelah balasan server.
    menulis_sesi: bool,
    /// Daftar siaran publik terakhir; token pratinjaunya diminta satu per satu setelah daftar tiba.
    siaran: Vec<shapes::SiaranPublik>,
    /// Ruang yang token pratinjaunya sedang diminta (langkah kedua `muatSiaranLangsung`).
    siaran_minta: Option<String>,
    /// Token pratinjau per ruang (url, token), dipakai lagi selama siarannya masih ada di daftar.
    pratinjau: std::collections::HashMap<String, (String, String)>,
    /// Ruang yang token pratinjaunya gagal di putaran ini (dicoba lagi saat daftar dimuat ulang).
    pratinjau_gagal: Vec<String>,
    /// Sasaran (resource/mutation) yang sedang dijawab lewat `answer_for`/`parse_for`; `None` lewat `answer`/`parse`.
    /// Rantai yang bisa berjalan untuk beberapa sasaran sekaligus disimpan per sasaran (runner menjaga paling banyak
    /// satu permintaan per sasaran).
    sasaran: Option<Target>,
    /// Umpan yang sudah diolah, menunggu salinannya ditulis ke app:/data/umpan.json (langkah kedua `muatUmpan`).
    umpan_tulis: HashMap<Option<Target>, Value>,
    /// `siapkanSampul` yang sedang berjalan.
    sampul: HashMap<Option<Target>, sampul::Rantai>,
    /// Nama berkas sampul terakhir yang dipakai tiap sasaran, supaya pembersihan satu sasaran tidak menghapus
    /// sampul sasaran lain.
    sampul_tetap: HashMap<Option<Target>, Vec<String>>,
}

impl Laporin {
    /// Nama berkas sampul yang dipakai sasaran selain `kunci` (rantai yang sedang berjalan dan hasil terakhirnya),
    /// supaya pembersihan folder satu `siapkanSampul` tidak menghapus sampul yang lain.
    fn sampul_lain(&self, kunci: Option<Target>) -> std::collections::HashSet<String> {
        let berjalan = self.sampul.iter().filter(|(k, _)| **k != kunci).flat_map(|(_, r)| r.nama());
        let terakhir = self.sampul_tetap.iter().filter(|(k, _)| **k != kunci).flat_map(|(_, n)| n.iter().cloned());
        berjalan.chain(terakhir).collect()
    }
}

/// Kartu tab Live yang diberi video pratinjau (sisanya tetap avatar).
const BATAS_PRATINJAU: usize = 6;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
enum TahapSesi {
    /// GET /saya dengan sesi di Store.
    #[default]
    Profil,
    /// Membaca `app:/data/sesi.json`.
    Pulihkan,
    /// Menghapus salinan sesi yang ditolak server (401).
    Hapus,
}

fn teks_arg(args: &[Value], i: usize) -> Result<&str, DataError> {
    args.get(i)
        .and_then(Value::as_str)
        .ok_or_else(|| DataError::BadArguments(dua!(format!("argumen {i} harus teks"), format!("argument {i} must be text"))))
}

fn angka_arg(args: &[Value], i: usize) -> Result<f64, DataError> {
    args.get(i)
        .and_then(Value::as_number)
        .ok_or_else(|| DataError::BadArguments(dua!(format!("argumen {i} harus angka"), format!("argument {i} must be a number"))))
}

fn bool_arg(args: &[Value], i: usize) -> Result<bool, DataError> {
    args.get(i)
        .and_then(Value::as_bool)
        .ok_or_else(|| DataError::BadArguments(dua!(format!("argumen {i} harus bool"), format!("argument {i} must be a boolean"))))
}

fn daftar_arg(args: &[Value], i: usize) -> Result<Vec<String>, DataError> {
    args.get(i).and_then(<Vec<String> as ContractValue>::from_value).ok_or_else(|| {
        DataError::BadArguments(dua!(format!("argumen {i} harus daftar teks"), format!("argument {i} must be a list of text")))
    })
}

fn gagal(kode: FailureCode, pesan: impl Into<String>) -> DataError {
    DataError::Failed(kode, pesan.into(), String::new())
}

/// GET JSON ke situs. Transport host tidak meminta kompresi sendiri: gzip diminta di sini dan dibuka [`badan`].
fn ambil(url: &str) -> Answer {
    Answer::Later(
        Request::get(url)
            .header("accept", "application/json")
            .header("accept-encoding", "gzip")
            .timeout(BATAS_WAKTU_MS)
            .independent_http(BATAS_BALASAN),
    )
}

/// Badan balasan 2xx (dibuka bila gzip), atau kegagalan yang disebut `failure(x).code` di Contract.
fn badan(outcome: Outcome) -> Result<Vec<u8>, DataError> {
    match outcome {
        Outcome::Response(r) if (200..300).contains(&r.status) => {
            umpan::buka_gzip(&r.headers, r.body).map_err(|e| gagal(FailureCode::Shape, e))
        }
        Outcome::Response(r) => Err(gagal(FailureCode::Error, format!("HTTP {}", r.status))),
        Outcome::Failed { kind: FailureKind::Timeout, message } => Err(gagal(FailureCode::Timeout, message)),
        Outcome::Failed { kind: FailureKind::Refused, message } => Err(gagal(FailureCode::Refused, message)),
        Outcome::Failed { message, .. } => Err(gagal(FailureCode::Offline, message)),
        lain => Err(DataError::Unavailable(dua!(
            format!("balasan tak terduga: {}", lain.summary()),
            format!("unexpected reply: {}", lain.summary())
        ))),
    }
}

fn balasan(ok: bool, galat: &str) -> Answer {
    Answer::Now(Balasan { ok, galat: galat.into() }.to_value())
}

fn token_galat(galat: &str) -> Answer {
    Answer::Now(TokenLive { galat: galat.into(), ..TokenLive::default() }.to_value())
}

/// Argumen `kirimLaporan(judul, deskripsi, nama, anonim, epochMs, selisihMenit, lampiran, adaLokasi, lat, lng, captcha)`.
fn isian(args: &[Value]) -> Result<kirim::Isian, DataError> {
    Ok(kirim::Isian {
        judul: teks_arg(args, 0)?.trim().to_string(),
        deskripsi: teks_arg(args, 1)?.trim().to_string(),
        nama: teks_arg(args, 2)?.trim().to_string(),
        anonim: bool_arg(args, 3)?,
        epoch_ms: angka_arg(args, 4)?,
        selisih_menit: angka_arg(args, 5)?,
        lampiran: daftar_arg(args, 6)?,
        // `adaLokasi`, `lat`, `lng`: angka NaN ditolak di batas data, jadi ada-tidaknya titik dikirim terpisah.
        lokasi: match args.get(7).and_then(Value::as_bool) {
            Some(true) => Some((angka_arg(args, 8)?, angka_arg(args, 9)?)),
            _ => None,
        },
        captcha: teks_arg(args, 10)?.trim().to_string(),
    })
}

/// Alamat yang dicek paket notifikasi (`paket/notifikasi`) di latar, tanpa Rust: halaman pertama umpan (15 laporan
/// terbaru, ±7 KB gzip, bukan seluruh umpan), daftar siaran publik, dan asal gambar `/_next/image`.
fn alamat_kabar() -> AlamatKabar {
    AlamatKabar {
        umpan: format!("{}/api/umpan/halaman", umpan::SITUS),
        live: format!("{}/api/mobile/live", kirim::API),
        situs: umpan::SITUS.to_string(),
    }
}

impl DataSource for Laporin {
    fn app_id(&self) -> &str {
        // Sama dengan app.json "app": { "id" }: plan yang dipanggang menyebut id ini.
        "id.laporin.exact"
    }

    fn grants(&self) -> &str {
        concat!(
            "net.fetch ",
            env!("LAPORIN_GRANT_SITUS"),
            "\n",
            "net.fetch ",
            env!("LAPORIN_API"),
            "\ndevice.photos fotoAlasan\n",
            "device.microphone mikrofonAlasan\n",
            "fs.read app:/data/laporan-saya.json\n",
            "fs.write app:/data/laporan-saya.json\n",
            "fs.read app:/data/sesi.json\n",
            "fs.write app:/data/sesi.json\n",
            "fs.read app:/data/umpan.json\n",
            "fs.write app:/data/umpan.json\n",
            "fs.read app:/data/sampul\n",
            "fs.write app:/data/sampul\n",
            "fs.read app:/tmp\n",
            "fs.write app:/tmp\n",
            "device.geolocation lokasiAlasan\n",
            "device.camera kameraAlasan\n",
            "device.notifications notifikasiAlasan\n",
            "secret.keep sesi-laporin"
        )
    }

    /// Saat build (bake) dan tanpa I/O: semua sumber dijawab saat aplikasi berjalan.
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "muatKomentar" if teks_arg(args, 0)?.is_empty() => Ok(Vec::<Komentar>::new().to_value()),
            // Bingkai pertama sebagai tamu; sesi tersimpan dibaca saat berjalan.
            "muatSesi" => Ok(Sesi::default().to_value()),
            "muatLaporanAkun" => Ok(Vec::<LaporanAkun>::new().to_value()),
            "muatDisukai" => Ok(Vec::<String>::new().to_value()),
            "muatVerifikasi" => Ok(StatusKtp::default().to_value()),
            "klienGoogle" => Ok(env!("LAPORIN_GOOGLE_KLIEN").to_string().to_value()),
            "alamatCaptcha" => Ok(kirim::alamat_captcha().to_value()),
            "alamatKabar" => Ok(alamat_kabar().to_value()),
            "aturBahasa" => {
                teks::atur_bahasa(args.first().and_then(Value::as_str).unwrap_or(""));
                Ok(Balasan { ok: true, galat: String::new() }.to_value())
            }
            // Tanpa URL yang bisa diunduh jawabannya tanpa I/O (semua ""); selain itu dijawab saat berjalan.
            "siapkanSampul" => match sampul::Rantai::tanpa_unduhan(&daftar_arg(args, 0)?) {
                Some(kosong) => Ok(kosong.to_value()),
                None => Err(DataError::Unavailable("dijawab saat berjalan".into())),
            },
            // Jangan dijawab saat bake: nilai bake diambil resource saat boot alih-alih bertanya ke HP.
            "muatUmpanTersimpan" => Err(DataError::Unavailable("dijawab saat berjalan".into())),
            "ajukanVerifikasi" => Err(DataError::Unavailable("dijawab saat berjalan".into())),
            "masukGoogle" | "masukEmail" | "daftarEmail" | "aturSuka" | "keluar" | "hapusAkun" | "kirimKomentar" | "laporkanKonten" => Err(DataError::Unavailable("dijawab saat berjalan".into())),
            "muatUmpan" | "muatKomentar" | "muatLaporanSaya" | "kirimLaporan" | "muatSiaranLangsung" | "mulaiSiaran" | "tontonSiaran" => {
                Err(DataError::Unavailable("dijawab saat berjalan".into()))
            }
            lain => Err(DataError::UnknownSource(lain.into())),
        }
    }

    fn answer(&mut self, store: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        let kunci = self.sasaran;
        Ok(match source {
            "aturBahasa" => {
                teks::atur_bahasa(args.first().and_then(Value::as_str).unwrap_or(""));
                balasan(true, "")
            }
            "muatUmpan" => {
                self.umpan_tulis.remove(&kunci);
                ambil(&umpan::alamat_umpan())
            }
            "muatUmpanTersimpan" => Answer::Later(umpan::minta_baca_simpanan()),
            "siapkanSampul" => {
                let (rantai, langkah) = sampul::Rantai::mulai(daftar_arg(args, 0)?);
                self.sampul.remove(&kunci);
                match langkah {
                    sampul::Langkah::Minta(minta) => {
                        self.sampul.insert(kunci, rantai);
                        Answer::Later(minta)
                    }
                    sampul::Langkah::Selesai(hasil) => Answer::Now(hasil.to_value()),
                }
            }
            "muatKomentar" => {
                let id = teks_arg(args, 0)?;
                if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
                    Answer::Now(Vec::<Komentar>::new().to_value())
                } else {
                    ambil(&format!("{}/api/laporan/{id}/komentar", umpan::SITUS))
                }
            }
            "muatLaporanSaya" => Answer::Later(simpan::minta_baca()),
            "muatSesi" => match akun::tersimpan(store) {
                Some(s) => {
                    self.tahap_sesi = TahapSesi::Profil;
                    Answer::Later(akun::minta_profil(&s.token))
                }
                None if !self.sesi_dibaca => {
                    self.sesi_dibaca = true;
                    self.tahap_sesi = TahapSesi::Pulihkan;
                    Answer::Later(akun::minta_baca_sesi())
                }
                None => Answer::Now(Sesi::default().to_value()),
            },
            "muatLaporanAkun" => match akun::tersimpan(store) {
                Some(s) => Answer::Later(akun::minta_laporan(&s.token)),
                None => Answer::Now(Vec::<LaporanAkun>::new().to_value()),
            },
            "masukGoogle" => match teks_arg(args, 0)?.trim() {
                "" => balasan(false, dua!("Google tidak memberi token masuk.", "Google didn't return a sign-in token.")),
                t => Answer::Later(akun::minta_masuk_google(t)),
            },
            "masukEmail" => Answer::Later(akun::minta_masuk_email(teks_arg(args, 0)?.trim(), teks_arg(args, 1)?)),
            "daftarEmail" => {
                let (nama, email, sandi) = (teks_arg(args, 0)?.trim(), teks_arg(args, 1)?.trim(), teks_arg(args, 2)?);
                Answer::Later(akun::minta_daftar(nama, email, sandi))
            }
            "muatDisukai" => match akun::tersimpan(store) {
                Some(s) => Answer::Later(akun::minta_disukai(&s.token)),
                None => Answer::Now(Vec::<String>::new().to_value()),
            },
            "aturSuka" => {
                let id = teks_arg(args, 0)?;
                let suka = bool_arg(args, 1)?;
                match akun::tersimpan(store) {
                    None => balasan(false, dua!("Masuk dulu untuk menyukai.", "Sign in to like reports.")),
                    Some(s) => Answer::Later(akun::minta_suka(&s.token, id, suka)),
                }
            }
            "hapusAkun" => match akun::tersimpan(store) {
                Some(s) => Answer::Later(akun::minta_hapus(&s.token)),
                None => balasan(false, dua!("Masuk dulu untuk menghapus akun.", "Sign in to delete your account.")),
            },
            "keluar" => match akun::tersimpan(store) {
                Some(s) => Answer::Later(akun::minta_keluar(&s.token)),
                None => {
                    self.menulis_sesi = true;
                    Answer::Later(akun::minta_tulis_sesi("{}"))
                }
            },
            "kirimKomentar" => {
                let (id, isi) = (teks_arg(args, 0)?, teks_arg(args, 1)?.trim());
                match akun::tersimpan(store) {
                    None => balasan(false, dua!("Masuk dulu untuk berkomentar.", "Sign in to comment.")),
                    Some(_) if isi.is_empty() => balasan(false, dua!("Tulis komentarnya dulu.", "Write your comment first.")),
                    Some(s) => Answer::Later(akun::minta_komentar(&s.token, id, isi)),
                }
            }
            "laporkanKonten" => {
                let (jenis, id, alasan, catatan) = (teks_arg(args, 0)?, teks_arg(args, 1)?, teks_arg(args, 2)?, teks_arg(args, 3)?);
                Answer::Later(akun::minta_laporkan(akun::tersimpan(store).map(|s| s.token), jenis, id, alasan, catatan))
            }
            "muatSiaranLangsung" => {
                self.siaran_minta = None;
                Answer::Later(siaran::minta_daftar())
            }
            "mulaiSiaran" => {
                let judul = teks_arg(args, 0)?.trim();
                let publik = args.get(1).and_then(Value::as_bool).unwrap_or(false);
                let ada_lokasi = args.get(2).and_then(Value::as_bool).unwrap_or(false);
                let lokasi = if ada_lokasi { Some((angka_arg(args, 3)?, angka_arg(args, 4)?)) } else { None };
                let sesi = akun::tersimpan(store);
                if judul.is_empty() {
                    token_galat(dua!("Tulis singkat apa yang sedang terjadi.", "Briefly describe what's happening."))
                } else if publik && sesi.is_none() {
                    token_galat(dua!(
                        "Masuk dulu untuk menyiarkan ke tab Live warga lain.",
                        "Sign in to broadcast on other people's Live tab."
                    ))
                } else {
                    Answer::Later(siaran::minta_token(sesi.as_ref().map(|s| s.token.as_str()), &siaran::Isian { judul, publik, lokasi }))
                }
            }
            "tontonSiaran" => {
                let sesi = akun::tersimpan(store);
                Answer::Later(siaran::minta_tonton(sesi.as_ref().map(|s| s.token.as_str()), teks_arg(args, 0)?))
            }
            "klienGoogle" => Answer::Now(env!("LAPORIN_GOOGLE_KLIEN").to_string().to_value()),
            "alamatCaptcha" => Answer::Now(kirim::alamat_captcha().to_value()),
            "alamatKabar" => Answer::Now(alamat_kabar().to_value()),
            "muatVerifikasi" => match akun::tersimpan(store) {
                Some(s) => Answer::Later(verifikasi::minta_status(&s.token)),
                None => Answer::Now(StatusKtp::default().to_value()),
            },
            "ajukanVerifikasi" => {
                let (nik, foto) = (teks_arg(args, 0)?.trim(), teks_arg(args, 1)?);
                match verifikasi::Ajuan::mulai(nik, foto, akun::tersimpan(store).map(|s| s.token)) {
                    Ok((ajuan, minta)) => {
                        self.ajuan = Some(ajuan);
                        Answer::Later(minta)
                    }
                    Err(galat) => balasan(false, &galat),
                }
            }
            "kirimLaporan" => match kirim::Kiriman::mulai(isian(args)?, akun::tersimpan(store).map(|s| s.token)) {
                Ok((kiriman, minta)) => {
                    self.kiriman = Some(kiriman);
                    Answer::Later(minta)
                }
                Err(galat) => balasan(false, &galat),
            },
            lain => return Err(DataError::UnknownSource(lain.into())),
        })
    }

    fn parse(&mut self, store: &mut Store, source: &str, args: &[Value], outcome: Outcome) -> Result<Answer, DataError> {
        let kunci = self.sasaran;
        match source {
            "muatSesi" => match self.tahap_sesi {
                TahapSesi::Pulihkan => {
                    self.tahap_sesi = TahapSesi::Profil;
                    Ok(match akun::pulihkan(store, outcome) {
                        Some(s) => Answer::Later(akun::minta_profil(&s.token)),
                        None => Answer::Now(Sesi::default().to_value()),
                    })
                }
                TahapSesi::Hapus => {
                    self.tahap_sesi = TahapSesi::Profil;
                    Ok(Answer::Now(Sesi::default().to_value()))
                }
                TahapSesi::Profil => {
                    let ditolak = matches!(&outcome, Outcome::Response(r) if r.status == 401);
                    let sesi = akun::sesi(store, &outcome);
                    if ditolak {
                        self.tahap_sesi = TahapSesi::Hapus;
                        return Ok(Answer::Later(akun::minta_tulis_sesi("{}")));
                    }
                    Ok(Answer::Now(sesi.to_value()))
                }
            },
            "muatLaporanAkun" => {
                let daftar = akun::laporan(store, &outcome).map_err(|e| gagal(FailureCode::Offline, e))?;
                Ok(Answer::Now(daftar.to_value()))
            }
            "muatDisukai" => {
                let daftar = akun::disukai(store, &outcome).map_err(|e| gagal(FailureCode::Offline, e))?;
                Ok(Answer::Now(daftar.to_value()))
            }
            // Langkah kedua: salinan sesi sudah ditulis (atau gagal ditulis: sesi tetap berlaku sampai aplikasi ditutup).
            "masukGoogle" | "masukEmail" | "daftarEmail" | "keluar" | "hapusAkun" if self.menulis_sesi => {
                self.menulis_sesi = false;
                Ok(balasan(true, ""))
            }
            "masukGoogle" | "masukEmail" | "daftarEmail" => Ok(match akun::simpan_masuk(store, &outcome) {
                Ok(sesi) => {
                    self.menulis_sesi = true;
                    Answer::Later(akun::minta_tulis_sesi(&sesi))
                }
                Err(e) => balasan(false, &e),
            }),
            "kirimKomentar" | "laporkanKonten" | "aturSuka" => Ok(match &outcome {
                Outcome::Response(r) if (200..300).contains(&r.status) => balasan(true, ""),
                lain => balasan(false, &akun::galat(lain)),
            }),
            // Keluar di HP selalu berhasil; server yang tak terjangkau hanya tidak sempat mencabut tokennya.
            // Terhapus di server: sesi di HP dilupakan seperti keluar. Gagal: galat server, atau keterangan jaringan.
            "hapusAkun" => Ok(match &outcome {
                Outcome::Response(r) if (200..300).contains(&r.status) => {
                    akun::lupakan(store);
                    self.menulis_sesi = true;
                    Answer::Later(akun::minta_tulis_sesi("{}"))
                }
                lain => balasan(false, &akun::galat(lain)),
            }),
            "keluar" => {
                akun::lupakan(store);
                self.menulis_sesi = true;
                Ok(Answer::Later(akun::minta_tulis_sesi("{}")))
            }
            // Daftar dulu, lalu token pratinjau tiap siaran yang belum punya (paling banyak BATAS_PRATINJAU).
            "muatSiaranLangsung" => {
                match self.siaran_minta.take() {
                    Some(ruang) => {
                        let t = siaran::token(&outcome, &ruang);
                        if t.token.is_empty() {
                            self.pratinjau_gagal.push(ruang);
                        } else {
                            self.pratinjau.insert(ruang, (t.url, t.token));
                        }
                    }
                    None => {
                        self.siaran = siaran::daftar(&outcome).map_err(|e| gagal(FailureCode::Offline, e))?;
                        self.pratinjau_gagal.clear();
                        let ada: Vec<String> = self.siaran.iter().map(|s| s.ruang.clone()).collect();
                        self.pratinjau.retain(|r, _| ada.contains(r));
                    }
                }
                let berikut = self
                    .siaran
                    .iter()
                    .take(BATAS_PRATINJAU)
                    .find(|s| !self.pratinjau.contains_key(&s.ruang) && !self.pratinjau_gagal.contains(&s.ruang));
                if let Some(s) = berikut {
                    self.siaran_minta = Some(s.ruang.clone());
                    return Ok(Answer::Later(siaran::minta_pratinjau(&s.ruang)));
                }
                let daftar: Vec<shapes::SiaranPublik> = self
                    .siaran
                    .iter()
                    .cloned()
                    .map(|mut s| {
                        if let Some((url, token)) = self.pratinjau.get(&s.ruang) {
                            s.pratinjau_url = url.clone();
                            s.pratinjau_token = token.clone();
                        }
                        s
                    })
                    .collect();
                Ok(Answer::Now(daftar.to_value()))
            }
            "mulaiSiaran" => Ok(Answer::Now(siaran::token(&outcome, "").to_value())),
            "tontonSiaran" => Ok(Answer::Now(siaran::token(&outcome, teks_arg(args, 0)?).to_value())),
            // Langkah kedua: salinan sudah ditulis, atau gagal ditulis (umpan dari jaringan tetap dijawab).
            "muatUmpan" if self.umpan_tulis.contains_key(&kunci) => {
                Ok(Answer::Now(self.umpan_tulis.remove(&kunci).unwrap_or_else(|| Umpan::default().to_value())))
            }
            "muatUmpan" => {
                let isi = badan(outcome)?;
                let nilai = umpan::olah_umpan(&isi).map_err(|e| gagal(FailureCode::Shape, e))?.to_value();
                Ok(match umpan::minta_simpan(&isi) {
                    Some(tulis) => {
                        self.umpan_tulis.insert(kunci, nilai);
                        Answer::Later(tulis)
                    }
                    None => Answer::Now(nilai),
                })
            }
            "muatUmpanTersimpan" => Ok(Answer::Now(umpan::olah_simpanan(outcome).to_value())),
            "siapkanSampul" => {
                let lain = self.sampul_lain(kunci);
                let Some(rantai) = self.sampul.get_mut(&kunci) else {
                    let n = daftar_arg(args, 0)?.len();
                    return Ok(Answer::Now(vec![String::new(); n].to_value()));
                };
                Ok(match rantai.lanjut(outcome, &lain) {
                    sampul::Langkah::Minta(minta) => Answer::Later(minta),
                    sampul::Langkah::Selesai(hasil) => {
                        if let Some(rantai) = self.sampul.remove(&kunci) {
                            self.sampul_tetap.insert(kunci, rantai.nama());
                        }
                        Answer::Now(hasil.to_value())
                    }
                })
            }
            "muatKomentar" => {
                let k = umpan::olah_komentar(&badan(outcome)?).map_err(|e| gagal(FailureCode::Shape, e))?;
                Ok(Answer::Now(k.to_value()))
            }
            "muatLaporanSaya" => match simpan::baca_balasan(outcome).map_err(|e| gagal(FailureCode::Storage, e))? {
                simpan::Balasan::Isi(semua) => Ok(Answer::Now(simpan::ke_laporan(&semua).to_value())),
                simpan::Balasan::Tertulis => Ok(Answer::Now(Vec::<LaporanSaya>::new().to_value())),
            },
            // Penolakan server adalah balasan `{ ok: false, galat }`, supaya formulir bisa menampilkannya.
            "muatVerifikasi" => {
                let status = verifikasi::status(&outcome).map_err(|e| gagal(FailureCode::Offline, e))?;
                Ok(Answer::Now(status.to_value()))
            }
            "ajukanVerifikasi" => {
                let Some(ajuan) = self.ajuan.as_mut() else {
                    return Ok(balasan(
                        false,
                        dua!("Pengajuan tidak dikenal. Coba kirim lagi.", "Unknown verification request. Try sending again."),
                    ));
                };
                Ok(match ajuan.lanjut(outcome) {
                    verifikasi::Langkah::Minta(minta) => Answer::Later(minta),
                    verifikasi::Langkah::Selesai(ok, galat) => {
                        self.ajuan = None;
                        balasan(ok, &galat)
                    }
                })
            }
            "kirimLaporan" => {
                let Some(kiriman) = self.kiriman.as_mut() else {
                    return Ok(balasan(false, dua!("Kiriman tidak dikenal. Coba kirim lagi.", "Unknown report submission. Try sending again.")));
                };
                Ok(match kiriman.lanjut(outcome) {
                    kirim::Langkah::Minta(minta) => Answer::Later(minta),
                    kirim::Langkah::Selesai(ok, galat) => {
                        self.kiriman = None;
                        balasan(ok, &galat)
                    }
                })
            }
            lain => Err(DataError::UnknownSource(lain.into())),
        }
    }

    fn answer_for(&mut self, target: Target, store: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        self.sasaran = Some(target);
        let jawab = self.answer(store, source, args);
        self.sasaran = None;
        jawab
    }

    fn parse_for(
        &mut self,
        target: Target,
        store: &mut Store,
        source: &str,
        args: &[Value],
        outcome: Outcome,
    ) -> Result<Answer, DataError> {
        self.sasaran = Some(target);
        let jawab = self.parse(store, source, args, outcome);
        self.sasaran = None;
        jawab
    }
}

#[cfg(test)]
mod uji {
    use super::*;
    use exact_runner::Response;
    use shapes::Umpan;

    fn later(a: Result<Answer, DataError>) -> Request {
        match a.expect("jawaban") {
            Answer::Later(r) => r,
            Answer::Now(v) => panic!("bukan permintaan: {v:?}"),
        }
    }

    fn now(a: Result<Answer, DataError>) -> Value {
        match a.expect("jawaban") {
            Answer::Now(v) => v,
            Answer::Later(r) => panic!("bukan nilai: {r:?}"),
        }
    }

    fn op(r: &Request) -> serde_json::Value {
        serde_json::from_slice(r.storage.as_ref().expect("permintaan berkas")).unwrap()
    }

    fn berkas(v: serde_json::Value) -> Outcome {
        Outcome::Storage(serde_json::to_vec(&v).unwrap())
    }

    const UMPAN: &str = r#"[{"id":1,"judul":"Asap","tanggal":"4 Agustus 2026","lat":-2,"lng":113,
        "media":[{"jenis":"gambar","url":"/a.jpg"}]}]"#;

    #[test]
    fn hapus_akun_menghapus_di_server_lalu_melupakan_sesi() {
        let mut l = Laporin::default();
        // Store dengan hibah aplikasi (`secret.keep sesi-laporin`), supaya sesi bisa disimpan.
        let mut store = Store::new(l.grants(), Vec::<(String, Vec<u8>)>::new());
        let v = now(l.answer(&mut store, "hapusAkun", &[]));
        assert_eq!(Balasan::from_value(&v).unwrap().galat, "Masuk dulu untuk menghapus akun.");
        let masuk = Outcome::Response(Response { status: 200, headers: Vec::new(),
            body: br#"{"token":"t1","pengguna":{"nama":"Ani","email":"ani@contoh.id"}}"#.to_vec() });
        akun::simpan_masuk(&mut store, &masuk).unwrap();
        let minta = later(l.answer(&mut store, "hapusAkun", &[]));
        assert_eq!((minta.method.as_str(), minta.url.as_str()), ("DELETE", format!("{}/api/mobile/saya", kirim::API).as_str()));
        assert!(minta.headers.contains(&("authorization".into(), "Bearer t1".into())));
        assert_eq!(minta.timeout_ms, Some(60_000));
        // Ditolak server: galatnya diteruskan, sesi tetap.
        let tolak = Outcome::Response(Response { status: 403, headers: Vec::new(), body: br#"{"galat":"Tidak boleh."}"#.to_vec() });
        let v = now(l.parse(&mut store, "hapusAkun", &[], tolak));
        assert_eq!(Balasan::from_value(&v).unwrap().galat, "Tidak boleh.");
        assert!(akun::tersimpan(&store).is_some());
        // Terhapus: sesi dilupakan, salinannya di HP dikosongkan, lalu berhasil.
        let ok = Outcome::Response(Response { status: 200, headers: Vec::new(), body: br#"{"ok":true}"#.to_vec() });
        let tulis = later(l.parse(&mut store, "hapusAkun", &[], ok));
        assert!(tulis.storage.is_some());
        assert!(akun::tersimpan(&store).is_none());
        let v = now(l.parse(&mut store, "hapusAkun", &[], berkas(serde_json::json!({}))));
        assert_eq!(Balasan::from_value(&v), Some(Balasan { ok: true, galat: String::new() }));
    }

    #[test]
    fn atur_bahasa_di_kedua_jalur() {
        let (mut l, mut store) = (Laporin::default(), Store::default());
        let b = now(l.answer(&mut store, "aturBahasa", &["EN".to_string().to_value()]));
        assert_eq!(Balasan::from_value(&b), Some(Balasan { ok: true, galat: String::new() }));
        assert!(teks::inggris());
        let b = now(l.answer(&mut store, "aturBahasa", &["ID".to_string().to_value()]));
        assert_eq!(Balasan::from_value(&b).map(|b| b.ok), Some(true));
        assert!(!teks::inggris());
        l.query("aturBahasa", &["EN".to_string().to_value()]).unwrap();
        assert!(teks::inggris());
        // Selain "EN" (juga tanpa argumen) = Indonesia.
        now(l.answer(&mut store, "aturBahasa", &[]));
        assert!(!teks::inggris());
    }

    #[test]
    fn galat_buatan_aplikasi_mengikuti_bahasa() {
        let (mut l, mut store) = (Laporin::default(), Store::default());
        let galat = |l: &mut Laporin, store: &mut Store| {
            let v = now(l.answer(store, "kirimKomentar", &["1".to_string().to_value(), "hai".to_string().to_value()]));
            Balasan::from_value(&v).unwrap().galat
        };
        assert_eq!(galat(&mut l, &mut store), "Masuk dulu untuk berkomentar.");
        now(l.answer(&mut store, "aturBahasa", &["EN".to_string().to_value()]));
        assert_eq!(galat(&mut l, &mut store), "Sign in to comment.");
        let v = now(l.answer(&mut store, "aturSuka", &["1".to_string().to_value(), true.to_value()]));
        assert_eq!(Balasan::from_value(&v).unwrap().galat, "Sign in to like reports.");
        let v = now(l.answer(&mut store, "mulaiSiaran", &["".to_string().to_value(), false.to_value(), false.to_value()]));
        assert_eq!(TokenLive::from_value(&v).unwrap().galat, "Briefly describe what's happening.");
        assert_eq!(verifikasi::periksa_nik("12"), Some("The NIK must be 16 digits."));
        assert_eq!(
            verifikasi::Ajuan::mulai("6271010101900001", "", None).err().unwrap(),
            "Sign in to verify your ID."
        );
        match l.answer(&mut store, "muatKomentar", &[]) {
            Err(DataError::BadArguments(m)) => assert_eq!(m, "argument 0 must be text"),
            lain => panic!("{lain:?}"),
        }
        let token = siaran::token(&Outcome::Response(Response { status: 200, headers: Vec::new(), body: b"{}".to_vec() }), "");
        assert_eq!(token.galat, "The live server didn't return a token.");
        let tolak = Outcome::Response(Response { status: 500, headers: Vec::new(), body: b"<html>".to_vec() });
        assert_eq!(akun::galat(&tolak), "The server refused (HTTP 500).");
        // Galat dari server diteruskan apa adanya.
        let server = Outcome::Response(Response { status: 409, headers: Vec::new(), body: br#"{"galat":"Email sudah terdaftar."}"#.to_vec() });
        assert_eq!(akun::galat(&server), "Email sudah terdaftar.");
    }

    #[test]
    fn laporan_saya_diformat_ulang_saat_dibaca() {
        let c = simpan::catatan("Asap", "Tebal", "", false, 1_794_242_400_000.0, 420.0);
        assert_eq!(c["dibuat"], "9 Nov 2026, 23.40");
        let lama = serde_json::json!({ "id": "1", "dibuat": "1 Okt 2026, 08.00" });
        assert_eq!(simpan::ke_laporan(&[c.clone(), lama.clone()])[0].dibuat, "9 Nov 2026, 23.40");
        teks::atur_bahasa("EN");
        let l = simpan::ke_laporan(&[c, lama]);
        assert_eq!((l[0].dibuat.as_str(), l[1].dibuat.as_str()), ("9 Nov 2026, 23:40", "1 Okt 2026, 08.00"));
    }

    #[test]
    fn umpan_gzip_lalu_salinan_lalu_nilai() {
        use std::io::Write;
        let (mut l, mut store) = (Laporin::default(), Store::default());
        let minta = later(l.answer(&mut store, "muatUmpan", &[]));
        assert_eq!(minta.url, format!("{}/api/umpan?bahasa=id", umpan::SITUS));
        assert!(minta.headers.contains(&("accept-encoding".into(), "gzip".into())));
        let mut z = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        z.write_all(UMPAN.as_bytes()).unwrap();
        let headers = vec![("Content-Encoding".to_string(), "gzip".to_string())];
        let balas = Outcome::Response(Response { status: 200, headers, body: z.finish().unwrap() });
        // Badan yang sudah dibuka ditulis ke app:/data/umpan.json, lalu nilainya dijawab.
        let tulis = later(l.parse(&mut store, "muatUmpan", &[], balas));
        let j = op(&tulis);
        assert_eq!((j["op"].as_str(), j["args"]["path"].as_str(), j["args"]["text"].as_str()), (Some("fs.atomicWriteFile"), Some("app:/data/umpan.json"), Some(UMPAN)));
        let u = Umpan::from_value(&now(l.parse(&mut store, "muatUmpan", &[], berkas(serde_json::Value::Null)))).unwrap();
        assert_eq!((u.laporan.len(), u.laporan[0].tanggal_pendek.as_str()), (1, "4 AGU"));
        // Gagal menulis salinan tidak menggagalkan umpan.
        later(l.answer(&mut store, "muatUmpan", &[]));
        let polos = Outcome::Response(Response { status: 200, headers: Vec::new(), body: UMPAN.as_bytes().to_vec() });
        later(l.parse(&mut store, "muatUmpan", &[], polos));
        let gagal = berkas(serde_json::json!({ "error": "outside the app's grants" }));
        assert_eq!(Umpan::from_value(&now(l.parse(&mut store, "muatUmpan", &[], gagal))).unwrap().laporan.len(), 1);
        // Galat HTTP tetap galat resource, tanpa menulis apa pun.
        later(l.answer(&mut store, "muatUmpan", &[]));
        let galat = Outcome::Response(Response { status: 502, headers: Vec::new(), body: Vec::new() });
        assert!(l.parse(&mut store, "muatUmpan", &[], galat).is_err());
        assert!(l.umpan_tulis.is_empty());
        // Bahasa Inggris meminta umpan berbahasa Inggris dari server.
        teks::atur_bahasa("EN");
        assert_eq!(later(l.answer(&mut store, "muatUmpan", &[])).url, format!("{}/api/umpan?bahasa=en", umpan::SITUS));
    }

    #[test]
    fn alamat_kabar_untuk_notifikasi() {
        let a = AlamatKabar::from_value(&Laporin::default().query("alamatKabar", &[]).unwrap()).unwrap();
        assert_eq!(a.umpan, format!("{}/api/umpan/halaman", umpan::SITUS));
        assert_eq!(a.live, format!("{}/api/mobile/live", kirim::API));
        assert!(Laporin::default().grants().contains("device.notifications notifikasiAlasan"));
    }

    #[test]
    fn umpan_tersimpan() {
        let (mut l, mut store) = (Laporin::default(), Store::default());
        assert!(l.query("muatUmpanTersimpan", &[]).is_err(), "tidak dijawab saat bake");
        let baca = later(l.answer(&mut store, "muatUmpanTersimpan", &[]));
        assert_eq!(op(&baca)["args"]["path"], "app:/data/umpan.json");
        let isi = berkas(serde_json::json!({ "base64": exact_runner::agent::base64(UMPAN.as_bytes()) }));
        let u = Umpan::from_value(&now(l.parse(&mut store, "muatUmpanTersimpan", &[], isi))).unwrap();
        assert_eq!(u.laporan.len(), 1);
        let tidak_ada = berkas(serde_json::json!({ "error": "ENOENT: no such file or directory" }));
        let u = Umpan::from_value(&now(l.parse(&mut store, "muatUmpanTersimpan", &[], tidak_ada))).unwrap();
        assert!(u.laporan.is_empty());
    }

    #[test]
    fn sampul_per_sasaran() {
        let (mut l, mut store) = (Laporin::default(), Store::default());
        let a = "https://situs.test/a.jpg".to_string();
        let b = "https://situs.test/b.jpg".to_string();
        let kosong = vec![String::new(), "x".to_string()].to_value();
        // Tanpa URL yang bisa diunduh: langsung, juga saat bake.
        assert_eq!(<Vec<String>>::from_value(&l.query("siapkanSampul", std::slice::from_ref(&kosong)).unwrap()), Some(vec![String::new(); 2]));
        assert_eq!(<Vec<String>>::from_value(&now(l.answer(&mut store, "siapkanSampul", &[kosong]))), Some(vec![String::new(); 2]));
        assert!(l.query("siapkanSampul", &[vec![a.clone()].to_value()]).is_err());
        // Dua sasaran berjalan bersamaan tanpa saling mengganggu; pembersihan satu tidak menghapus sampul yang lain.
        let (t1, t2) = (Target::Resource(1), Target::Resource(2));
        let (x1, x2) = (vec![a.clone()].to_value(), vec![b.clone()].to_value());
        let m1 = later(l.answer_for(t1, &mut store, "siapkanSampul", std::slice::from_ref(&x1)));
        let m2 = later(l.answer_for(t2, &mut store, "siapkanSampul", std::slice::from_ref(&x2)));
        assert_eq!((op(&m1)["op"].as_str(), op(&m2)["op"].as_str()), (Some("fs.mkdir"), Some("fs.mkdir")));
        let s1 = later(l.parse_for(t1, &mut store, "siapkanSampul", std::slice::from_ref(&x1), berkas(serde_json::Value::Null)));
        assert_eq!(op(&s1)["args"]["path"], sampul::jalur(&a));
        let s2 = later(l.parse_for(t2, &mut store, "siapkanSampul", std::slice::from_ref(&x2), berkas(serde_json::Value::Null)));
        assert_eq!(op(&s2)["args"]["path"], sampul::jalur(&b));
        let ada = || berkas(serde_json::json!({ "size": 9, "isFile": true }));
        let d1 = later(l.parse_for(t1, &mut store, "siapkanSampul", std::slice::from_ref(&x1), ada()));
        assert_eq!(op(&d1)["op"], "fs.readdir");
        let isi = serde_json::json!([sampul::nama_berkas(&a), sampul::nama_berkas(&b)]);
        let h1 = now(l.parse_for(t1, &mut store, "siapkanSampul", &[x1], berkas(isi.clone())));
        assert_eq!(<Vec<String>>::from_value(&h1), Some(vec![sampul::jalur(&a)]));
        let d2 = later(l.parse_for(t2, &mut store, "siapkanSampul", std::slice::from_ref(&x2), ada()));
        assert_eq!(op(&d2)["op"], "fs.readdir");
        let h2 = now(l.parse_for(t2, &mut store, "siapkanSampul", &[x2], berkas(isi)));
        assert_eq!(<Vec<String>>::from_value(&h2), Some(vec![sampul::jalur(&b)]));
    }
}
