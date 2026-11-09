//! Umpan laporan terverifikasi (`GET {SITUS}/api/umpan?bahasa=id`) dan komentarnya
//! (`GET {SITUS}/api/laporan/{id}/komentar`), dengan aturan klien laporin-apps: lewati laporan tanpa
//! id angka, judul, koordinat sah (bukan 0,0) atau larik media.

use serde_json::Value as Json;

use crate::shapes::{Komentar, Laporan, Pulau, Umpan};
use crate::teks;

pub const SITUS: &str = "https://fire.nusantara.earth";

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
    let kredit = pertama.map(|m| teks_atau(m, "keterangan", "Laporan warga")).unwrap_or("Laporan warga");
    let pulau = teks_atau(j, "pulau", "");
    let tempat = teks_atau(j, "provinsi", if pulau.is_empty() { "Indonesia" } else { pulau });
    let tanggal = teks_atau(j, "tanggal", "");
    let (lintang, bujur) = teks::koordinat(lat, lng);
    Some(Laporan {
        id: id.to_string(),
        slug: teks_atau(j, "slug", "").to_string(),
        judul: judul.trim().to_string(),
        pulau: pulau.to_string(),
        tempat: tempat.to_string(),
        tanggal: tanggal.to_string(),
        tanggal_pendek: teks::tanggal_pendek(tanggal),
        lokasi: teks_atau(j, "lokasi", "").to_string(),
        lintang,
        bujur,
        deskripsi: teks_atau(j, "deskripsi", "").trim().to_string(),
        sampul: gambar(sampul, 828),
        sampul_kecil: gambar(sampul, 384),
        kredit: kredit.to_string(),
        inisial: teks::inisial(kredit),
        jumlah_media: media.len() as f64,
        ada_video: media.iter().any(|m| m.get("jenis").and_then(Json::as_str) == Some("video")),
        vertikal: j.get("vertikal").and_then(Json::as_bool).unwrap_or(false),
        suka: j.get("jumlahSuka").and_then(Json::as_f64).unwrap_or(0.0).max(0.0),
        komentar: j.get("jumlahKomentar").and_then(Json::as_f64).unwrap_or(0.0).max(0.0),
    })
}

/// Teks JSON umpan → laporan yang sah (urutan server: terbaru dulu) dan pulau menurut jumlah laporannya.
pub fn olah_umpan(body: &[u8]) -> Result<Umpan, String> {
    let data: Json = serde_json::from_slice(body).map_err(|e| format!("JSON umpan: {e}"))?;
    let butir = data.as_array().ok_or("umpan bukan larik")?;
    let laporan: Vec<Laporan> = butir.iter().filter_map(laporan).collect();
    let mut pulau: Vec<Pulau> = Vec::new();
    for l in laporan.iter().filter(|l| !l.pulau.is_empty()) {
        match pulau.iter_mut().find(|p| p.nama == l.pulau) {
            Some(p) => p.jumlah += 1.0,
            None => pulau.push(Pulau { nama: l.pulau.clone(), jumlah: 1.0 }),
        }
    }
    // Stabil: pulau dengan jumlah sama tetap pada urutan kemunculan pertama.
    pulau.sort_by(|a, b| b.jumlah.total_cmp(&a.jumlah));
    Ok(Umpan { laporan, pulau })
}

/// Teks JSON komentar → akar komentar (terbaru dulu), dengan jumlah balasan masing-masing.
pub fn olah_komentar(body: &[u8]) -> Result<Vec<Komentar>, String> {
    let data: Json = serde_json::from_slice(body).map_err(|e| format!("JSON komentar: {e}"))?;
    let butir = data.get("komentar").and_then(Json::as_array).ok_or("komentar bukan larik")?;
    Ok(butir
        .iter()
        .filter_map(|k| {
            let nama = teks_atau(k, "nama", "Warga");
            Some(Komentar {
                id: k.get("id")?.as_i64()?.to_string(),
                nama: nama.to_string(),
                inisial: teks::inisial(nama),
                isi: k.get("isi")?.as_str()?.trim().to_string(),
                waktu: teks_atau(k, "waktu", "").to_string(),
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
        // Tanpa provinsi: tempat = pulau; tanpa media: kredit bawaan, tanpa sampul.
        assert_eq!(u.laporan[1].tempat, "Sumatra");
        assert_eq!(u.laporan[1].kredit, "Laporan warga");
        assert_eq!(u.laporan[1].sampul, "");
        let pulau: Vec<(&str, f64)> = u.pulau.iter().map(|p| (p.nama.as_str(), p.jumlah)).collect();
        assert_eq!(pulau, [("Kalimantan", 2.0), ("Sumatra", 1.0)]);
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
