# Fix Verification Report (Static)

Source issues: `.tmp/delivery_architecture_static_audit_report_codex.md` (Issues / Suggestions section).

Verification boundary:
- Static-only checks against current `repo/` contents.
- No runtime execution, no tests run.

Date: 2026-04-15

## Summary
- Issues checked: 5
- Fixed: 5
- Not fixed: 0

## Verification Results

### 1) High — Security traceability references non-existent authorization file
- Status: **Fixed**
- What changed: Documentation no longer references the non-existent `src/auth/policy.rs`; it points to the implemented helper in `src/users/policy.rs`.
- Evidence:
  - `repo/DELIVERY_PLAN.md:39`
  - `repo/DELIVERY_PLAN.md:373`
  - `repo/DELIVERY_PLAN.md:520`
  - `repo/DELIVERY_PLAN.md:581`
  - `repo/DELIVERY_PLAN.md:589`

### 2) High — API matrix incomplete for finance approval route
- Status: **Fixed**
- What changed: `PATCH /payments/adjustments/{id}/approve` is now present in the API matrix.
- Evidence:
  - `repo/docs/api-matrix.md:82`
  - Route still exists in code: `repo/src/payments/handler.rs:181`

### 3) Medium — Checklist claimed “idempotency middleware” though it was handler/service-level
- Status: **Fixed**
- What changed: Reviewer checklist now accurately describes handler/service-level idempotency controls and explicitly notes it is not Actix middleware.
- Evidence:
  - `repo/docs/reviewer-checklist.md:42`

### 4) Medium — `tests/booking_lifecycle.rs` labeled “integration” but was unit-only
- Status: **Fixed**
- What changed: The test file now clearly states it is unit-scoped and enumerates what is explicitly out of scope (HTTP/DB/inventory side effects).
- Evidence:
  - `repo/tests/booking_lifecycle.rs:3`
  - `repo/tests/booking_lifecycle.rs:12`

### 5) Medium — Asset attachment size limit hardcoded instead of config-driven
- Status: **Fixed**
- What changed: Attachment upload size limit is now driven by `cfg.storage.max_upload_bytes` passed from handler to service.
- Evidence:
  - Handler passes config limit: `repo/src/assets/handler.rs:205`
  - Service enforces passed limit (no 10MB hardcode): `repo/src/assets/service.rs:249`
  - Config field exists: `repo/src/config/mod.rs:88`

## Notes
- This verification only confirms the previously reported *static* issues are resolved by current file contents. Runtime behavior and end-to-end correctness remain **Manual Verification Required** under the static-only boundary.

