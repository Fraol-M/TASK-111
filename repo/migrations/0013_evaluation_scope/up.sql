-- Add participant scope to evaluations (JSON array of user/group IDs that this evaluation covers)
ALTER TABLE evaluations ADD COLUMN participant_scope JSONB NOT NULL DEFAULT '[]';
