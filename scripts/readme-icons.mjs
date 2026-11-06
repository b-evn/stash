// Redraws the README's small icon tiles (docs/icons/*.svg) from the glyphs in docs/readme-art.html, plus
// a few only the README uses. Each tile follows the reader's colour scheme on its own.
//   node scripts/readme-icons.mjs
import { readFileSync, writeFileSync } from 'node:fs';

const art = readFileSync(new URL('../docs/readme-art.html', import.meta.url), 'utf8');
const start = art.indexOf('const I = {');
const I = {};
for (const m of art.slice(start, art.indexOf('};', start)).matchAll(/^\s+(\w+): '(.*)',?$/gm)) I[m[1]] = m[2];
Object.assign(I, {
  eye: '<path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12Z"/><circle cx="12" cy="12" r="3"/>',
  bolt: '<path d="M13 3 5 13.5h6L10 21l8-10.5h-6Z"/>',
  camera: '<path d="M4 8.5A2.5 2.5 0 0 1 6.5 6h1.7l1.3-2h5l1.3 2h1.7A2.5 2.5 0 0 1 20 8.5v8a2.5 2.5 0 0 1-2.5 2.5h-11A2.5 2.5 0 0 1 4 16.5Z"/><circle cx="12" cy="12.5" r="3.5"/>',
  layers: '<path d="m12 3.5 8.5 4.5-8.5 4.5L3.5 8Z"/><path d="m3.5 12 8.5 4.5 8.5-4.5"/><path d="m3.5 16 8.5 4.5 8.5-4.5"/>',
  shield: '<path d="M12 3 19 6v5.5c0 4.5-3 7.8-7 9.5-4-1.7-7-5-7-9.5V6Z"/><path d="m9 12 2.2 2.2L15.5 10"/>',
  cloudoff: '<path d="M7 18.5h9.5a4 4 0 0 0 1.3-7.8A6 6 0 0 0 7.2 9 4.75 4.75 0 0 0 7 18.5Z"/><path d="m4 4 16 16"/>',
});

const USED = ['text', 'link', 'code', 'image', 'file', 'color', 'eye', 'bolt', 'camera', 'layers', 'enter', 'search', 'tab', 'command', 'lock', 'shield', 'key', 'cloudoff'];
for (const n of USED) {
  writeFileSync(new URL(`../docs/icons/${n}.svg`, import.meta.url), `<svg xmlns="http://www.w3.org/2000/svg" width="56" height="56" viewBox="0 0 56 56"><style>.t{fill:url(#d)}.r{stroke:rgba(241,207,139,.45)}.g{stroke:#f1cf8b}@media (prefers-color-scheme: light){.t{fill:url(#l)}.r{stroke:rgba(168,118,34,.4)}.g{stroke:#a87622}}</style><defs><linearGradient id="d" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#26262a"/><stop offset="1" stop-color="#141416"/></linearGradient><linearGradient id="l" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#ffffff"/><stop offset="1" stop-color="#f4f4f5"/></linearGradient></defs><rect class="t r" x="1" y="1" width="54" height="54" rx="15" stroke-width="1"/><g class="g" transform="translate(14 14) scale(1.1667)" fill="none" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">${I[n].replace(/ class="[^"]*"/g, '')}</g></svg>`);
}
console.log(`${USED.length} icons in docs/icons`);
