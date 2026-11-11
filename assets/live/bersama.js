// Bagian bersama siar.html dan tonton.html: peran dari identitas server (lib/live.ts laporin-apps), daftar penonton
// untuk lembar Penonton, label kualitas (lib/kualitas-live.ts), dan bahasa halaman (isi.bahasa "id" | "en", tak ada =
// "id"): nama pengganti peserta tanpa nama dan pemisah desimal laju mengikutinya; kata awal pesan dan urutan kolom
// tidak berubah.
'use strict';
window.Siaran = (function () {
  /** Nama pengganti per peran, pemisah desimal laju. "id" sama persis dengan sebelum ada terjemahan. */
  const TEKS = {
    id: { petugas: 'Petugas', pelapor: 'Pelapor', warga: 'Warga', desimal: ',' },
    en: { petugas: 'Officer', pelapor: 'Reporter', warga: 'Citizen', desimal: '.' },
  };
  let bahasa = 'id';

  /** Bahasa dari isi.bahasa (selain "en" = "id"); true bila berganti, supaya halaman mengirim ulang teksnya. */
  function aturBahasa(b) {
    const baru = b === 'en' ? 'en' : 'id';
    if (baru === bahasa) return false;
    bahasa = baru;
    document.documentElement.lang = baru;
    return true;
  }

  /** teks(kunci, isian) untuk kamus halaman { id: {…}, en: {…} } dalam bahasa kini; {nama} diisi dari `isian`. */
  function penerjemah(kamus) {
    return (kunci, isian) => {
      const t = kunci in kamus[bahasa] ? kamus[bahasa][kunci] : kamus.id[kunci];
      return isian ? t.replace(/\{(\w+)\}/g, (_, k) => String(isian[k])) : t;
    };
  }
  const teks = penerjemah(TEKS);

  /** Nama peserta tanpa nama: perannya ("Warga" / "Citizen", …). */
  const namaPeran = (peran) => teks(peran);

  function kirim(teks) {
    if (window.laporin) window.laporin.kirim(teks);
  }

  function peranDari(identity) {
    if (identity.startsWith('petugas-')) return 'petugas';
    if (identity.startsWith('pelapor-')) return 'pelapor';
    return 'warga';
  }

  const bersih = (t) => String(t).replace(/[\t\n]/g, ' ');

  function nama(ruang, identity) {
    return ruang.remoteParticipants.get(identity)?.name || namaPeran(peranDari(identity));
  }

  /** "komentar <peran>\t<nama>\t<isi>": nama akun tidak bisa menyamar sebagai petugas, perannya dari identitas. */
  function komentar(ruang, identity, teks) {
    kirim(`komentar ${peranDari(identity)}\t${bersih(nama(ruang, identity))}\t${teks.slice(0, 200).replace(/\s+/g, ' ')}`);
  }

  /**
   * "daftar <baris>…", baris "<peran>\t<nama>\t<1 bila diri sendiri>" dipisah baris baru: petugas dulu, lalu yang
   * paling baru bergabung. Pelapor tidak termasuk; `saya` (penonton) ikut di daftar.
   */
  function daftar(ruang, saya) {
    const orang = [...ruang.remoteParticipants.values()].filter((p) => peranDari(p.identity) !== 'pelapor');
    if (saya) orang.push(saya);
    orang.sort(
      (a, b) =>
        Number(peranDari(b.identity) === 'petugas') - Number(peranDari(a.identity) === 'petugas') ||
        (b.joinedAt?.getTime?.() || 0) - (a.joinedAt?.getTime?.() || 0),
    );
    const baris = orang.map((p) => {
      const peran = peranDari(p.identity);
      return `${peran}\t${bersih(p.name || namaPeran(peran))}\t${p === saya ? 1 : 0}`;
    });
    kirim(`daftar ${baris.join('\n')}`);
  }

  function fpsLabel(fps) {
    if (fps >= 45) return 60;
    if (fps >= 27) return 30;
    return Math.max(1, Math.round(fps));
  }

  function resolusi(lebar, tinggi, fps) {
    const p = Math.min(lebar, tinggi);
    return fps ? `${p}p${fpsLabel(fps)}` : `${p}p`;
  }

  /** "3,1 Mbps" / "3.1 Mbps", "450 kbps". */
  function bitrate(bps) {
    if (bps >= 1000000) return `${(bps / 1000000).toFixed(1).replace('.', teks('desimal'))} Mbps`;
    return `${Math.max(0, Math.round(bps / 1000))} kbps`;
  }

  return { kirim, peranDari, nama, komentar, daftar, resolusi, bitrate, aturBahasa, penerjemah, namaPeran };
})();
