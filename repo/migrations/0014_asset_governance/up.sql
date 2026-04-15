-- Asset governance / registry fields required by asset management policy
ALTER TABLE assets
    ADD COLUMN classification     TEXT,
    ADD COLUMN brand              TEXT,
    ADD COLUMN model              TEXT,
    ADD COLUMN owner_unit         TEXT,
    ADD COLUMN responsible_user_id UUID REFERENCES users(id),
    ADD COLUMN useful_life_months INTEGER;
