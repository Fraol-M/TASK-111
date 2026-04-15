ALTER TABLE payment_adjustments
    DROP COLUMN IF EXISTS state,
    DROP COLUMN IF EXISTS approved_by,
    DROP COLUMN IF EXISTS updated_at;
