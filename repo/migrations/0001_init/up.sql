-- Enable pgcrypto for UUID generation
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- User role enum
CREATE TYPE user_role AS ENUM (
    'administrator',
    'operations_manager',
    'finance',
    'asset_manager',
    'evaluator',
    'member'
);

-- User account status
CREATE TYPE user_status AS ENUM (
    'active',
    'suspended',
    'deleted'
);

-- Core users table
CREATE TABLE users (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    username    VARCHAR(100) UNIQUE NOT NULL,
    password_hash TEXT       NOT NULL,
    role        user_role   NOT NULL DEFAULT 'member',
    status      user_status NOT NULL DEFAULT 'active',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Password history for preventing reuse
CREATE TABLE password_history (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    password_hash TEXT      NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Auth sessions (server-side session tracking for revocation)
CREATE TABLE auth_sessions (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  TEXT        NOT NULL UNIQUE,
    expires_at  TIMESTAMPTZ NOT NULL,
    revoked_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_session_expires CHECK (expires_at > created_at)
);

-- Idempotency key store for replay-safe writes
CREATE TABLE idempotency_keys (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    key_value       TEXT        UNIQUE NOT NULL,
    request_hash    TEXT        NOT NULL,
    response_status SMALLINT,
    response_body   TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ NOT NULL
);

-- Append-only audit log (app user has INSERT only, no UPDATE/DELETE)
CREATE TABLE audit_logs (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    correlation_id  TEXT,
    actor_user_id   UUID        REFERENCES users(id),
    action          TEXT        NOT NULL,
    entity_type     TEXT        NOT NULL,
    entity_id       TEXT        NOT NULL,
    old_value       JSONB,
    new_value       JSONB,
    metadata        JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
