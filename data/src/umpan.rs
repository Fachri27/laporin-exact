//! Umpan laporan terverifikasi (`GET {SITUS}/api/umpan?bahasa=id|en`) dan komentarnya
//! (`GET {SITUS}/api/laporan/{id}/komentar`), dengan aturan klien laporin-apps: lewati laporan tanpa
//! id angka, judul, koordinat sah (bukan 0,0) atau larik media. `SITUS` adalah `LAPORIN_SITUS` saat build
//! (bawaan: mock-server, yang meneruskan produksi dan menambahkan suka & komentar mock).
//!
//! Server memilih bahasa judul, deskripsi dan tanggal dengan `?bahasa=` (EN kosong jatuh ke ID); crate ini
//! menyeragamkan tanggalnya ("4 November 2026") dan membuat label bawaan dalam bahasa aktif.
//!
//! Transport HTTP host tidak meminta atau membuka kompresi sendiri: permintaan JSON ke situs membawa
//! `accept-encoding: gzip` (umpan ±225 KB mentah, ±51 KB gzip) dan [`buka_gzip`] membukanya.
//!
//! Badan umpan terakhir yang sah disalin ke [`BERKAS_UMPAN`], supaya Beranda langsung terisi saat aplikasi dibuka
//! dingin (`muatUmpanTersimpan()`), sebelum jaringan menjawab.

use std::io::Read;

use exact_runner::{Outcome, Request};
use serde_json::{json, Value as Json};

use crate::shapes::{Komentar, Laporan, Pulau, Sumber, Umpan};
use crate::teks::{self, dua};

pub const SITUS: &str = env!("LAPORIN_SITUS");

/// Salinan badan `GET /api/umpan` terakhir yang sah (JSON yang sudah dibuka gzip-nya).
pub const BERKAS_UMPAN: &str = "app:/data/umpan.json";

/// Batas JSON setelah dibuka (bom gzip tidak boleh menghabiskan memori).
const BATAS_TERBUKA: u64 = 16 << 20;

/// Nama pengirim bawaan bila media pertama tidak berketerangan.
const KREDIT_ID: &str = "Laporan warga";
const KREDIT_EN: &str = "Citizen report";

/// Alamat umpan dalam bahasa aktif.
pub fn alamat_umpan() -> String {
    format!("{SITUS}/api/umpan?bahasa={}", dua!("id", "en"))
}

/// Badan balasan, dibuka bila server memampatkannya dengan gzip (`content-encoding: gzip`, nama kepala tanpa
/// peduli huruf besar). Host yang sudah membukanya sendiri tapi tetap meneruskan kepalanya (URLSession) dikenali
/// dari bita ajaib gzip yang tidak ada: badannya dipakai apa adanya.
pub fn buka_gzip(headers: &[(String, String)], body: Vec<u8>) -> Result<Vec<u8>, String> {
    let gzip = headers.iter().any(|(k, v)| {
        k.eq_ignore_ascii_case("content-encoding")
            && v.split(',').any(|e| e.trim().eq_ignore_ascii_case("gzip") || e.trim().eq_ignore_ascii_case("x-gzip"))
    });
    if !gzip || !body.starts_with(&[0x1f, 0x8b]) {
        return Ok(body);
    }
    let mut terbuka = Vec::with_capacity(body.len() * 5);
    flate2::read::MultiGzDecoder::new(body.as_slice())
        .take(BATAS_TERBUKA + 1)
        .read_to_end(&mut terbuka)
        .map_err(|e| format!("gzip: {e}"))?;
    if terbuka.len() as u64 > BATAS_TERBUKA {
        return Err(dua!("umpan terlalu besar setelah dibuka", "feed too large after decompression").into());
    }
    Ok(terbuka)
}

/// Tulis salinan umpan; `None` bila badannya bukan UTF-8 (tidak mungkin untuk JSON yang lolos [`olah_umpan`]).
pub fn minta_simpan(body: &[u8]) -> Option<Request> {
    let teks = std::str::from_utf8(body).ok()?;
    Some(exact_data::storage::request("fs.atomicWriteFile", json!({ "path": BERKAS_UMPAN, "text": teks })))
}

pub fn minta_baca_simpanan() -> Request {
    exact_data::storage::request("fs.readFile", json!({ "path": BERKAS_UMPAN }))
}

/// Umpan dari salinan di HP. Berkas yang belum ada, tak terbaca atau rusak = umpan kosong, bukan galat:
/// Beranda lalu menunggu `muatUmpan()` seperti sebelum ada salinan.
pub fn olah_simpanan(outcome: Outcome) -> Umpan {
    let isi = exact_data::storage::response(outcome)
        .ok()
        .and_then(|v| v.get("base64").and_then(Json::as_str).and_then(teks::base64));
    isi.and_then(|bita| olah_umpan(&bita).ok()).unwrap_or_default()
}

/// Keterangan media yang menandai pelapor anonim (server mengganti nama pelapor dengan "anonim").
fn anonim(kredit: &str) -> bool {
    let k = kredit.trim();
    k.eq_ignore_ascii_case("anonim") || k.eq_ignore_ascii_case("anonymous")
}

/// Video diputar MediaPlayer Android langsung dari produksi: tumpukan jaringan Android menolak HTTP polos
/// (mock-server lewat `adb reverse`), dan mock hanya meneruskan berkas yang sama.
pub const MEDIA: &str = "https://fire.nusantara.earth";

/// Gambar yang diperkecil server Next.js. Tanpa `image/webp` di Accept ia membalas JPEG, yang bisa
/// didekode Exact (JPEG dan PNG saja; WebP tidak).
fn gambar(path: &str, lebar: u32) -> String {
    if path.is_empty() {
        return String::new();
    }
    format!("{SITUS}/_next/image?url={}&w={lebar}&q=75", teks::enkode_uri(path))
}

fn teks_atau<'a>(j: &'a Json, kunci: &str, cadangan: &'a str) -> &'a str {
    j.get(kunci).and_then(Json::as_str).filter(|s| !s.trim().is_empty()).unwrap_or(cadangan)
}

fn laporan(j: &Json) -> Option<Laporan> {
    let id = j.get("id")?.as_i64()?;
    let judul = j.get("judul")?.as_str()?;
    let lat = j.get("lat")?.as_f64()?;
    let lng = j.get("lng")?.as_f64()?;
    if !lat.is_finite() || !lng.is_finite() || (lat == 0.0 && lng == 0.0) {
        return None;
    }
    let media = j.get("media")?.as_array()?;
    let pertama = media.first();
    // Sampul: media pertama, atau posternya bila video.
    let sampul = pertama
        .map(|m| match m.get("jenis").and_then(Json::as_str) {
            Some("video") => teks_atau(m, "poster", ""),
            _ => teks_atau(m, "url", ""),
        })
        .unwrap_or("");
    // Sorotan Beranda: poster video pertama bila ada (seperti KartuSorotan aslinya), supaya gambar yang menutupi
    // pemutar sama dengan bingkai pertama videonya, bukan foto lain yang lalu berganti.
    let sampul_sorotan = media
        .iter()
        .find(|m| m.get("jenis").and_then(Json::as_str) == Some("video"))
        .map(|m| teks_atau(m, "poster", ""))
        .filter(|p| !p.is_empty())
        .unwrap_or(sampul);
    let bawaan = dua!(KREDIT_ID, KREDIT_EN);
    let kredit = pertama.map(|m| teks_atau(m, "keterangan", bawaan)).unwrap_or(bawaan);
    let kredit = if teks::inggris() && anonim(kredit) { "Anonymous" } else { kredit };
    let pulau = teks_atau(j, "pulau", "");
    let tempat = teks_atau(j, "provinsi", if pulau.is_empty() { "Indonesia" } else { pulau });
    let tanggal = teks::tanggal_teks(teks_atau(j, "tanggal", ""));
    let lokasi = teks_atau(j, "lokasi", "");
    // Tempat singkat di meta kartu kisi: bagian pertama `lokasi` (desa), atau tempat bila tak ada.
    let desa = lokasi.split(',').next().map(str::trim).filter(|d| !d.is_empty()).unwrap_or(tempat);
    // Semua media untuk penggeser di Detail: gambarnya (poster bila video) dan, sejajar, alamat videonya
    // ("" untuk foto). Media tanpa gambar dilewati.
    let (galeri, galeri_video): (Vec<String>, Vec<String>) = media
        .iter()
        .filter_map(|m| {
            let video = m.get("jenis").and_then(Json::as_str) == Some("video");
            let gambar_m = if video { teks_atau(m, "poster", "") } else { teks_atau(m, "url", "") };
            if gambar_m.is_empty() {
                return None;
            }
            let alamat = if video { teks_atau(m, "url", "") } else { "" };
            let alamat = if alamat.is_empty() { String::new() } else { format!("{MEDIA}{alamat}") };
            Some((gambar(gambar_m, 1200), alamat))
        })
        .unzip();
    let (lintang, bujur) = teks::koordinat(lat, lng);
    Some(Laporan {
        id: id.to_string(),
        slug: teks_atau(j, "slug", "").to_string(),
        judul: judul.trim().to_string(),
        pulau: pulau.to_string(),
        tempat: tempat.to_string(),
        desa: desa.to_string(),
        tanggal_pendek: teks::tanggal_pendek(&tanggal),
        tanggal,
        lokasi: lokasi.to_string(),
        lintang,
        bujur,
        lat,
        lng,
        deskripsi: teks_atau(j, "deskripsi", "").trim().to_string(),
        sampul: gambar(sampul_sorotan, 828),
        sampul_kecil: gambar(sampul, 640),
        kredit: kredit.to_string(),
        inisial: teks::inisial(kredit),
        jumlah_kredit: 0.0,
        galeri,
        galeri_video,
        jumlah_media: media.len() as f64,
        ada_video: media.iter().any(|m| m.get("jenis").and_then(Json::as_str) == Some("video")),
        vertikal: j.get("vertikal").and_then(Json::as_bool).unwrap_or(false),
        suka: j.get("jumlahSuka").and_then(Json::as_f64).unwrap_or(0.0).max(0.0),
        komentar: j.get("jumlahKomentar").and_then(Json::as_f64).unwrap_or(0.0).max(0.0),
    })
}

/// Teks JSON umpan → laporan yang sah (urutan server: terbaru dulu) dan pulau menurut jumlah laporannya.
pub fn olah_umpan(body: &[u8]) -> Result<Umpan, String> {
    let data: Json = serde_json::from_slice(body).map_err(|e| dua!(format!("JSON umpan: {e}"), format!("Feed JSON: {e}")))?;
    let butir = data.as_array().ok_or(dua!("umpan bukan larik", "feed is not an array"))?;
    let mut laporan: Vec<Laporan> = butir.iter().filter_map(laporan).collect();
    // "{n} laporan terverifikasi" di Detail: laporan lain dengan nama pengirim (kredit) yang sama.
    let mut per_kredit: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for l in &laporan {
        *per_kredit.entry(l.kredit.clone()).or_default() += 1.0;
    }
    for l in &mut laporan {
        l.jumlah_kredit = per_kredit[&l.kredit];
    }
    let pulau = hitung_per(&laporan, |l| &l.pulau, usize::MAX);
    // "Sedang ramai" di Cari: delapan provinsi dengan laporan terbanyak.
    let provinsi = hitung_per(&laporan, |l| if l.tempat == l.pulau || l.tempat == "Indonesia" { "" } else { &l.tempat }, 8);
    // "Sumber Terverifikasi": sepuluh pengirim terbanyak, tanpa nama bawaan dan tanpa pelapor anonim.
    let bukan_sumber = |k: &str| k == KREDIT_ID || k == KREDIT_EN || anonim(k);
    let sumber = hitung_per(&laporan, |l| if bukan_sumber(&l.kredit) { "" } else { &l.kredit }, 10)
        .into_iter()
        .map(|p| Sumber { inisial: teks::inisial(&p.nama), nama: p.nama, jumlah: p.jumlah })
        .collect();
    Ok(Umpan { laporan, pulau, provinsi, sumber })
}

/// Jumlah laporan per nilai `kunci` (yang kosong dilewati), menurun; seri tetap pada urutan kemunculan pertama.
fn hitung_per(laporan: &[Laporan], kunci: impl Fn(&Laporan) -> &str, batas: usize) -> Vec<Pulau> {
    let mut hasil: Vec<Pulau> = Vec::new();
    for nama in laporan.iter().map(&kunci).filter(|k| !k.is_empty()) {
        match hasil.iter_mut().find(|p| p.nama == nama) {
            Some(p) => p.jumlah += 1.0,
            None => hasil.push(Pulau { nama: nama.to_string(), jumlah: 1.0 }),
        }
    }
    hasil.sort_by(|a, b| b.jumlah.total_cmp(&a.jumlah));
    hasil.truncate(batas);
    hasil
}

/// Teks JSON komentar → akar komentar (terbaru dulu), dengan jumlah balasan masing-masing.
pub fn olah_komentar(body: &[u8]) -> Result<Vec<Komentar>, String> {
    let data: Json =
        serde_json::from_slice(body).map_err(|e| dua!(format!("JSON komentar: {e}"), format!("Comments JSON: {e}")))?;
    let butir = data.get("komentar").and_then(Json::as_array).ok_or(dua!("komentar bukan larik", "comments is not an array"))?;
    let bawaan = dua!("Warga", "Citizen");
    Ok(butir
        .iter()
        .filter_map(|k| {
            let nama = teks_atau(k, "nama", bawaan);
            Some(Komentar {
                id: k.get("id")?.as_i64()?.to_string(),
                nama: nama.to_string(),
                inisial: teks::inisial(nama),
                isi: k.get("isi")?.as_str()?.trim().to_string(),
                waktu: teks::waktu_relatif(teks_atau(k, "waktu", "")),
                balasan: k.get("balasan").and_then(Json::as_array).map_or(0.0, |b| b.len() as f64),
            })
        })
        .collect())
}

#[cfg(test)]
mod uji {
    use super::*;

    const CONTOH: &str = r#"[
      {"id":173,"slug":"kebakaran-besar","pulau":"Kalimantan","provinsi":"Kalimantan Tengah",
       "tanggal":"4 November 2026","judul":" Kebakaran Besar ","lokasi":"Handiwung, Katingan","lat":-2.00665,
       "lng":113.50224,"deskripsi":"Asap tebal.","vertikal":false,"jumlahKomentar":2,"jumlahSuka":3,
       "media":[{"jenis":"video","url":"/media/video/a.mov","poster":"/media/poster/a.jpg","keterangan":"Muhamad"},
                {"jenis":"gambar","url":"/media/gambar/b.jpg"}]},
      {"id":12,"judul":"Tanpa koordinat","lat":0,"lng":0,"media":[]},
      {"id":"x","judul":"Id bukan angka","lat":1,"lng":1,"media":[]},
      {"id":5,"pulau":"Sumatra","provinsi":null,"tanggal":"1 November 2026","judul":"Asap di Riau","lat":0.5,
       "lng":101.4,"media":[]},
      {"id":6,"pulau":"Kalimantan","tanggal":"1 November 2026","judul":"Titik api","lat":-1,"lng":114,"media":[]}
    ]"#;

    #[test]
    fn umpan_mengikuti_aturan_klien() {
        let u = olah_umpan(CONTOH.as_bytes()).unwrap();
        let id: Vec<&str> = u.laporan.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(id, ["173", "5", "6"]);
        let l = &u.laporan[0];
        assert_eq!(l.judul, "Kebakaran Besar");
        assert_eq!(l.tempat, "Kalimantan Tengah");
        assert_eq!(l.kredit, "Muhamad");
        assert_eq!(l.inisial, "MU");
        assert!(l.ada_video);
        assert_eq!(l.jumlah_media, 2.0);
        assert_eq!(l.sampul, format!("{SITUS}/_next/image?url=%2Fmedia%2Fposter%2Fa.jpg&w=828&q=75"));
        assert_eq!((l.lintang.as_str(), l.bujur.as_str()), ("2,01° LS", "113,50° BT"));
        assert_eq!(l.tanggal_pendek, "4 NOV");
        assert_eq!(l.desa, "Handiwung");
        assert_eq!(l.galeri.len(), 2);
        assert_eq!(l.galeri_video, [format!("{MEDIA}/media/video/a.mov"), String::new()]);
        assert_eq!(l.jumlah_kredit, 1.0);
        assert_eq!(u.laporan[1].desa, "Sumatra");
        // Tanpa provinsi: tempat = pulau; tanpa media: kredit bawaan, tanpa sampul.
        assert_eq!(u.laporan[1].tempat, "Sumatra");
        assert_eq!(u.laporan[1].kredit, "Laporan warga");
        assert_eq!(u.laporan[1].sampul, "");
        let pulau: Vec<(&str, f64)> = u.pulau.iter().map(|p| (p.nama.as_str(), p.jumlah)).collect();
        assert_eq!(pulau, [("Kalimantan", 2.0), ("Sumatra", 1.0)]);
        let prov: Vec<&str> = u.provinsi.iter().map(|p| p.nama.as_str()).collect();
        assert_eq!(prov, ["Kalimantan Tengah"]);
        let sumber: Vec<(&str, &str)> = u.sumber.iter().map(|s| (s.nama.as_str(), s.inisial.as_str())).collect();
        assert_eq!(sumber, [("Muhamad", "MU")]);
    }

    #[test]
    fn umpan_bahasa_inggris() {
        teks::atur_bahasa("EN");
        assert_eq!(alamat_umpan(), format!("{SITUS}/api/umpan?bahasa=en"));
        let u = olah_umpan(CONTOH.as_bytes()).unwrap();
        let l = &u.laporan[0];
        assert_eq!((l.lintang.as_str(), l.bujur.as_str()), ("2.01° S", "113.50° E"));
        assert_eq!((l.tanggal.as_str(), l.tanggal_pendek.as_str()), ("4 November 2026", "4 NOV"));
        assert_eq!(u.laporan[1].kredit, "Citizen report");
        // Umpan `?bahasa=en` memberi tanggal en-US; tampilannya tetap "4 August 2026" / "4 AUG".
        let en = olah_umpan(
            br#"[{"id":1,"judul":"Fire","tanggal":"August 4, 2026","lat":1,"lng":1,
                 "media":[{"jenis":"gambar","url":"/a.jpg","keterangan":"anonim"}]}]"#,
        )
        .unwrap();
        let l = &en.laporan[0];
        assert_eq!((l.tanggal.as_str(), l.tanggal_pendek.as_str()), ("4 August 2026", "4 AUG"));
        assert_eq!(l.kredit, "Anonymous");
        assert!(en.sumber.is_empty(), "pelapor anonim bukan sumber");
        let k = olah_komentar(br#"{"komentar":[{"id":1,"isi":"Hi","waktu":"3 hari yang lalu"}]}"#).unwrap();
        assert_eq!((k[0].nama.as_str(), k[0].waktu.as_str()), ("Citizen", "3 days ago"));
        teks::atur_bahasa("ID");
        assert_eq!(alamat_umpan(), format!("{SITUS}/api/umpan?bahasa=id"));
        let k = olah_komentar(br#"{"komentar":[{"id":1,"isi":"Hai","waktu":"3 hari yang lalu"}]}"#).unwrap();
        assert_eq!((k[0].nama.as_str(), k[0].waktu.as_str()), ("Warga", "3 hari yang lalu"));
    }

    #[test]
    fn gzip_dibuka_bila_kepalanya_gzip() {
        use std::io::Write;
        let mut z = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(CONTOH.as_bytes()).unwrap();
        let mampat = z.finish().unwrap();
        let kepala = |v: &str| vec![("Content-Type".to_string(), "application/json".to_string()), ("Content-Encoding".to_string(), v.to_string())];
        let terbuka = buka_gzip(&kepala("gzip"), mampat.clone()).unwrap();
        assert_eq!(terbuka, CONTOH.as_bytes());
        assert_eq!(olah_umpan(&terbuka).unwrap().laporan.len(), 3);
        // Nama kepala tanpa peduli huruf besar.
        assert_eq!(buka_gzip(&[("content-encoding".into(), "GZIP".into())], mampat.clone()).unwrap(), CONTOH.as_bytes());
        // Tanpa kepala: badan apa adanya, juga bila badannya gzip.
        assert_eq!(buka_gzip(&[], mampat.clone()).unwrap(), mampat);
        // Host yang sudah membuka badannya tapi tetap meneruskan kepalanya.
        assert_eq!(buka_gzip(&kepala("gzip"), CONTOH.as_bytes().to_vec()).unwrap(), CONTOH.as_bytes());
        // gzip rusak = galat, bukan umpan setengah.
        assert!(buka_gzip(&kepala("gzip"), mampat[..mampat.len() / 2].to_vec()).is_err());
    }

    #[test]
    fn salinan_umpan() {
        let minta = minta_simpan(CONTOH.as_bytes()).unwrap();
        let isi: Json = serde_json::from_slice(minta.storage.as_ref().unwrap()).unwrap();
        assert_eq!(isi["op"], "fs.atomicWriteFile");
        assert_eq!(isi["args"]["path"], BERKAS_UMPAN);
        assert_eq!(isi["args"]["text"], CONTOH);
        let baca: Json = serde_json::from_slice(minta_baca_simpanan().storage.as_ref().unwrap()).unwrap();
        assert_eq!((baca["op"].as_str(), baca["args"]["path"].as_str()), (Some("fs.readFile"), Some(BERKAS_UMPAN)));
        let berkas = |v: Json| Outcome::Storage(serde_json::to_vec(&v).unwrap());
        // base64 dari CONTOH, seperti `fs.readFile` host.
        let b64 = exact_runner::agent::base64(CONTOH.as_bytes());
        assert_eq!(olah_simpanan(berkas(json!({ "base64": b64 }))).laporan.len(), 3);
        // Belum ada, rusak, bukan JSON umpan: umpan kosong.
        assert!(olah_simpanan(berkas(json!({ "error": "ENOENT: no such file" }))).laporan.is_empty());
        assert!(olah_simpanan(berkas(json!({ "base64": exact_runner::agent::base64(b"{rusak") }))).laporan.is_empty());
        assert!(olah_simpanan(Outcome::Failed { kind: exact_runner::FailureKind::Unsupported, message: "x".into() }).laporan.is_empty());
    }

    #[test]
    fn komentar_dengan_balasan() {
        let k = olah_komentar(
            br#"{"komentar":[{"id":15,"nama":"Dewi Hartati","isi":"Berkabut asap!","waktu":"24 hari yang lalu",
                "sebutan":null,"balasan":[{"id":16,"nama":"Budi","isi":"Iya","waktu":"1 hari yang lalu"}]}]}"#,
        )
        .unwrap();
        assert_eq!(k.len(), 1);
        assert_eq!((k[0].id.as_str(), k[0].inisial.as_str(), k[0].balasan), ("15", "DH", 1.0));
    }
}
