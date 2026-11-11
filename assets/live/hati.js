// Hati melayang di atas video siaran (FloatingHearts interaksi-live.tsx laporin-apps): naik 240 px dalam 1,5 detik
// sambil membesar lalu memudar, warna acak dari palet siaran, paling banyak 30 sekaligus. Dipakai siar.html dan
// tonton.html; lapisannya di bawah tombol Contract, jadi hati tidak pernah menghalangi ketukan.
'use strict';
(function () {
  const WARNA = ['#FF4A1C', '#EDE8DF', '#FF8A3D', '#FF4A1C', '#FFB59E'];
  const BENTUK =
    'M256 448a32 32 0 01-18-5.57c-78.59-53.35-112.62-89.93-131.39-112.8-40-48.75-59.15-98.8-58.61-153C48.63 114.52 ' +
    '98.46 64 159.08 64c44.08 0 74.61 24.83 92.39 45.51a6 6 0 009.06 0C278.31 88.81 308.84 64 352.92 64c60.62 0 110.45 ' +
    '50.52 111.08 112.64.54 54.21-18.63 104.26-58.61 153-18.77 22.87-52.8 59.45-131.39 112.8a32 32 0 01-18 5.56z';
  const lapis = document.createElement('div');
  lapis.style.cssText = 'position:fixed;inset:0;pointer-events:none;overflow:hidden';
  document.addEventListener('DOMContentLoaded', () => document.body.appendChild(lapis));
  let hidup = 0;

  function hati(x, y) {
    if (hidup >= 30) return;
    hidup += 1;
    const warna = WARNA[Math.floor(Math.random() * WARNA.length)];
    const geser = (Math.random() * 2 - 1) * 35;
    const putar = (Math.random() * 2 - 1) * 15;
    const el = document.createElement('div');
    el.style.cssText = `position:absolute;left:${x - 22}px;top:${y - 22}px;width:44px;height:44px`;
    el.innerHTML = `<svg viewBox="0 0 512 512" width="44" height="44"><path fill="${warna}" d="${BENTUK}"/></svg>`;
    (lapis.isConnected ? lapis : document.body).appendChild(el);
    const t = (dx, dy, s) => `translate(${dx}px, ${dy}px) rotate(${putar}deg) scale(${s})`;
    el.animate(
      [
        { offset: 0, opacity: 0, transform: t(0, 0, 0.3) },
        { offset: 0.08, opacity: 1, transform: t(geser * 0.15, -40, 0.9) },
        { offset: 0.15, opacity: 1, transform: t(geser * 0.3, -70, 1.25) },
        { offset: 0.5, opacity: 1, transform: t(geser, -170, 1.05) },
        { offset: 0.75, opacity: 1, transform: t(geser * 0.25, -215, 0.95) },
        { offset: 1, opacity: 0, transform: t(-geser / 2, -240, 0.9) },
      ],
      { duration: 1500, easing: 'cubic-bezier(0.25, 0.46, 0.45, 0.94)', fill: 'forwards' },
    ).onfinish = () => {
      el.remove();
      hidup -= 1;
    };
  }

  // Hati sendiri muncul di atas tombol hati; hati dari orang lain sedikit di kirinya, paling banyak 3 tiap 300 ms.
  window.hatiSendiri = () => hati(window.innerWidth - 44, window.innerHeight * 0.6);
  let antre = 0;
  let jalan = 0;
  window.hatiTamu = (n) => {
    antre = Math.min(antre + n, 30);
    if (jalan) return;
    const lepas = () => {
      const k = Math.min(3, antre);
      for (let i = 0; i < k; i += 1) {
        hati(window.innerWidth - 56 + (Math.random() * 2 - 1) * 15, window.innerHeight * 0.62 + i * 30);
      }
      antre -= k;
      jalan = antre > 0 ? setTimeout(lepas, 300) : 0;
    };
    lepas();
  };
})();
