#!/usr/bin/env bash
set -euo pipefail

# ---------------------------------------------------------------------------
# run_tests.sh — Docker-based test runner for Venue Booking & Ops System
#
# Env file injection (no manual copy/paste required):
#   Priority: .env.test → .env → built-in defaults
#
# Usage:
#   ./run_tests.sh                         # run all tests
#   ./run_tests.sh test_login_success      # run a specific test
#
# Optional speed knobs:
#   TEST_THREADS=4 ./run_tests.sh          # parallel test execution
#   KEEP_VOLUMES=1 ./run_tests.sh          # keep DB/build caches (default)
#   KEEP_VOLUMES=0 ./run_tests.sh          # cold reset (old behavior)
# ---------------------------------------------------------------------------

SPECIFIC_TEST="${1:-}"
TEST_THREADS="${TEST_THREADS:-4}"
KEEP_VOLUMES="${KEEP_VOLUMES:-1}"

# Determine which env file to use
if [ -f ".env.test" ]; then
  ENV_FILE=".env.test"
  echo "[run_tests] Using .env.test"
elif [ -f ".env" ]; then
  ENV_FILE=".env"
  echo "[run_tests] .env.test not found; falling back to .env"
else
  # Write minimal test defaults to a temp file
  ENV_FILE="$(mktemp /tmp/venue_test_env.XXXXXX)"
  CLEANUP_ENV=1
  cat > "$ENV_FILE" <<'ENVEOF'
DATABASE_URL=postgres://venue:venue@db_test:5432/venue_ops_test
TEST_DATABASE_URL=postgres://venue:venue@db_test:5432/venue_ops_test
APP__SERVER__HOST=0.0.0.0
APP__SERVER__PORT=8081
APP__SERVER__WORKERS=1
APP__ENCRYPTION__KEY_HEX=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
APP__JWT__SECRET=test-secret-do-not-use-in-production
APP__JWT__EXPIRY_SECONDS=3600
APP__JOBS__HOLD_EXPIRY_INTERVAL_SECS=60
APP__JOBS__PAYMENT_TIMEOUT_INTERVAL_SECS=60
APP__JOBS__REMINDER_INTERVAL_SECS=3600
APP__JOBS__DND_RESOLVE_INTERVAL_SECS=300
APP__JOBS__ZERO_QTY_INTERVAL_SECS=600
APP__JOBS__TIER_RECALC_HOUR=2
APP__JOBS__BACKUP_HOUR=3
APP__BOOKING__HOLD_TIMEOUT_MINUTES=15
APP__PAYMENT__INTENT_TIMEOUT_MINUTES=30
APP__DND__START_HOUR=21
APP__DND__END_HOUR=7
APP__BACKUP__DIR=/tmp/test-backups
APP__STORAGE__RECONCILIATION_DIR=/tmp/test-reconciliation
APP__STORAGE__ATTACHMENTS_DIR=/tmp/test-attachments
APP__STORAGE__MAX_UPLOAD_BYTES=10485760
RUST_LOG=error
ENVEOF
  echo "[run_tests] No env file found; using built-in test defaults"
fi

COMPOSE_OPTS="-f docker-compose.test.yml --env-file $ENV_FILE"

cleanup() {
  echo "[run_tests] Tearing down test containers..."
  if [ "$KEEP_VOLUMES" = "0" ]; then
    docker compose $COMPOSE_OPTS down -v --remove-orphans 2>/dev/null || true
  else
    docker compose $COMPOSE_OPTS down --remove-orphans 2>/dev/null || true
  fi
  if [ "${CLEANUP_ENV:-0}" = "1" ]; then
    rm -f "$ENV_FILE"
  fi
}
trap cleanup EXIT

echo "[run_tests] Starting isolated test database..."
docker compose $COMPOSE_OPTS up -d db_test

echo "[run_tests] Waiting for PostgreSQL to be ready..."
RETRIES=30
until docker compose $COMPOSE_OPTS exec db_test pg_isready -U venue -d venue_ops_test 2>/dev/null; do
  RETRIES=$((RETRIES - 1))
  if [ "$RETRIES" -le 0 ]; then
    echo "[run_tests] ERROR: Database did not become ready in time."
    exit 1
  fi
  sleep 1
done
echo "[run_tests] Database is ready."

# Build test command
if [ -n "$SPECIFIC_TEST" ]; then
  TEST_CMD="cargo test $SPECIFIC_TEST -- --nocapture --test-threads=$TEST_THREADS"
else
  TEST_CMD="cargo test --all -- --test-threads=$TEST_THREADS"
fi

echo "[run_tests] Running: $TEST_CMD"
docker compose $COMPOSE_OPTS run --rm test_runner sh -c "$TEST_CMD"
