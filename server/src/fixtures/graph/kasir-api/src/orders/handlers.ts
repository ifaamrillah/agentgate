import { reserveStock, releaseStock } from '../stock/guard';
import { saveOrder, findOrder, markRefunded } from './repo';

/** Buat pesanan baru dan kunci stok tiap item. */
export async function createOrder(req: any, res: any) {
  const items = req.body.items;
  for (const item of items) {
    await reserveStock(item.sku, item.qty);
  }
  const order = await saveOrder(items);
  res.json(order);
}

/** Ambil satu pesanan. */
export async function getOrder(req: any, res: any) {
  res.json(await findOrder(req.params.id));
}

/** Refund pesanan: kembalikan stok lalu tandai refund. */
export async function refundOrder(req: any, res: any) {
  const order = await findOrder(req.params.id);
  for (const item of order.items) {
    await releaseStock(item.sku, item.qty);
  }
  res.json(await markRefunded(order.id));
}
