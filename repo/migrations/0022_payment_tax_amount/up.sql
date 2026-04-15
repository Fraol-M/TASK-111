-- Model tax separately so loyalty points can be accrued on the net (taxable)
-- portion of a payment as required by the loyalty rules:
--   points = floor(net_amount_cents / 100)  where net = amount_cents - tax_cents
--
-- Payments carry the tax portion on the intent+payment rows so that a single
-- payment aggregate holds both the gross charge (amount_cents) and the tax
-- component (tax_cents). Historical rows default to 0 tax — their net equals
-- their gross and the points they previously accrued remain consistent.
ALTER TABLE payment_intents
    ADD COLUMN tax_cents BIGINT NOT NULL DEFAULT 0
        CHECK (tax_cents >= 0 AND tax_cents <= amount_cents);

ALTER TABLE payments
    ADD COLUMN tax_cents BIGINT NOT NULL DEFAULT 0
        CHECK (tax_cents >= 0 AND tax_cents <= amount_cents);
