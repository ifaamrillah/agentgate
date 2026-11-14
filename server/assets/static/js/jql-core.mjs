// Logika murni autocomplete JQL (R3), dipakai jql-suggest.js dan diuji node
// (server/tests/js/jql_core.test.mjs). Server (GET …/search/advanced/suggest) menentukan konteks
// dan saran; modul ini hanya menyisipkan saran dan menggerakkan pilihan.

/** Teks sebelum kursor; kursor tak diketahui = seluruh teks. */
export const prefixAt = (value, cursor) => (cursor == null ? value : value.slice(0, Math.min(cursor, value.length)));

/**
 * Sisipkan `insert` menggantikan `partial` yang berada tepat sebelum kursor. Bila teks sebelum
 * kursor tidak berakhir dengan `partial` (jawaban server basi), sisip di kursor saja. Spasi akhir
 * sisipan dilewati bila setelah kursor sudah ada spasi.
 * @returns {{ value: string, cursor: number }}
 */
export function apply(value, cursor, partial, insert) {
  const at = cursor == null ? value.length : Math.min(cursor, value.length);
  const before = value.slice(0, at);
  const after = value.slice(at);
  const start = partial && before.endsWith(partial) ? at - partial.length : at;
  let text = insert;
  if (text.endsWith(" ") && /^\s/.test(after)) text = text.slice(0, -1);
  const head = value.slice(0, start) + text;
  return { value: head + after, cursor: head.length + (text === insert ? 0 : 1) };
}

/** Indeks aktif berikutnya di daftar `count` item (berputar); -1 = tidak ada. */
export function move(active, count, delta) {
  if (count <= 0) return -1;
  if (active < 0) return delta > 0 ? 0 : count - 1;
  return (active + delta + count) % count;
}
