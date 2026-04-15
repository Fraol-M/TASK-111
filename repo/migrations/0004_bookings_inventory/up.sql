-- Booking lifecycle states
CREATE TYPE booking_state AS ENUM (
    'draft',
    'held',
    'confirmed',
    'changed',
    'cancelled',
    'completed',
    'exception_pending',
    'expired'
);

-- Inventory publish status
CREATE TYPE publish_status AS ENUM ('published', 'unpublished', 'archived');

-- Pickup points (physical locations)
-- NOTE: cutoff_hours column added in migration 0019_cutoff_by_zone_pickup
CREATE TABLE pickup_points (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT        NOT NULL,
    address     TEXT,
    active      BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Delivery zones
-- NOTE: cutoff_hours column added in migration 0019_cutoff_by_zone_pickup
CREATE TABLE delivery_zones (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT        NOT NULL,
    description TEXT,
    active      BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Inventory items (sellable units)
CREATE TABLE inventory_items (
    id              UUID            PRIMARY KEY DEFAULT gen_random_uuid(),
    sku             TEXT            UNIQUE NOT NULL,
    name            TEXT            NOT NULL,
    description     TEXT,
    available_qty   INTEGER         NOT NULL DEFAULT 0 CHECK (available_qty >= 0),
    safety_stock    INTEGER         NOT NULL DEFAULT 0 CHECK (safety_stock >= 0),
    publish_status  publish_status  NOT NULL DEFAULT 'published',
    pickup_point_id UUID            REFERENCES pickup_points(id),
    zone_id         UUID            REFERENCES delivery_zones(id),
    -- Fulfillment cutoff hours before start_at
    cutoff_hours    INTEGER         NOT NULL DEFAULT 2,
    version         INTEGER         NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW()
);

-- Bookings (created before inventory_holds to allow FK)
CREATE TABLE bookings (
    id                      UUID            PRIMARY KEY DEFAULT gen_random_uuid(),
    member_id               UUID            NOT NULL REFERENCES users(id),
    state                   booking_state   NOT NULL DEFAULT 'draft',
    start_at                TIMESTAMPTZ     NOT NULL,
    end_at                  TIMESTAMPTZ     NOT NULL,
    inventory_hold_expires_at TIMESTAMPTZ,
    change_reason           TEXT,
    pickup_point_id         UUID            REFERENCES pickup_points(id),
    zone_id                 UUID            REFERENCES delivery_zones(id),
    total_cents             BIGINT          NOT NULL DEFAULT 0,
    version                 INTEGER         NOT NULL DEFAULT 0,
    created_at              TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    updated_at              TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_booking_times CHECK (end_at > start_at)
);

-- Inventory holds (atomic reservations with 15-minute timeout)
CREATE TABLE inventory_holds (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    inventory_item_id   UUID        NOT NULL REFERENCES inventory_items(id),
    booking_id          UUID        REFERENCES bookings(id),
    quantity            INTEGER     NOT NULL CHECK (quantity > 0),
    expires_at          TIMESTAMPTZ NOT NULL,
    released_at         TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Booking line items
CREATE TABLE booking_items (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    booking_id          UUID        NOT NULL REFERENCES bookings(id) ON DELETE CASCADE,
    inventory_item_id   UUID        NOT NULL REFERENCES inventory_items(id),
    quantity            INTEGER     NOT NULL CHECK (quantity > 0),
    unit_price_cents    BIGINT      NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Booking state change history (tamper-evident)
CREATE TABLE booking_status_history (
    id              UUID            PRIMARY KEY DEFAULT gen_random_uuid(),
    booking_id      UUID            NOT NULL REFERENCES bookings(id) ON DELETE CASCADE,
    from_state      booking_state,
    to_state        booking_state   NOT NULL,
    reason          TEXT,
    actor_user_id   UUID            REFERENCES users(id),
    created_at      TIMESTAMPTZ     NOT NULL DEFAULT NOW()
);

-- Append-only inventory ledger (source of truth for movements)
CREATE TABLE inventory_ledger (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    inventory_item_id   UUID        NOT NULL REFERENCES inventory_items(id),
    delta               INTEGER     NOT NULL,
    qty_after           INTEGER     NOT NULL,
    reason              TEXT        NOT NULL,
    -- Unique for idempotent writes
    correlation_id      TEXT        UNIQUE,
    actor_user_id       UUID        REFERENCES users(id),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Internal restock alerts generated when inventory hits zero or safety stock
CREATE TABLE restock_alerts (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    inventory_item_id   UUID        NOT NULL REFERENCES inventory_items(id),
    triggered_qty       INTEGER     NOT NULL DEFAULT 0,
    triggered_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    acknowledged_at     TIMESTAMPTZ,
    acknowledged_by     UUID        REFERENCES users(id)
);
