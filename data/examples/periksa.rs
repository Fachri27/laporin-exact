//! Olah berkas JSON asli dengan inti yang sama dengan aplikasi:
//! `cargo run -p laporin-exact-data --example periksa -- umpan|komentar <berkas>`.

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (jenis, berkas) = (a.get(1).map(String::as_str).unwrap_or(""), a.get(2).expect("berkas JSON"));
    let isi = std::fs::read(berkas).expect("baca berkas");
    match jenis {
        "umpan" => {
            let u = laporin_data::umpan::olah_umpan(&isi).unwrap();
            println!("{} laporan sah; pulau: {:?}", u.laporan.len(), u.pulau.iter().map(|p| (&p.nama, p.jumlah)).collect::<Vec<_>>());
            for l in u.laporan.iter().take(3) {
                println!("- [{}] {} | {} | {} | {} {} | {}", l.id, l.judul, l.tempat, l.tanggal_pendek, l.lintang, l.bujur, l.sampul_kecil);
            }
        }
        "komentar" => {
            for k in laporin_data::umpan::olah_komentar(&isi).unwrap() {
                println!("- {} ({}) {}: {}", k.nama, k.inisial, k.waktu, k.isi);
            }
        }
        _ => eprintln!("jenis: umpan | komentar"),
    }
}
