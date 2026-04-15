DROP INDEX IF EXISTS notifications_user_trigger_ref_idx;
ALTER TABLE notifications DROP COLUMN IF EXISTS reference_id;
