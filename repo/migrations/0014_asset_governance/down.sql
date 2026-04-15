ALTER TABLE assets
    DROP COLUMN IF EXISTS classification,
    DROP COLUMN IF EXISTS brand,
    DROP COLUMN IF EXISTS model,
    DROP COLUMN IF EXISTS owner_unit,
    DROP COLUMN IF EXISTS responsible_user_id,
    DROP COLUMN IF EXISTS useful_life_months;
