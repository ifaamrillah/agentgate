import express from 'express';
import { createOrder, getOrder, refundOrder } from './orders/handlers';
import { payWithQris, qrisCallback } from './payments/qris';

/** Daftarkan semua route HTTP kasir-api. */
export function buildServer() {
  const app = express();
  app.post('/v1/orders', createOrder);
  app.get('/v1/orders/:id', getOrder);
  app.post('/v1/orders/:id/refund', refundOrder);
  app.post('/v1/payments/qris', payWithQris);
  app.post('/v1/payments/qris/callback', qrisCallback);
  return app;
}
