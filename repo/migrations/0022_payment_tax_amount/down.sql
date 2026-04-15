ALTER TABLE payments DROP COLUMN IF EXISTS tax_cents;
ALTER TABLE payment_intents DROP COLUMN IF EXISTS tax_cents;
