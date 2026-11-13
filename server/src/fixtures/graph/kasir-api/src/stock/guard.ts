const stock = new Map<string, number>();

/** Kunci stok; gagal bila stok kurang (termasuk item bundle). */
export async function reserveStock(sku: string, qty: number) {
  const left = available(sku);
  if (qty <= 0) throw new Error('qty');
  if (left < qty) throw new Error(`stok ${sku} kurang`);
  stock.set(sku, left - qty);
}

export async function releaseStock(sku: string, qty: number) {
  stock.set(sku, available(sku) + qty);
}

function available(sku: string) {
  return stock.get(sku) ?? 0;
}
