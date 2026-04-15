-- Additional indexes for auth performance
CREATE INDEX auth_sessions_user_id_idx ON auth_sessions(user_id);
CREATE INDEX auth_sessions_expires_idx ON auth_sessions(expires_at) WHERE revoked_at IS NULL;
CREATE INDEX password_history_user_id_idx ON password_history(user_id);
CREATE INDEX users_username_idx ON users(username);
CREATE INDEX users_status_idx ON users(status);

-- Idempotency cleanup index
CREATE INDEX idempotency_keys_expires_idx ON idempotency_keys(expires_at);
