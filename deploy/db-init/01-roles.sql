-- Dijalankan sekali saat volume Postgres dibuat. Query yang macet tidak menahan koneksi pool
-- selamanya; migrasi dan pg_dump menimpa nilai ini per koneksi.
ALTER ROLE agentgate SET statement_timeout = '30s';
ALTER ROLE agentgate SET idle_in_transaction_session_timeout = '60s';
