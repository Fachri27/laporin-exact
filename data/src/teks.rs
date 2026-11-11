//! Format teks yang dipakai tampilan: tanggal, jam WIB, koordinat, inisial, dalam bahasa Indonesia atau Inggris.
//! Tanpa jam atau zona sistem: waktu selalu datang sebagai argumen (epoch ms + selisih menit).
//!
//! Bahasa dipilih Contract lewat `aturBahasa("ID" | "EN")` ([`atur_bahasa`]) dan berlaku global untuk semua teks
//! yang dibuat crate ini sesudahnya (tanggal, koordinat, label bawaan, galat buatan aplikasi). Bawaan: Indonesia.

#[cfg(not(test))]
use std::sync::atomic::{AtomicBool, Ordering};

/// Bahasa Inggris aktif. Satu nilai untuk seluruh proses: `answer` dan `parse` bisa berjalan di utas mana pun.
#[cfg(not(test))]
static INGGRIS: AtomicBool = AtomicBool::new(false);

// Uji berjalan paralel di banyak utas: setiap uji memegang bahasanya sendiri, jadi uji bahasa Inggris tidak
// mengubah keluaran uji lain yang mengharapkan bahasa Indonesia.
#[cfg(test)]
thread_local! {
    static INGGRIS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `aturBahasa(b)`: "EN" (huruf besar/kecil sama) = Inggris, selain itu Indonesia.
pub fn atur_bahasa(kode: &str) {
    let inggris = kode.trim().eq_ignore_ascii_case("EN");
    #[cfg(not(test))]
    INGGRIS.store(inggris, Ordering::Relaxed);
    #[cfg(test)]
    INGGRIS.with(|b| b.set(inggris));
}

/// Teks yang dibuat crate ini berbahasa Inggris.
pub fn inggris() -> bool {
    #[cfg(not(test))]
    return INGGRIS.load(Ordering::Relaxed);
    #[cfg(test)]
    return INGGRIS.with(std::cell::Cell::get);
}

/// `dua!(indonesia, inggris)`: teks menurut bahasa aktif. Hanya cabang yang terpilih yang dievaluasi, jadi
/// `format!` di kedua cabang tidak memformat dua kali.
macro_rules! dua {
    ($id:expr, $en:expr $(,)?) => {
        if $crate::teks::inggris() {
            $en
        } else {
            $id
        }
    };
}
pub(crate) use dua;

const BULAN: [&str; 12] = [
    "Januari", "Februari", "Maret", "April", "Mei", "Juni", "Juli", "Agustus", "September", "Oktober",
    "November", "Desember",
];

const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October",
    "November", "December",
];

/// Nama bulan 1–12 dalam bahasa aktif.
fn bulan(b: u32) -> &'static str {
    if inggris() { MONTHS[b as usize - 1] } else { BULAN[b as usize - 1] }
}

/// Jam:menit dalam bahasa aktif: "23.40" (Indonesia) atau "23:40".
fn jam_menit(menit_dalam_hari: i64) -> String {
    let (j, m) = (menit_dalam_hari / 60, menit_dalam_hari % 60);
    if inggris() { format!("{j:02}:{m:02}") } else { format!("{j:02}.{m:02}") }
}

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

/// "9 Nov 2026, 23.40" (Inggris "9 Nov 2026, 23:40") dari epoch ms dan selisih menit ke timur UTC.
pub fn waktu_singkat(epoch_ms: f64, selisih_menit: f64) -> String {
    let menit = (epoch_ms / 60_000.0).floor() as i64 + selisih_menit.round() as i64;
    let (t, b, d) = sipil(menit.div_euclid(1440));
    format!("{d} {} {t}, {}", &bulan(b)[..3], jam_menit(menit.rem_euclid(1440)))
}

/// "2026-11-09" → "9 November 2026" (nama bulan dalam bahasa aktif); teks lain dikembalikan apa adanya.
pub fn tanggal_panjang(iso: &str) -> String {
    match pecah_iso_tanggal(iso) {
        Some((t, b, d)) => format!("{d} {} {t}", bulan(b)),
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

/// "2026-11-09T14:14:00.000Z" → "21.14 WIB" (Inggris "21:14 WIB"; UTC+7).
pub fn jam_wib(iso: &str) -> String {
    let jam = || -> Option<String> {
        let (t, b, d) = pecah_iso_tanggal(iso)?;
        let j: i64 = iso.get(11..13)?.parse().ok()?;
        let m: i64 = iso.get(14..16)?.parse().ok()?;
        let menit = hari_dari(t, b, d) * 1440 + j * 60 + m + 7 * 60;
        Some(format!("{} WIB", jam_menit(menit.rem_euclid(1440))))
    };
    jam().unwrap_or_default()
}

/// "2026-11-09T17:51:07.191Z" → "10 Nov 2026, 00.51" (Inggris "10 Nov 2026, 00:51"; WIB, UTC+7); teks lain
/// dikembalikan apa adanya.
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

/// Nomor bulan dari namanya, lengkap atau tiga huruf, Indonesia atau Inggris ("Agustus", "Agt", "aug" → 8).
fn bulan_dari(kata: &str) -> Option<u32> {
    let k = kata.trim_end_matches('.').to_lowercase();
    let sama = |nama: &str| k == nama.to_lowercase() || (k.chars().count() == 3 && nama.to_lowercase().starts_with(&k));
    let i = (0..12).find(|&i| sama(BULAN[i]) || sama(MONTHS[i])).or(match k.as_str() {
        "agt" => Some(7),
        "sept" => Some(8),
        _ => None,
    })?;
    Some(i as u32 + 1)
}

/// Tanggal teks dari server: "4 November 2026" (`?bahasa=id`), "November 4, 2026" (`?bahasa=en`, en-US) atau
/// tanpa tahun → (tanggal, bulan, tahun bila ada).
fn pecah_tanggal_teks(teks: &str) -> Option<(u32, u32, Option<&str>)> {
    let kata: Vec<&str> = teks.split(|c: char| c.is_whitespace() || c == ',').filter(|k| !k.is_empty()).collect();
    let angka = |k: &str| k.bytes().all(|b| b.is_ascii_digit());
    let hari = |k: &str| k.parse::<u32>().ok().filter(|d| angka(k) && (1..=31).contains(d));
    let (d, b, t) = match kata.as_slice() {
        [d, b] if hari(d).is_some() => (hari(d)?, bulan_dari(b)?, None),
        [b, d] if hari(d).is_some() => (hari(d)?, bulan_dari(b)?, None),
        [d, b, t] if hari(d).is_some() => (hari(d)?, bulan_dari(b)?, Some(*t)),
        [b, d, t] => (hari(d)?, bulan_dari(b)?, Some(*t)),
        _ => return None,
    };
    if t.is_some_and(|t| t.len() != 4 || !angka(t)) {
        return None;
    }
    Some((d, b, t))
}

/// Tanggal teks server dalam bahasa aktif dan satu urutan: "4 November 2026" ↔ "4 November 2026" (bulan Inggris);
/// "November 4, 2026" (umpan `?bahasa=en`) juga menjadi "4 November 2026". Teks lain dikembalikan apa adanya.
pub fn tanggal_teks(teks: &str) -> String {
    match pecah_tanggal_teks(teks) {
        Some((d, b, Some(t))) => format!("{d} {} {t}", bulan(b)),
        Some((d, b, None)) => format!("{d} {}", bulan(b)),
        None => teks.to_string(),
    }
}

/// "4 November 2026" → "4 NOV"; "4 Agustus 2026" → "4 AGU" (Inggris "4 AUG"), dari tanggal teks mana pun yang
/// dikenal [`tanggal_teks`].
pub fn tanggal_pendek(tanggal: &str) -> String {
    if let Some((d, b, _)) = pecah_tanggal_teks(tanggal) {
        return format!("{d} {}", bulan(b)[..3].to_uppercase());
    }
    let mut kata = tanggal.split_whitespace();
    match (kata.next(), kata.next()) {
        (Some(d), Some(b)) => format!("{d} {}", b.chars().take(3).collect::<String>().to_uppercase()),
        _ => tanggal.to_uppercase(),
    }
}

/// Lintang dan bujur gaya Laporin: "2,01° LS" dan "113,50° BT" (Inggris "2.01° S" dan "113.50° E").
pub fn koordinat(lat: f64, lng: f64) -> (String, String) {
    if inggris() {
        let angka = |v: f64| format!("{:.2}", v.abs());
        return (
            format!("{}° {}", angka(lat), if lat < 0.0 { "S" } else { "N" }),
            format!("{}° {}", angka(lng), if lng < 0.0 { "W" } else { "E" }),
        );
    }
    let angka = |v: f64| format!("{:.2}", v.abs()).replace('.', ",");
    (
        format!("{}° {}", angka(lat), if lat < 0.0 { "LS" } else { "LU" }),
        format!("{}° {}", angka(lng), if lng < 0.0 { "BB" } else { "BT" }),
    )
}

/// Waktu relatif komentar dari server (`Intl.RelativeTimeFormat("id")`: "24 hari yang lalu", "kemarin") dalam
/// bahasa aktif: "24 days ago", "yesterday". Bentuk lain dikembalikan apa adanya.
pub fn waktu_relatif(teks: &str) -> String {
    if !inggris() {
        return teks.to_string();
    }
    let kecil = teks.trim().to_lowercase();
    let tetap = match kecil.as_str() {
        "sekarang" | "baru saja" => Some("now"),
        "kemarin" => Some("yesterday"),
        "kemarin dulu" => Some("2 days ago"),
        "hari ini" => Some("today"),
        "minggu lalu" => Some("last week"),
        "bulan lalu" => Some("last month"),
        "tahun lalu" => Some("last year"),
        _ => None,
    };
    if let Some(t) = tetap {
        return t.into();
    }
    let satuan = |s: &str| {
        Some(match s {
            "detik" => "second",
            "menit" => "minute",
            "jam" => "hour",
            "hari" => "day",
            "minggu" => "week",
            "bulan" => "month",
            "tahun" => "year",
            _ => return None,
        })
    };
    let kata: Vec<&str> = kecil.split_whitespace().collect();
    match kata.as_slice() {
        [n, s, "yang", "lalu"] | [n, s, "lalu"] if n.parse::<u64>().is_ok() => match satuan(s) {
            Some(u) => format!("{n} {u}{} ago", if *n == "1" { "" } else { "s" }),
            None => teks.to_string(),
        },
        _ => teks.to_string(),
    }
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
    fn tanggal_teks_server() {
        assert_eq!(tanggal_teks("4 November 2026"), "4 November 2026");
        assert_eq!(tanggal_teks("November 4, 2026"), "4 November 2026");
        assert_eq!(tanggal_teks("12 Agustus 2026"), "12 Agustus 2026");
        assert_eq!(tanggal_teks("bukan tanggal"), "bukan tanggal");
        assert_eq!(tanggal_pendek("4 Agustus 2026"), "4 AGU");
        assert_eq!(tanggal_pendek("August 4, 2026"), "4 AGU");
        assert_eq!(tanggal_pendek("4 Mei 2026"), "4 MEI");
        assert_eq!(waktu_relatif("24 hari yang lalu"), "24 hari yang lalu");
    }

    #[test]
    fn bahasa_inggris() {
        atur_bahasa("EN");
        assert!(inggris());
        assert_eq!(waktu_singkat(1_794_242_400_000.0, 420.0), "9 Nov 2026, 23:40");
        assert_eq!(tanggal_panjang("2026-11-09"), "9 November 2026");
        assert_eq!(tanggal_panjang("2026-08-17"), "17 August 2026");
        assert_eq!(jam_wib("2026-11-09T14:14:00.000Z"), "21:14 WIB");
        assert_eq!(waktu_iso_wib("2026-11-09T17:51:07.191Z"), "10 Nov 2026, 00:51");
        assert_eq!(waktu_singkat(1_779_000_000_000.0, 420.0), "17 May 2026, 13:40");
        assert_eq!(tanggal_teks("4 Agustus 2026"), "4 August 2026");
        assert_eq!(tanggal_teks("August 4, 2026"), "4 August 2026");
        assert_eq!(tanggal_pendek("4 November 2026"), "4 NOV");
        assert_eq!(tanggal_pendek("4 Agustus 2026"), "4 AUG");
        assert_eq!(tanggal_pendek("May 4, 2026"), "4 MAY");
        assert_eq!(tanggal_pendek("4 Desember 2026"), "4 DEC");
        assert_eq!(koordinat(-2.00665, 113.50224), ("2.01° S".into(), "113.50° E".into()));
        assert_eq!(koordinat(3.5, -70.25), ("3.50° N".into(), "70.25° W".into()));
        assert_eq!(waktu_relatif("24 hari yang lalu"), "24 days ago");
        assert_eq!(waktu_relatif("1 jam yang lalu"), "1 hour ago");
        assert_eq!(waktu_relatif("kemarin"), "yesterday");
        assert_eq!(waktu_relatif("bulan lalu"), "last month");
        assert_eq!(waktu_relatif("sesuatu lain"), "sesuatu lain");
        // Selain "EN" kembali ke Indonesia.
        atur_bahasa("ID");
        assert!(!inggris());
        assert_eq!(koordinat(-2.00665, 113.50224), ("2,01° LS".into(), "113,50° BT".into()));
        atur_bahasa("en");
        assert!(inggris());
        atur_bahasa("fr");
        assert_eq!(waktu_singkat(1_794_242_400_000.0, 420.0), "9 Nov 2026, 23.40");
    }

    #[test]
    fn base64_dan_uri() {
        assert_eq!(base64("W3siYSI6MX1d").unwrap(), b"[{\"a\":1}]");
        assert_eq!(base64("aGk=").unwrap(), b"hi");
        assert_eq!(enkode_uri("/media/gambar/a b.jpg"), "%2Fmedia%2Fgambar%2Fa%20b.jpg");
    }
}
