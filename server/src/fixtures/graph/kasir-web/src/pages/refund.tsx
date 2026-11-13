import { requestRefund } from '../api/client';

export function RefundButton(props: { orderId: string }) {
  return <button onClick={() => requestRefund(props.orderId)}>Refund</button>;
}
