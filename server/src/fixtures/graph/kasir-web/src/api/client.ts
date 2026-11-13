/** Klien HTTP kasir-api. */
export async function placeOrder(items: any[]) {
  return fetch('/v1/orders', { method: 'POST', body: JSON.stringify({ items }) });
}

export async function requestRefund(id: string) {
  return fetch(`/v1/orders/${id}/refund`, { method: 'POST' });
}

export async function startQris(orderId: string) {
  return fetch('/v1/payments/qris', { method: 'POST', body: JSON.stringify({ orderId }) });
}
