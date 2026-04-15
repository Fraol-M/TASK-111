-- Asset lifecycle status
CREATE TYPE asset_status AS ENUM (
    'active',
    'maintenance',
    'retired',
    'disposed'
);

-- Depreciation calculation methods
CREATE TYPE depreciation_method AS ENUM (
    'straight_line',
    'declining_balance',
    'none'
);

-- Asset master data
CREATE TABLE assets (
    id                  UUID                PRIMARY KEY DEFAULT gen_random_uuid(),
    asset_code          TEXT                NOT NULL UNIQUE,
    name                TEXT                NOT NULL,
    description         TEXT,
    status              asset_status        NOT NULL DEFAULT 'active',
    -- AES-GCM encrypted cent string (masked for non-Finance/Admin roles)
    procurement_cost    TEXT,
    depreciation_method depreciation_method NOT NULL DEFAULT 'straight_line',
    useful_life_years   INTEGER,
    purchase_date       DATE,
    location            TEXT,
    -- Optimistic concurrency version
    version             INTEGER             NOT NULL DEFAULT 0,
    created_at          TIMESTAMPTZ         NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ         NOT NULL DEFAULT NOW()
);

-- Immutable version snapshots — created before every asset edit
CREATE TABLE asset_versions (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    asset_id      UUID        NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    version_no    INTEGER     NOT NULL,
    snapshot_json JSONB       NOT NULL,
    created_by    UUID        REFERENCES users(id),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (asset_id, version_no)
);

-- Asset file attachments (stored locally, UUID-named to prevent path traversal)
CREATE TABLE asset_attachments (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    asset_id    UUID        NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    file_name   TEXT        NOT NULL,
    stored_name TEXT        NOT NULL UNIQUE,
    mime_type   TEXT        NOT NULL,
    size_bytes  BIGINT      NOT NULL,
    uploaded_by UUID        NOT NULL REFERENCES users(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
