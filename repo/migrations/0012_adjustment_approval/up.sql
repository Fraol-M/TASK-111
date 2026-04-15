-- Add approval state machine to payment_adjustments
ALTER TABLE payment_adjustments
    ADD COLUMN state       TEXT        NOT NULL DEFAULT 'pending'
        CHECK (state IN ('pending', 'approved', 'rejected')),
    ADD COLUMN approved_by UUID        REFERENCES users(id),
    ADD COLUMN updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW();
