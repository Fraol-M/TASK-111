# Audit Report 2 - Issue Recheck (Static)

Date: 2026-04-16
Scope: Re-checked only the five issues listed in `.tmp/audit_report-2.md` using static code review (no runtime/test execution).

## Verdict
- Fixed: 5/5
- Partially fixed: 0/5
- Unfixed: 0/5

## Issue-by-issue status

1. **Prompt role model incomplete (`Reviewer` not represented)**  
   - **Status:** Fixed  
   - **Evidence:** `repo/src/users/model.rs:24` adds `reviewer`; `repo/migrations/0026_reviewer_role/up.sql:7` adds enum value; role policies include reviewer in `repo/src/users/policy.rs:38` and `repo/src/users/policy.rs:41`; reviewer extractor exists in `repo/src/common/extractors.rs:103`.

2. **Pickup-point/zone cutoff configurability lacks API management surface**  
   - **Status:** Fixed  
   - **Evidence:** CRUD-style routes for pickup points and zones are registered in `repo/src/inventory/handler.rs:21` to `repo/src/inventory/handler.rs:31`; app wires inventory module in `repo/src/app.rs:59`; API docs list cutoff management endpoints in `repo/docs/api-matrix.md:34` to `repo/docs/api-matrix.md:43`.

3. **Tier-change tamper-evident audit incomplete on automatic recalculation path**  
   - **Status:** Fixed  
   - **Evidence:** `recalculate_tier` now writes audit log `tier_recalculated` in `repo/src/members/service.rs:463` with hash-chain fields (`row_hash`/`previous_hash`) passed to audited insert (`repo/src/members/service.rs:474` to `repo/src/members/service.rs:475`); daily job path calls this function in `repo/src/jobs/tier_recalc.rs:68`.

4. **Manual adjustment creation lacks immutable audit event**  
   - **Status:** Fixed  
   - **Evidence:** `create_adjustment` now inserts `adjustment_created` audit event in same transaction as row creation (`repo/src/payments/service.rs:420` to `repo/src/payments/service.rs:440`, action at `repo/src/payments/service.rs:444`).

5. **Coverage gaps (reconciliation success/dedupe/multipart + requirement-fit paths)**  
   - **Status:** Fixed  
   - **Evidence:** dedicated reconciliation integration tests cover success, duplicate checksum conflict, empty multipart, and oversize upload in `repo/tests/reconciliation_integration.rs:103`, `repo/tests/reconciliation_integration.rs:163`, `repo/tests/reconciliation_integration.rs:216`, `repo/tests/reconciliation_integration.rs:260`; explicit tier-recalculation audit assertion now exists in `repo/tests/high_risk.rs:1294` and validates `tier_recalculated` payload/hash fields at `repo/tests/high_risk.rs:1405`, `repo/tests/high_risk.rs:1418`, `repo/tests/high_risk.rs:1423`, `repo/tests/high_risk.rs:1433`.

## Notes
- This recheck is static-only and does not prove runtime behavior.
- If needed, next step is to run targeted tests: `cargo test --test reconciliation_integration` and `cargo test --test high_risk test_tier_recalculation_emits_audit_event`.
