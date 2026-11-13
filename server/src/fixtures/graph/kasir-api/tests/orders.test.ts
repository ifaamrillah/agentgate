import { reserveStock } from '../src/stock/guard';

test('stok kurang ditolak', async () => {
  await expect(reserveStock('kopi', 3)).rejects.toThrow();
});
