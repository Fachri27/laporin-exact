//! Inti Rust Laporin: menjawab setiap sumber data yang dipanggil `app.contract`.
//!
//! Runner Exact tidak pernah melakukan I/O sendiri: `answer` menyerahkan permintaan (HTTP atau
//! berkas) ke host, lalu `parse` mengolah balasannya menjadi shape yang dideklarasikan Contract.
//!
//! | Sumber | Permintaan | Olahan |
//! |---|---|---|
//! | `muatUmpan()` | GET fire.nusantara.earth/api/umpan | `umpan::olah_umpan` |
//! | `muatKomentar(id)` | GET fire.nusantara.earth/api/laporan/{id}/komentar | `umpan::olah_komentar` |
//! | `muatHotspot()` | GET geoplatform.nusantara.earth/api/hotspot | `hotspot::olah_hotspot` |
//! | `muatLaporanSaya()` | baca app:/data/laporan-saya.json | `simpan::ke_laporan` |
//! | `muatSesi()`, `muatLaporanAkun()` | GET /api/mobile/saya, /saya/laporan dengan sesi di `Store` (`akun`) |
//! | `masukUji(nama)`, `keluar()` | POST /api/mobile/masuk/uji, /keluar | balasan `{ ok, galat }` |
//! | `kirimLaporan(…)` | tiket, perkecil & baca foto, POST multipart ke `LAPORIN_API` (`kirim`) | balasan `{ ok, galat }` |

pub mod akun;
pub mod hotspot;
pub mod kirim;
mod shapes;
pub mod simpan;
pub mod teks;
pub mod umpan;

use exact_runner::failure::FailureCode;
use exact_runner::{Answer, DataError, DataSource, FailureKind, Outcome, Request, Store, Value};

use shapes::{Balasan, ContractValue, Komentar, LaporanAkun, LaporanSaya, Sesi};

/// Batas balasan HTTP: umpan ±160 laporan dan hotspot hari puncak (puluhan ribu baris) muat.
const BATAS_BALASAN: u32 = 16 << 20;
const BATAS_WAKTU_MS: u32 = 20_000;

#[derive(Debug, Default)]
pub struct Laporin {
    /// Kiriman yang sedang berjalan di antara `answer` dan `parse` (satu per waktu).
    kiriman: Option<kirim::Kiriman>,
}

fn teks_arg(args: &[Value], i: usize) -> Result<&str, DataError> {
    args.get(i).and_then(Value::as_str).ok_or_else(|| DataError::BadArguments(format!("argumen {i} harus teks")))
}

fn angka_arg(args: &[Value], i: usize) -> Result<f64, DataError> {
    args.get(i).and_then(Value::as_number).ok_or_else(|| DataError::BadArguments(format!("argumen {i} harus angka")))
}

fn gagal(kode: FailureCode, pesan: impl Into<String>) -> DataError {
    DataError::Failed(kode, pesan.into(), String::new())
}

fn ambil(url: &str) -> Answer {
    Answer::Later(
        Request::get(url)
            .header("accept", "application/json")
            .timeout(BATAS_WAKTU_MS)
            .independent_http(BATAS_BALASAN),
    )
}

/// Badan balasan 2xx, atau kegagalan yang disebut `failure(x).code` di Contract.
fn badan(outcome: Outcome) -> Result<Vec<u8>, DataError> {
    match outcome {
        Outcome::Response(r) if (200..300).contains(&r.status) => Ok(r.body),
        Outcome::Response(r) => Err(gagal(FailureCode::Error, format!("HTTP {}", r.status))),
        Outcome::Failed { kind: FailureKind::Timeout, message } => Err(gagal(FailureCode::Timeout, message)),
        Outcome::Failed { kind: FailureKind::Refused, message } => Err(gagal(FailureCode::Refused, message)),
        Outcome::Failed { message, .. } => Err(gagal(FailureCode::Offline, message)),
        lain => Err(DataError::Unavailable(format!("balasan tak terduga: {}", lain.summary()))),
    }
}

fn balasan(ok: bool, galat: &str) -> Answer {
    Answer::Now(Balasan { ok, galat: galat.into() }.to_value())
}

/// Argumen `kirimLaporan(judul, deskripsi, nama, anonim, epochMs, selisihMenit, lampiran)`.
fn isian(args: &[Value]) -> Result<kirim::Isian, DataError> {
    Ok(kirim::Isian {
        judul: teks_arg(args, 0)?.trim().to_string(),
        deskripsi: teks_arg(args, 1)?.trim().to_string(),
        nama: teks_arg(args, 2)?.trim().to_string(),
        anonim: args.get(3).and_then(Value::as_bool).ok_or_else(|| DataError::BadArguments("argumen 3 harus bool".into()))?,
        epoch_ms: angka_arg(args, 4)?,
        selisih_menit: angka_arg(args, 5)?,
        lampiran: args
            .get(6)
            .and_then(<Vec<String> as ContractValue>::from_value)
            .ok_or_else(|| DataError::BadArguments("argumen 6 harus daftar teks".into()))?,
    })
}

impl DataSource for Laporin {
    fn app_id(&self) -> &str {
        // Sama dengan app.json "app": { "id" }: plan yang dipanggang menyebut id ini.
        "id.laporin.exact"
    }

    fn grants(&self) -> &str {
        concat!(
            "net.fetch https://fire.nusantara.earth\n",
            "net.fetch https://geoplatform.nusantara.earth\n",
            "net.fetch ",
            env!("LAPORIN_API"),
            "\ndevice.photos fotoAlasan\n",
            "fs.read app:/data/laporan-saya.json\n",
            "fs.write app:/data/laporan-saya.json\n",
            "fs.read app:/tmp\n",
            "fs.write app:/tmp\n",
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
            "masukUji" | "keluar" => Err(DataError::Unavailable("dijawab saat berjalan".into())),
            "muatUmpan" | "muatHotspot" | "muatKomentar" | "muatLaporanSaya" | "kirimLaporan" => {
                Err(DataError::Unavailable("dijawab saat berjalan".into()))
            }
            lain => Err(DataError::UnknownSource(lain.into())),
        }
    }

    fn answer(&mut self, store: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
        Ok(match source {
            "muatUmpan" => ambil(&format!("{}/api/umpan?bahasa=id", umpan::SITUS)),
            "muatHotspot" => ambil(&format!("{}/api/hotspot", hotspot::GEO)),
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
                Some(s) => Answer::Later(akun::minta_profil(&s.token)),
                None => Answer::Now(Sesi::default().to_value()),
            },
            "muatLaporanAkun" => match akun::tersimpan(store) {
                Some(s) => Answer::Later(akun::minta_laporan(&s.token)),
                None => Answer::Now(Vec::<LaporanAkun>::new().to_value()),
            },
            "masukUji" => {
                let nama = teks_arg(args, 0)?.trim();
                Answer::Later(akun::minta_masuk_uji(if nama.is_empty() { "Penguji" } else { nama }))
            }
            "keluar" => match akun::tersimpan(store) {
                Some(s) => Answer::Later(akun::minta_keluar(&s.token)),
                None => balasan(true, ""),
            },
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

    fn parse(&mut self, store: &mut Store, source: &str, _args: &[Value], outcome: Outcome) -> Result<Answer, DataError> {
        match source {
            "muatSesi" => Ok(Answer::Now(akun::sesi(store, &outcome).to_value())),
            "muatLaporanAkun" => {
                let daftar = akun::laporan(store, &outcome).map_err(|e| gagal(FailureCode::Offline, e))?;
                Ok(Answer::Now(daftar.to_value()))
            }
            "masukUji" => Ok(match akun::simpan_masuk(store, &outcome) {
                Ok(_) => balasan(true, ""),
                Err(e) => balasan(false, &e),
            }),
            // Keluar di HP selalu berhasil; server yang tak terjangkau hanya tidak sempat mencabut tokennya.
            "keluar" => {
                akun::lupakan(store);
                Ok(balasan(true, ""))
            }
            "muatUmpan" => {
                let u = umpan::olah_umpan(&badan(outcome)?).map_err(|e| gagal(FailureCode::Shape, e))?;
                Ok(Answer::Now(u.to_value()))
            }
            "muatHotspot" => {
                let h = hotspot::olah_hotspot(&badan(outcome)?).map_err(|e| gagal(FailureCode::Shape, e))?;
                Ok(Answer::Now(h.to_value()))
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
            "kirimLaporan" => {
                let Some(kiriman) = self.kiriman.as_mut() else {
                    return Ok(balasan(false, "Kiriman tidak dikenal. Coba kirim lagi."));
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
}
