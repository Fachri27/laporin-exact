//! Format teks bahasa Indonesia yang dipakai tampilan: tanggal, jam WIB, koordinat, inisial.
//! Tanpa jam atau zona sistem: waktu selalu datang sebagai argumen (epoch ms + selisih menit).

const BULAN: [&str; 12] = [
    "Januari", "Februari", "Maret", "April", "Mei", "Juni", "Juli", "Agustus", "September", "Oktober",
    "November", "Desember",
];

/// Hari sejak 1970-01-01 → (tahun, bulan 1–12, tanggal). Algoritme civil_from_days (Howard Hinnant).
fn sipil(hari: i64) -> (i64, u32, u32) {
    let z = hari + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// (tahun, bulan, tanggal) → hari sejak 1970-01-01 (days_from_civil).
fn hari_dari(t: i64, b: u32, d: u32) -> i64 {
    let t = if b <= 2 { t - 1 } else { t };
    let era = t.div_euclid(400);
    let yoe = t.rem_euclid(400);
    let mp = (i64::from(b) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// "9 Nov 2026, 23.40" dari epoch ms dan selisih menit ke timur UTC.
pub fn waktu_singkat(epoch_ms: f64, selisih_menit: f64) -> String {
    let menit = (epoch_ms / 60_000.0).floor() as i64 + selisih_menit.round() as i64;
    let (t, b, d) = sipil(menit.div_euclid(1440));
    let dalam_hari = menit.rem_euclid(1440);
    format!("{d} {} {t}, {:02}.{:02}", &BULAN[b as usize - 1][..3], dalam_hari / 60, dalam_hari % 60)
}

/// "2026-11-09" → "9 November 2026"; teks lain dikembalikan apa adanya.
pub fn tanggal_panjang(iso: &str) -> String {
    match pecah_iso_tanggal(iso) {
        Some((t, b, d)) => format!("{d} {} {t}", BULAN[b as usize - 1]),
        None => iso.to_string(),
    }
}

fn pecah_iso_tanggal(iso: &str) -> Option<(i64, u32, u32)> {
    let mut bagian = iso.get(..10)?.split('-');
    let t = bagian.next()?.parse().ok()?;
    let b: u32 = bagian.next()?.parse().ok()?;
    let d: u32 = bagian.next()?.parse().ok()?;
    ((1..=12).contains(&b) && (1..=31).contains(&d)).then_some((t, b, d))
}

/// "2026-11-09T14:14:00.000Z" → "21.14 WIB" (UTC+7).
pub fn jam_wib(iso: &str) -> String {
    let jam = || -> Option<String> {
        let (t, b, d) = pecah_iso_tanggal(iso)?;
        let j: i64 = iso.get(11..13)?.parse().ok()?;
        let m: i64 = iso.get(14..16)?.parse().ok()?;
        let menit = hari_dari(t, b, d) * 1440 + j * 60 + m + 7 * 60;
        let dalam_hari = menit.rem_euclid(1440);
        Some(format!("{:02}.{:02} WIB", dalam_hari / 60, dalam_hari % 60))
    };
    jam().unwrap_or_default()
}

/// "2026-11-09T17:51:07.191Z" → "10 Nov 2026, 00.51" (WIB, UTC+7); teks lain dikembalikan apa adanya.
pub fn waktu_iso_wib(iso: &str) -> String {
    let waktu = || -> Option<String> {
        let (t, b, d) = pecah_iso_tanggal(iso)?;
        let j: i64 = iso.get(11..13)?.parse().ok()?;
        let m: i64 = iso.get(14..16)?.parse().ok()?;
        let menit = hari_dari(t, b, d) * 1440 + j * 60 + m;
        Some(waktu_singkat((menit * 60_000) as f64, 420.0))
    };
    waktu().unwrap_or_else(|| iso.to_string())
}

/// "4 November 2026" → "4 NOV".
pub fn tanggal_pendek(tanggal: &str) -> String {
    let mut kata = tanggal.split_whitespace();
    match (kata.next(), kata.next()) {
        (Some(d), Some(b)) => format!("{d} {}", b.chars().take(3).collect::<String>().to_uppercase()),
        _ => tanggal.to_uppercase(),
    }
}

/// Lintang dan bujur gaya Laporin: "2,01° LS" dan "113,50° BT".
pub fn koordinat(lat: f64, lng: f64) -> (String, String) {
    let angka = |v: f64| format!("{:.2}", v.abs()).replace('.', ",");
    (
        format!("{}° {}", angka(lat), if lat < 0.0 { "LS" } else { "LU" }),
        format!("{}° {}", angka(lng), if lng < 0.0 { "BB" } else { "BT" }),
    )
}

/// Inisial dua huruf dari nama: "Dewi Hartati" → "DH", "Muhamad" → "MU".
pub fn inisial(nama: &str) -> String {
    let kata: Vec<&str> = nama.split_whitespace().filter(|k| k.chars().any(char::is_alphanumeric)).collect();
    let hasil: String = match kata.as_slice() {
        [] => "?".into(),
        [satu] => satu.chars().filter(|c| c.is_alphanumeric()).take(2).collect(),
        [a, .., b] => [a, b].iter().filter_map(|k| k.chars().find(|c| c.is_alphanumeric())).collect(),
    };
    hasil.to_uppercase()
}

/// encodeURIComponent untuk path media di query `_next/image`.
pub fn enkode_uri(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Dekode base64 standar (balasan `fs.readFile` host Exact).
pub fn base64(teks: &str) -> Option<Vec<u8>> {
    let nilai = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    let bersih: Vec<u8> = teks.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=').collect();
    let mut out = Vec::with_capacity(bersih.len() * 3 / 4);
    for potong in bersih.chunks(4) {
        let mut acc = 0u32;
        for (i, &c) in potong.iter().enumerate() {
            acc |= nilai(c)? << (18 - 6 * i);
        }
        let bita = acc.to_be_bytes();
        out.extend_from_slice(&bita[1..potong.len()]);
    }
    Some(out)
}

#[cfg(test)]
mod uji {
    use super::*;

    #[test]
    fn waktu_dan_tanggal() {
        // 2026-11-09T16:40:00Z = 23.40 WIB.
        assert_eq!(waktu_singkat(1_794_242_400_000.0, 420.0), "9 Nov 2026, 23.40");
        assert_eq!(tanggal_panjang("2026-11-09"), "9 November 2026");
        assert_eq!(jam_wib("2026-11-09T14:14:00.000Z"), "21.14 WIB");
        assert_eq!(jam_wib("2026-11-09T20:05:00.000Z"), "03.05 WIB");
        assert_eq!(tanggal_pendek("4 November 2026"), "4 NOV");
        assert_eq!(waktu_iso_wib("2026-11-09T17:51:07.191Z"), "10 Nov 2026, 00.51");
    }

    #[test]
    fn koordinat_dan_inisial() {
        assert_eq!(koordinat(-2.00665, 113.50224), ("2,01° LS".into(), "113,50° BT".into()));
        assert_eq!(inisial("Dewi Hartati"), "DH");
        assert_eq!(inisial("Muhamad"), "MU");
        assert_eq!(inisial(""), "?");
    }

    #[test]
    fn base64_dan_uri() {
        assert_eq!(base64("W3siYSI6MX1d").unwrap(), b"[{\"a\":1}]");
        assert_eq!(base64("aGk=").unwrap(), b"hi");
        assert_eq!(enkode_uri("/media/gambar/a b.jpg"), "%2Fmedia%2Fgambar%2Fa%20b.jpg");
    }
}
