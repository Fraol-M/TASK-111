-- Enforce append-only semantics on financial/audit ledgers and the audit log.
-- These tables form evidentiary trails: once inserted, rows must not be altered
-- or deleted at the DB layer. A trigger raises an exception on UPDATE/DELETE,
-- so even accidental or privileged application-layer writes are rejected.
--
-- Tables covered:
--   * audit_logs       — tamper-evident application audit chain
--   * points_ledger    — loyalty points ledger (every earn/redeem/adjust)
--   * wallet_ledger    — wallet top-up/adjust/redeem entries
--   * inventory_ledger — inventory movement ledger
--
-- If a legitimate business correction is required, it MUST be expressed as a
-- new compensating row (negative delta, reversal ledger entry, or
-- superseding audit event) — never as an in-place mutation.

CREATE OR REPLACE FUNCTION reject_ledger_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION
        'Append-only table %: UPDATE and DELETE are not permitted (op=%). '
        'Record a compensating row instead.',
        TG_TABLE_NAME, TG_OP
        USING ERRCODE = 'insufficient_privilege';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER audit_logs_append_only
    BEFORE UPDATE OR DELETE ON audit_logs
    FOR EACH ROW EXECUTE FUNCTION reject_ledger_mutation();

CREATE TRIGGER points_ledger_append_only
    BEFORE UPDATE OR DELETE ON points_ledger
    FOR EACH ROW EXECUTE FUNCTION reject_ledger_mutation();

CREATE TRIGGER wallet_ledger_append_only
    BEFORE UPDATE OR DELETE ON wallet_ledger
    FOR EACH ROW EXECUTE FUNCTION reject_ledger_mutation();

CREATE TRIGGER inventory_ledger_append_only
    BEFORE UPDATE OR DELETE ON inventory_ledger
    FOR EACH ROW EXECUTE FUNCTION reject_ledger_mutation();
