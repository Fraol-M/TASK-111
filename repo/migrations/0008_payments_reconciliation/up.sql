-- Payment intent lifecycle states
CREATE TYPE intent_state AS ENUM (
    'open',
    'captured',
    'failed',
    'timed_out',
    'cancelled'
);

-- Payment record states
CREATE TYPE payment_state AS ENUM (
    'pending',
    'completed',
    'failed',
    'refunded',
    'partially_refunded'
);

-- Refund approval states
CREATE TYPE refund_state AS ENUM (
    'pending',
    'approved',
    'rejected',
    'processed'
);

-- Payment intents (pre-authorization; auto-expires after 30 minutes)
CREATE TABLE payment_intents (
    id              UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    booking_id      UUID         REFERENCES bookings(id),
    member_id       UUID         NOT NULL REFERENCES users(id),
    amount_cents    BIGINT       NOT NULL CHECK (amount_cents > 0),
    state           intent_state NOT NULL DEFAULT 'open',
    idempotency_key TEXT         NOT NULL UNIQUE,
    expires_at      TIMESTAMPTZ  NOT NULL,
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

-- Settled payment records
CREATE TABLE payments (
    id                 UUID          PRIMARY KEY DEFAULT gen_random_uuid(),
    intent_id          UUID          NOT NULL REFERENCES payment_intents(id),
    member_id          UUID          NOT NULL REFERENCES users(id),
    booking_id         UUID          REFERENCES bookings(id),
    amount_cents       BIGINT        NOT NULL CHECK (amount_cents > 0),
    payment_method     TEXT          NOT NULL,
    state              payment_state NOT NULL DEFAULT 'pending',
    idempotency_key    TEXT          NOT NULL UNIQUE,
    external_reference TEXT,
    version            INTEGER       NOT NULL DEFAULT 0,
    created_at         TIMESTAMPTZ   NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMPTZ   NOT NULL DEFAULT NOW()
);

-- Refund requests (partial or full; cumulative cap enforced in service)
CREATE TABLE refunds (
    id              UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    payment_id      UUID         NOT NULL REFERENCES payments(id),
    amount_cents    BIGINT       NOT NULL CHECK (amount_cents > 0),
    reason          TEXT,
    state           refund_state NOT NULL DEFAULT 'pending',
    idempotency_key TEXT         NOT NULL UNIQUE,
    requested_by    UUID         NOT NULL REFERENCES users(id),
    approved_by     UUID         REFERENCES users(id),
    created_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

-- Manual compensation adjustments
CREATE TABLE payment_adjustments (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    payment_id   UUID        NOT NULL REFERENCES payments(id),
    amount_cents BIGINT      NOT NULL,
    reason       TEXT        NOT NULL,
    created_by   UUID        REFERENCES users(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Reconciliation import header (one per file upload)
CREATE TABLE reconciliation_imports (
    id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    file_name      TEXT        NOT NULL,
    -- SHA-256 hex of uploaded file bytes (duplicate detection)
    file_checksum  TEXT        NOT NULL UNIQUE,
    status         TEXT        NOT NULL DEFAULT 'pending',
    total_rows     INTEGER     NOT NULL DEFAULT 0,
    matched_rows   INTEGER     NOT NULL DEFAULT 0,
    unmatched_rows INTEGER     NOT NULL DEFAULT 0,
    imported_by    UUID        REFERENCES users(id),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Individual reconciliation row records
CREATE TABLE reconciliation_rows (
    id                    UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    import_id             UUID        NOT NULL REFERENCES reconciliation_imports(id) ON DELETE CASCADE,
    external_reference    TEXT        NOT NULL,
    external_amount_cents BIGINT      NOT NULL,
    payment_id            UUID        REFERENCES payments(id),
    internal_amount_cents BIGINT,
    discrepancy_cents     BIGINT,
    status                TEXT        NOT NULL DEFAULT 'unmatched',
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
