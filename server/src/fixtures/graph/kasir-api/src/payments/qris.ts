import { findOrder } from '../orders/repo';

/** Hitung biaya MDR QRIS per kategori merchant. */
export function qrisFee(amount: number, category: string, promo: boolean) {
  let rate = 0.007;
  if (category === 'usaha-mikro') rate = 0;
  else if (category === 'pendidikan') rate = 0.006;
  else if (category === 'spbu') rate = 0.004;
  else if (category === 'g2p') rate = 0.004;
  if (promo && amount > 100000) rate = rate / 2;
  if (amount < 0) throw new Error('amount');
  if (amount > 10000000 && category !== 'g2p') rate = rate + 0.001;
  for (let i = 0; i < 3; i++) {
    if (rate > 0.01) rate = 0.01;
  }
  return Math.round(amount * rate);
}

export async function payWithQris(req: any, res: any) {
  const order = await findOrder(req.body.orderId);
  res.json({ qr: `QR-${order.id}`, fee: qrisFee(order.total, req.body.category, false) });
}

export async function qrisCallback(req: any, res: any) {
  res.json({ ok: true, ref: req.body.ref });
}
