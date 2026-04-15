-- Notification delivery channels
CREATE TYPE notification_channel AS ENUM ('in_app', 'email', 'sms', 'push');

-- Notification delivery states
CREATE TYPE delivery_state AS ENUM (
    'pending',
    'delivered',
    'failed',
    'suppressed_dnd',
    'opted_out'
);

-- Triggers that fire notifications
CREATE TYPE template_trigger AS ENUM (
    'booking_confirmed',
    'booking_cancelled',
    'booking_changed',
    'booking_reminder_24h',
    'booking_completed',
    'booking_exception',
    'payment_captured',
    'refund_approved',
    'points_earned',
    'tier_upgraded',
    'tier_downgraded',
    'wallet_topup',
    'custom'
);

-- Notification templates with typed variable schema
CREATE TABLE notification_templates (
    id               UUID                 PRIMARY KEY DEFAULT gen_random_uuid(),
    name             TEXT                 NOT NULL,
    trigger_type     template_trigger     NOT NULL,
    channel          notification_channel NOT NULL,
    subject_template TEXT,
    body_template    TEXT                 NOT NULL,
    variable_schema  JSONB,
    is_critical      BOOLEAN              NOT NULL DEFAULT FALSE,
    created_at       TIMESTAMPTZ          NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ          NOT NULL DEFAULT NOW(),
    UNIQUE (trigger_type, channel)
);

-- Notification instances (one per user per event)
CREATE TABLE notifications (
    id             UUID                 PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id        UUID                 NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    template_id    UUID                 REFERENCES notification_templates(id),
    trigger_type   template_trigger     NOT NULL,
    channel        notification_channel NOT NULL,
    subject        TEXT,
    body           TEXT                 NOT NULL,
    payload_hash   TEXT                 NOT NULL,
    delivery_state delivery_state       NOT NULL DEFAULT 'pending',
    dnd_suppressed BOOLEAN              NOT NULL DEFAULT FALSE,
    read_at        TIMESTAMPTZ,
    created_at     TIMESTAMPTZ          NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ          NOT NULL DEFAULT NOW()
);

-- Delivery attempt log (one row per send attempt)
CREATE TABLE notification_attempts (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    notification_id UUID        NOT NULL REFERENCES notifications(id) ON DELETE CASCADE,
    attempted_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    succeeded       BOOLEAN     NOT NULL,
    error_detail    TEXT
);

-- Group threads for on-site program communication
CREATE TABLE group_threads (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT        NOT NULL,
    description TEXT,
    created_by  UUID        REFERENCES users(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Group membership
CREATE TABLE group_members (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    thread_id   UUID        NOT NULL REFERENCES group_threads(id) ON DELETE CASCADE,
    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    joined_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    removed_at  TIMESTAMPTZ,
    UNIQUE (thread_id, user_id)
);

-- Group messages
CREATE TABLE group_messages (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    thread_id  UUID        NOT NULL REFERENCES group_threads(id) ON DELETE CASCADE,
    sender_id  UUID        NOT NULL REFERENCES users(id),
    body       TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Per-user read receipts for group messages
CREATE TABLE group_message_receipts (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    message_id UUID        NOT NULL REFERENCES group_messages(id) ON DELETE CASCADE,
    user_id    UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    read_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (message_id, user_id)
);

-- DND suppression queue: notifications to re-deliver after DND window
CREATE TABLE dnd_queue (
    id                   UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    notification_id      UUID        NOT NULL REFERENCES notifications(id) ON DELETE CASCADE,
    user_id              UUID        NOT NULL REFERENCES users(id),
    scheduled_deliver_at TIMESTAMPTZ NOT NULL,
    processed_at         TIMESTAMPTZ,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
