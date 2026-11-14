// Logika murni autocomplete JQL (R3): teks sebelum kursor, sisip saran menggantikan kata yang
// sedang diketik, dan navigasi daftar saran. Dijalankan bin/tests/js_test.sh (node --test).
import { test } from "node:test";
import assert from "node:assert/strict";
import * as J from "../../assets/static/js/jql-core.mjs";

test("prefix: teks sebelum kursor (atau seluruh teks bila kursor tak diketahui)", () => {
  assert.equal(J.prefixAt("status = done", 6), "status");
  assert.equal(J.prefixAt("status", null), "status");
  assert.equal(J.prefixAt("abc", 99), "abc");
});

test("sisip: mengganti kata parsial di depan kursor, sisa teks setelah kursor tetap", () => {
  assert.deepEqual(J.apply("assig", 5, "assig", "assignee "), { value: "assignee ", cursor: 9 });
  assert.deepEqual(J.apply("status = do AND x", 11, "do", "done "), { value: "status = done AND x", cursor: 14 });
  // Kutip terbuka diganti seluruhnya.
  assert.deepEqual(J.apply('sprint = "Spr', 13, '"Spr', '"Sprint 4" '), { value: 'sprint = "Sprint 4" ', cursor: 20 });
  // Tanpa parsial: sisip di kursor.
  assert.deepEqual(J.apply("label = ", 8, "", "kasir "), { value: "label = kasir ", cursor: 14 });
  // Parsial tidak cocok dengan teks (jawaban basi): sisip di kursor tanpa menghapus apa pun.
  assert.deepEqual(J.apply("label = ab", 10, "zz", "x"), { value: "label = abx", cursor: 11 });
});

test("sisip: spasi ganda tidak dibuat bila setelah kursor sudah ada spasi", () => {
  assert.deepEqual(J.apply("status  AND", 6, "status", "status "), { value: "status  AND", cursor: 7 });
});

test("navigasi daftar: berputar, -1 = tidak ada yang aktif", () => {
  assert.equal(J.move(-1, 3, 1), 0);
  assert.equal(J.move(2, 3, 1), 0);
  assert.equal(J.move(0, 3, -1), 2);
  assert.equal(J.move(-1, 3, -1), 2);
  assert.equal(J.move(1, 0, 1), -1);
});
