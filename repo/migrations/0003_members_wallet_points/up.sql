-- Member tier levels
CREATE TYPE member_tier AS ENUM ('silver', 'gold', 'platinum');

-- Reason for blacklist/freeze action
CREATE TYPE blacklist_reason AS ENUM (
    'fraud',
    'payment_default',
    'policy_violation',
    'manual'
);

-- Points transaction types
CREATE TYPE points_txn_type AS ENUM ('earn', 'redeem', 'adjust', 'expire');

-- Wallet transaction types
CREATE TYPE wallet_txn_type AS ENUM ('top_up', 'debit', 'refund', 'adjustment');

-- Member profile and loyalty data
CREATE TABLE members (
    user_id                UUID        PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    tier                   member_tier NOT NULL DEFAULT 'silver',
    points_balance         INTEGER     NOT NULL DEFAULT 0 CHECK (points_balance >= 0),
    -- AES-GCM encrypted string of cent integer (e.g. encrypt("10099") = "$100.99")
    wallet_balance         TEXT        NOT NULL DEFAULT '',
    blacklist_flag         BOOLEAN     NOT NULL DEFAULT FALSE,
    blacklist_reason       blacklist_reason,
    blacklisted_at         TIMESTAMPTZ,
    redemption_frozen_until TIMESTAMPTZ,
    -- Rolling 12-month net spend in cents (updated on payment capture)
    rolling_12m_spend      BIGINT      NOT NULL DEFAULT 0,
    -- Optimistic concurrency version
    version                INTEGER     NOT NULL DEFAULT 0,
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Tier change history (tamper-evident)
CREATE TABLE member_tier_history (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    from_tier       member_tier,
    to_tier         member_tier NOT NULL,
    reason          TEXT,
    actor_user_id   UUID        REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Append-only points ledger (source of truth)
CREATE TABLE points_ledger (
    id              UUID            PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID            NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    txn_type        points_txn_type NOT NULL,
    delta           INTEGER         NOT NULL,
    balance_after   INTEGER         NOT NULL CHECK (balance_after >= 0),
    reference_id    UUID,
    note            TEXT,
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW()
);

-- Append-only wallet ledger (source of truth)
CREATE TABLE wallet_ledger (
    id                  UUID            PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID            NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    txn_type            wallet_txn_type NOT NULL,
    delta_cents         BIGINT          NOT NULL,
    balance_after_cents BIGINT          NOT NULL,
    reference_id        UUID,
    note                TEXT,
    created_at          TIMESTAMPTZ     NOT NULL DEFAULT NOW()
);

-- Member notification and channel preferences
CREATE TABLE member_preferences (
    user_id                  UUID        PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    -- JSON array of opted-out trigger categories e.g. ["booking_t24h"]
    notification_opt_out     JSONB       NOT NULL DEFAULT '[]',
    preferred_channel        TEXT        NOT NULL DEFAULT 'in_app',
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Blacklist and freeze events (audit trail)
CREATE TABLE blacklist_events (
    id              UUID             PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID             NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    action          TEXT             NOT NULL,   -- 'blacklist','freeze','unblacklist','unfreeze'
    reason          blacklist_reason,
    duration_days   INTEGER,
    note            TEXT,
    actor_user_id   UUID             REFERENCES users(id),
    created_at      TIMESTAMPTZ      NOT NULL DEFAULT NOW()
);
