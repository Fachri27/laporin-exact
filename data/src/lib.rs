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
//! | `kirimLaporan(…)` | baca, lalu tulis berkas yang sama | balasan `{ ok, galat }` |

pub mod hotspot;
mod shapes;
pub mod simpan;
pub mod teks;
pub mod umpan;

use exact_runner::failure::FailureCode;
use exact_runner::{Answer, DataError, DataSource, FailureKind, Outcome, Request, Store, Value};

use shapes::{Balasan, ContractValue, Komentar, LaporanSaya};

/// Batas balasan HTTP: umpan ±160 laporan dan hotspot hari puncak (puluhan ribu baris) muat.
const BATAS_BALASAN: u32 = 16 << 20;
const BATAS_WAKTU_MS: u32 = 20_000;

#[derive(Debug, Default)]
pub struct Laporin;

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

impl DataSource for Laporin {
    fn app_id(&self) -> &str {
        // Sama dengan app.json "app": { "id" }: plan yang dipanggang menyebut id ini.
        "id.laporin.exact"
    }

    fn grants(&self) -> &str {
        "net.fetch https://fire.nusantara.earth\n\
         net.fetch https://geoplatform.nusantara.earth\n\
         fs.read app:/data/laporan-saya.json\n\
         fs.write app:/data/laporan-saya.json"
    }

    /// Saat build (bake) dan tanpa I/O: semua sumber dijawab saat aplikasi berjalan.
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "muatKomentar" if teks_arg(args, 0)?.is_empty() => Ok(Vec::<Komentar>::new().to_value()),
            "muatUmpan" | "muatHotspot" | "muatKomentar" | "muatLaporanSaya" | "kirimLaporan" => {
                Err(DataError::Unavailable("dijawab saat berjalan".into()))
            }
            lain => Err(DataError::UnknownSource(lain.into())),
        }
    }

    fn answer(&mut self, _store: &mut Store, source: &str, args: &[Value]) -> Result<Answer, DataError> {
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
            "muatLaporanSaya" | "kirimLaporan" => Answer::Later(simpan::minta_baca()),
            lain => return Err(DataError::UnknownSource(lain.into())),
        })
    }

    fn parse(&mut self, _store: &mut Store, source: &str, args: &[Value], outcome: Outcome) -> Result<Answer, DataError> {
        match source {
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
            // Penolakan menulis adalah balasan `{ ok: false, galat }`, supaya formulir bisa menampilkannya.
            "kirimLaporan" => Ok(match simpan::baca_balasan(outcome) {
                Ok(simpan::Balasan::Isi(mut semua)) => {
                    let (judul, deskripsi, nama) = (teks_arg(args, 0)?, teks_arg(args, 1)?, teks_arg(args, 2)?);
                    let (epoch, selisih) = (angka_arg(args, 4)?, angka_arg(args, 5)?);
                    if judul.trim().is_empty() {
                        return Ok(balasan(false, "Isi judul singkat dulu."));
                    }
                    semua.insert(0, simpan::catatan(judul, deskripsi, nama, epoch, selisih));
                    Answer::Later(simpan::minta_tulis(&semua))
                }
                Ok(simpan::Balasan::Tertulis) => balasan(true, ""),
                Err(e) => balasan(false, &format!("Laporan gagal disimpan di HP: {e}")),
            }),
            lain => Err(DataError::UnknownSource(lain.into())),
        }
    }
}
