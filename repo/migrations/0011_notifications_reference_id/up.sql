-- Add reference_id to notifications for booking-scoped deduplication
ALTER TABLE notifications ADD COLUMN reference_id UUID;

CREATE INDEX notifications_user_trigger_ref_idx
    ON notifications (user_id, trigger_type, reference_id)
    WHERE reference_id IS NOT NULL;
