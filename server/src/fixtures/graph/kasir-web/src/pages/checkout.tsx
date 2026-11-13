import { placeOrder, startQris } from '../api/client';

/** Layar checkout kasir: bayar tunai atau QRIS. */
export function CheckoutPage(props: { items: any[] }) {
  async function pay(method: string) {
    const res = await placeOrder(props.items);
    const order = await res.json();
    if (method === 'qris') await startQris(order.id);
  }
  return <button onClick={() => pay('qris')}>Bayar QRIS</button>;
}
