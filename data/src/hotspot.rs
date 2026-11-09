//! Olahan payload `GET {GEO}/api/hotspot` (24 jam terakhir), diadaptasi dari
//! laporin-apps-rust `modules/inti-rust/rust/src/hotspot.rs`: aturan baris sah (`barisSah`) yang
//! sama, ditambah ringkasan yang ditampilkan tab Peta (kepercayaan, satelit, peringkat).
//!
//! Baris `titik`: [lng, lat, kepercayaan 0–2, FRP, menit epoch, satelit 0–2, indeks provinsi, malam 0|1,
//! indeks kabupaten (−1 = tidak ada)].

use std::collections::HashMap;

use serde_json::Value as Json;

use crate::shapes::{Hotspot, Peringkat, Satelit};
use crate::teks;

pub const GEO: &str = "https://geoplatform.nusantara.earth";

/// Satelit menurut indeksnya: nama dan sensornya.
const SATELIT: [(&str, &str); 3] =
    [("NASA Terra/Aqua", "MODIS"), ("NASA Suomi NPP", "VIIRS"), ("NOAA-20", "VIIRS")];

/// Baris peringkat yang ditampilkan.
const PUNCAK: usize = 10;

struct Baris {
    kepercayaan: usize,
    satelit: usize,
    provinsi: f64,
    malam: bool,
    kabupaten: f64,
}

/// `Number.isInteger(x) && x >= min && x <= maks`.
fn bulat(v: &Json, min: f64, maks: f64) -> Option<f64> {
    let x = v.as_f64()?;
    (x.fract() == 0.0 && x >= min && x <= maks).then_some(x)
}

fn baris_sah(t: &Json) -> Option<Baris> {
    let t = t.as_array()?;
    if t.len() < 9 || t[0].as_f64()?.abs() > 180.0 || t[1].as_f64()?.abs() > 90.0 {
        return None;
    }
    t[3].as_f64()?;
    t[4].as_f64()?;
    Some(Baris {
        kepercayaan: bulat(&t[2], 0.0, 2.0)? as usize,
        satelit: bulat(&t[5], 0.0, 2.0)? as usize,
        provinsi: bulat(&t[6], 0.0, f64::INFINITY)?,
        malam: bulat(&t[7], 0.0, 1.0)? == 1.0,
        kabupaten: bulat(&t[8], -1.0, f64::INFINITY)?,
    })
}

fn daftar_nama(v: Option<&Json>) -> Vec<&str> {
    v.and_then(Json::as_array).map_or_else(Vec::new, |a| a.iter().map(|p| p.as_str().unwrap_or("-")).collect())
}

/// Indeks di luar daftar (atau −1) tidak dihitung, seperti `data.provinsi[t[6]]` yang undefined di JS.
fn nama_di<'a>(nama: &[&'a str], indeks: f64) -> Option<&'a str> {
    (indeks >= 0.0).then(|| nama.get(indeks as usize).copied()).flatten()
}

#[derive(Default)]
struct Kumpulan {
    jumlah: HashMap<String, (u32, u32)>,
}

impl Kumpulan {
    fn tambah(&mut self, nama: Option<&str>, tinggi: bool) {
        if let Some(nama) = nama {
            let e = self.jumlah.entry(nama.to_owned()).or_default();
            e.0 += 1;
            e.1 += u32::from(tinggi);
        }
    }

    /// Peringkat menurun menurut jumlah (seri: nama menaik), dipotong ke `PUNCAK`; `persen` relatif
    /// terhadap yang teratas, untuk lebar batang.
    fn peringkat(self) -> Vec<Peringkat> {
        let mut baris: Vec<(String, (u32, u32))> = self.jumlah.into_iter().collect();
        baris.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then_with(|| a.0.cmp(&b.0)));
        let teratas = baris.first().map_or(1, |b| b.1 .0.max(1)) as f64;
        baris
            .into_iter()
            .take(PUNCAK)
            .map(|(nama, (jumlah, tinggi))| Peringkat {
                nama,
                jumlah: jumlah as f64,
                persen: (jumlah as f64 / teratas * 100.0).round(),
                tinggi: tinggi as f64,
            })
            .collect()
    }
}

/// Teks JSON /api/hotspot → ringkasan tab Peta. Galat hanya bila bukan JSON atau `titik` bukan larik.
pub fn olah_hotspot(body: &[u8]) -> Result<Hotspot, String> {
    let data: Json = serde_json::from_slice(body).map_err(|e| format!("JSON hotspot: {e}"))?;
    let titik = data.get("titik").and_then(Json::as_array).ok_or("titik bukan larik")?;
    let provinsi = daftar_nama(data.get("provinsi"));
    let kabupaten = daftar_nama(data.get("kabupaten"));

    let mut per_satelit = [[0u32; 3]; 3];
    let mut malam = 0u32;
    let (mut per_provinsi, mut per_kabupaten) = (Kumpulan::default(), Kumpulan::default());
    for b in titik.iter().filter_map(baris_sah) {
        per_satelit[b.satelit][b.kepercayaan] += 1;
        malam += u32::from(b.malam);
        let tinggi = b.kepercayaan == 2;
        per_provinsi.tambah(nama_di(&provinsi, b.provinsi), tinggi);
        per_kabupaten.tambah(nama_di(&kabupaten, b.kabupaten), tinggi);
    }
    let menurut = |k: usize| per_satelit.iter().map(|s| s[k]).sum::<u32>() as f64;
    let teks_di = |kunci: &str| data.get(kunci).and_then(Json::as_str).unwrap_or("");
    Ok(Hotspot {
        hari: teks::tanggal_panjang(teks_di("hari")),
        diperbarui: teks::jam_wib(teks_di("diperbarui")),
        total: menurut(0) + menurut(1) + menurut(2),
        tinggi: menurut(2),
        sedang: menurut(1),
        rendah: menurut(0),
        malam: malam as f64,
        satelit: SATELIT
            .iter()
            .zip(per_satelit)
            .map(|(&(nama, sensor), [rendah, sedang, tinggi])| Satelit {
                nama: nama.into(),
                sensor: sensor.into(),
                jumlah: (rendah + sedang + tinggi) as f64,
                tinggi: tinggi as f64,
                sedang: sedang as f64,
                rendah: rendah as f64,
            })
            .collect(),
        provinsi: per_provinsi.peringkat(),
        kabupaten: per_kabupaten.peringkat(),
    })
}

#[cfg(test)]
mod uji {
    use super::*;

    #[test]
    fn baris_tidak_sah_dibuang_dan_peringkat_terurut() {
        let h = olah_hotspot(
            br#"{"hari":"2026-11-09","diperbarui":"2026-11-09T14:14:00.000Z",
                "provinsi":["Riau","Kalimantan Tengah"],"kabupaten":["Siak","Katingan"],"titik":[
                [101.5, 0.5, 2, 12.3, 29000000, 1, 0, 0, 0],
                [113.5, -2.0, 1, 3.1, 29000000, 0, 1, 1, 1],
                [113.6, -2.1, 2, 9.0, 29000000, 2, 1, 1, 1],
                [113.7, -2.2, 0, 1.0, 29000000, 2, 1, 0, -1],
                [181, 0.5, 2, 1, 1, 1, 0, 0, 0],
                [101, 0.5, 1.5, 1, 1, 1, 0, 0, 0],
                [101, 0.5, 2, "1", 1, 1, 0, 0, 0],
                [101, 0.5, 2, 1, 1, 1, 9, 0, 9]
            ]}"#,
        )
        .unwrap();
        assert_eq!((h.total, h.tinggi, h.sedang, h.rendah, h.malam), (5.0, 3.0, 1.0, 1.0, 2.0));
        assert_eq!((h.hari.as_str(), h.diperbarui.as_str()), ("9 November 2026", "21.14 WIB"));
        let prov: Vec<(&str, f64, f64)> = h.provinsi.iter().map(|p| (p.nama.as_str(), p.jumlah, p.persen)).collect();
        assert_eq!(prov, [("Kalimantan Tengah", 3.0, 100.0), ("Riau", 1.0, 33.0)]);
        let kab: Vec<(&str, f64)> = h.kabupaten.iter().map(|p| (p.nama.as_str(), p.jumlah)).collect();
        assert_eq!(kab, [("Katingan", 2.0), ("Siak", 1.0)]);
        assert_eq!(h.satelit[2].jumlah, 2.0);
    }

    #[test]
    fn bukan_json_atau_tanpa_titik_galat() {
        assert!(olah_hotspot(b"<html>").is_err());
        assert!(olah_hotspot(br#"{"titik":3}"#).is_err());
    }
}
