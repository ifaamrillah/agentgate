const orders = new Map<string, any>();

export async function saveOrder(items: any[]) {
  const id = String(orders.size + 1);
  const order = { id, items, status: 'paid' };
  orders.set(id, order);
  return order;
}

export async function findOrder(id: string) {
  return orders.get(id);
}

export async function markRefunded(id: string) {
  const order = orders.get(id);
  order.status = 'refunded';
  return order;
}

/** Sisa migrasi lama, tidak dipanggil lagi. */
export function legacyOrderNumber(id: string) {
  return `ORD-${id.padStart(6, '0')}`;
}
