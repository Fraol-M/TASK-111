# Reviewer Checklist

## Requirement → File → Test Mapping

| Requirement | Primary File(s) | Test(s) |
|------------|-----------------|---------|
| Booking state machine | `src/bookings/state_machine.rs` | `src/bookings/state_machine.rs#tests`, `tests/booking_lifecycle.rs` |
| Inventory atomic holds | `src/inventory/service.rs:create_hold` | `src/inventory/service.rs` (FOR UPDATE lock) |
| Inventory oversell prevention | `src/inventory/service.rs` — `available_qty >= qty` check | `tests/high_risk.rs` |
| Optimistic concurrency | All `repository.rs` with `version.eq(expected_version)` | `tests/asset_versioning.rs` |
| AES-256-GCM encryption | `src/common/crypto.rs` | `tests/asset_versioning.rs#test_encrypt_decrypt_round_trip` |
| Argon2id password hashing | `src/auth/service.rs:hash_password` | `tests/auth_flow.rs` |
| JWT + session revocation | `src/common/extractors.rs`, `src/auth/repository.rs` | `tests/auth_flow.rs#test_login_and_logout_flow` |
| DND suppression | `src/notifications/service.rs:is_dnd_active` | `tests/notification_dnd.rs` |
| DND queue redelivery | `src/notifications/service.rs:deliver_dnd_queue` | `tests/notification_dnd.rs#test_deliver_dnd_queue_marks_suppressed_notification_delivered` |
| Multi-channel dispatch with in_app fallback | `src/notifications/service.rs:dispatch_to_channel` — non-in_app returns Err + in_app fallback created; migration 0021 | `tests/notification_dnd.rs` |
| Template variable validation | `src/notifications/service.rs:render_template` | `tests/notification_dnd.rs#test_template_render_missing_var_returns_error` |
| Refund cap enforcement | `src/payments/service.rs:request_refund` | `tests/high_risk.rs` |
| Reconciliation checksum | `src/reconciliation/service.rs:compute_checksum` + `import_file` | `src/reconciliation/service.rs#tests::checksum_is_deterministic_and_hex`, `…::checksum_distinguishes_byte_changes` |
| CSV column-order-agnostic parsing | `src/reconciliation/service.rs:parse_csv` | `src/reconciliation/service.rs#tests::parse_csv_is_column_order_agnostic`, `…::parse_csv_rejects_missing_required_column`, `…::parse_csv_rejects_invalid_amount`, `…::parse_csv_rejects_invalid_date_format` |
| Asset versioning (before-image) | `src/assets/repository.rs:update_asset` | `tests/asset_versioning.rs` |
| Asset cost masking | `src/assets/service.rs:mask_or_decrypt_cost` | `tests/asset_versioning.rs#test_mask_output_format` |
| Evaluation node-level permission | `src/evaluations/service.rs:transition_assignment` | `tests/asset_versioning.rs#test_evaluator_outside_scope_denied` |
| Background jobs | `src/jobs/bootstrap.rs` | `src/jobs/*.rs` |
| Hold expiry job | `src/jobs/hold_expiry.rs` | `src/bookings/state_machine.rs#test_held_to_expired_allowed` |
| Tier recalculation | `src/jobs/tier_recalc.rs`, `src/members/service.rs:recalculate_tier` | `tests/high_risk.rs` |
| Database backup | `src/jobs/backup.rs` | — |
| Pagination | `src/common/pagination.rs` | All list endpoints |
| Audit logging | `src/common/audit.rs`, `src/audit/` | All audited routes |
| RBAC extractors | `src/common/extractors.rs` | `tests/auth_flow.rs#test_member_forbidden_from_admin_route` |

## Static Audit Checklist

- [x] All domain routes registered in `src/app.rs` match `docs/api-matrix.md` (`/health` is at root, not under `/api/v1`)
- [x] Migrations cover all tables referenced in `src/schema.rs`
- [x] `.env.example` keys match `src/config/mod.rs` field names (including `APP__BOOKING__INVENTORY_STRATEGY`)
- [x] `docs/api-matrix.md` covers every route group; idempotency is via `Idempotency-Key` header (bookings/members) or `idempotency_key` body field (payments)
- [x] `docs/permission-matrix.md` covers every route × role combination; preferences GET is self/admin only
- [x] `docs/state-machines.md` covers booking, evaluation, payment intent, notification dispatch
- [x] Every state machine has unit tests for invalid transitions
- [x] Idempotency hash: `tests/idempotency.rs` verifies same-request → same-hash
- [x] Handler/service-level idempotency controls applied (no Actix middleware — each handler calls `common::idempotency::check_or_register` before the write and `store_response` on success): bookings (`src/bookings/handler.rs`), points adjust + wallet top-up (`src/members/handler.rs`), payment intents/captures/refunds (`src/payments/handler.rs` + service via `idempotency_key` body field). Inventory holds use an atomic `inventory_ledger.correlation_id` UNIQUE + `ON CONFLICT DO NOTHING` (`src/inventory/service.rs`).
- [x] No float arithmetic for money anywhere in codebase (all `i64` cents)
- [x] `audit_logs` has no UPDATE/DELETE in any service code
- [x] `run_tests.sh` exists and is executable; auto-injects env file
- [x] `docker-compose.test.yml` defines isolated `db_test` + `test_runner` services
- [x] `.env.test` and `.env.example` present
- [x] `docker compose up --build` and `./run_tests.sh` are the only commands needed

## Running the System

```bash
# Start full stack (app + postgres)
docker compose up --build

# Health check
curl http://localhost:8080/health
# Expected: {"status":"ok","db":"ok"}

# Run all tests in isolated Docker environment
chmod +x run_tests.sh
./run_tests.sh
```
