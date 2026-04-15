# API Specification — NexusOrder Desk

**Base URL:** `https://127.0.0.1:4433`  
**API prefix:** `/api`  
**Format:** `application/json` unless endpoint accepts multipart uploads.

---

## Table of Contents

1. [Authentication](#authentication)
2. [Request Headers](#request-headers)
3. [Response Envelope](#response-envelope)
4. [Error Codes](#error-codes)
5. [Roles & Permissions](#roles--permissions)
6. [Endpoints](#endpoints)
   - [System Health](#system-health)
   - [Auth](#auth)
   - [Users](#users)
   - [Catalog & Vendors](#catalog--vendors)
   - [Cart](#cart)
   - [Orders](#orders)
   - [Approvals](#approvals)
   - [Fulfillment](#fulfillment)
   - [Payment Intents](#payment-intents)
   - [Reconciliation](#reconciliation)
   - [Refunds](#refunds)
   - [RMA / After-Sales](#rma--after-sales)
   - [Reason Codes](#reason-codes)
   - [Rules Engine](#rules-engine)
   - [Notifications](#notifications)
   - [Search](#search)
   - [Audit](#audit)
   - [Backup & Restore](#backup--restore)
   - [Settings](#settings)
   - [Updates](#updates)
7. [Config Reference](#config-reference)

---

## Authentication

Primary auth model uses an HttpOnly cookie:

- Cookie name: `nexusorder_session`
- Issued by: `POST /api/auth/login`
- Transport: HTTPS localhost
- Cookie options: `httpOnly`, `sameSite=strict`, `secure` in production

Unauthenticated routes:

- `GET /api/system/health`
- `POST /api/auth/login`

Internal machine-to-service routes (updates) additionally require:

- `X-Internal-Key: <internal_api_key>`

For interactive update operations (`import`, `apply`, `rollback`), **both** are required:

- Valid `X-Internal-Key`
- Valid authenticated admin session cookie

---

## Request Headers

| Header | Required | Description |
|--------|----------|-------------|
| `Cookie` | Protected routes | Must include `nexusorder_session=<token>` |
| `Content-Type` | Requests with body | `application/json` or `multipart/form-data` |
| `X-Internal-Key` | Update routes | Required on all `/api/updates/*` endpoints |
| `X-Correlation-Id` | Optional | Request correlation/audit tracing |

---

## Response Envelope

### Success

```json
{
  "data": { }
}
```

Some list endpoints also include pagination metadata:

```json
{
  "data": [ ],
  "meta": {
    "page": 1,
    "pageSize": 50,
    "total": 123
  }
}
```

### Error

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Human-readable message",
    "details": {
      "issues": []
    }
  }
}
```

`details` may be omitted depending on error type.

---

## Error Codes

| HTTP | Code | Meaning |
|------|------|---------|
| 400 | `VALIDATION_ERROR` | Request schema/body/query validation failed |
| 400 | `REQUIRED` / `FILE_REQUIRED` | Required body field or file missing |
| 401 | `UNAUTHORIZED` / `AUTH_FAILED` | Missing or invalid session/internal key |
| 403 | `FORBIDDEN` / `AUTH_LOCKED` | Insufficient permission or account lockout |
| 404 | `NOT_FOUND` | Entity does not exist |
| 409 | `CONFLICT` | Duplicate or optimistic concurrency conflict |
| 422 | Business rule codes | Domain rule violation (state transition, throttle, scope, etc.) |
| 500 | `INTERNAL_ERROR` | Unhandled server error |

Common business-rule codes include:

- `CHECKOUT_THROTTLED`
- `USER_BLACKLISTED`
- `ITEM_OUT_OF_SCOPE`
- `ITEM_UNAVAILABLE`
- `INVALID_STATE_TRANSITION`
- `INVALID_SIGNATURE`
- `REFUND_ORDER_MISMATCH`
- `PAYMENT_NOT_REFUNDABLE`
- `REFUND_EXCEEDS_PAID`

---

## Roles & Permissions

### Roles

| Role | Description |
|------|-------------|
| `student` | Browse catalog, manage cart, place requests, create RMAs |
| `faculty_advisor` | Review scoped orders, approve/deny |
| `corporate_mentor` | Confirm delivery for scoped orders |
| `department_admin` | Full admin operations (catalog, rules, payments, users, backups, updates) |

### High-level Capability Summary

| Capability | student | faculty_advisor | corporate_mentor | department_admin |
|-----------|:---:|:---:|:---:|:---:|
| Catalog read | ✓ | ✓ | ✓ | ✓ |
| Cart create/update/delete | ✓ | | | |
| Checkout | ✓ | | | |
| Orders read | ✓ | ✓ | ✓ | ✓ |
| Approve/deny | | ✓ | | ✓ |
| Confirm delivery | | | ✓ | ✓ |
| Reconciliation import/repair | | | | ✓ |
| Refund create/read | | | | ✓ |
| Rules create/update/manage | | | | ✓ |
| Users manage | | | | ✓ |
| Backup/restore | | | | ✓ |
| Update apply/rollback | | | | ✓ |

### Object-level Rules (service-layer)

- Students can access only their own orders.
- Advisors and mentors are scope-limited (`school`, `major`, `class`, `cohort`).
- Admins can access all scope-bound objects.
- Cart/checkout scope is enforced against catalog `eligibleScopes`.

---

## Endpoints

### System Health

#### `GET /api/system/health`

No authentication required.

**Response 200 / 503**

```json
{
  "data": {
    "status": "ok"
  }
}
```

#### `GET /api/system/health/details`

Requires admin permission `system:read`.

Returns detailed health diagnostics.

---

### Auth

#### `POST /api/auth/login`

No authentication required.

**Request body**

```json
{
  "username": "student1",
  "password": "Test@1234567"
}
```

**Response 200**

```json
{
  "data": {
    "user": {
      "id": "uuid",
      "username": "student1",
      "role": "student",
      "scope": {
        "school": "SCI"
      },
      "displayName": "student1"
    }
  }
}
```

#### `POST /api/auth/logout`

Requires authentication.

#### `GET /api/auth/session`

Requires authentication.

#### `POST /api/auth/change-password`

Requires authentication.

**Request body**

```json
{
  "currentPassword": "OldP@ssw0rd1!",
  "newPassword": "NewP@ssw0rd2!"
}
```

---

### Users

#### `GET /api/users`

Requires `users:manage_users`.

#### `POST /api/users`

Requires `users:manage_users`.

#### `POST /api/users/:id/blacklist`

Requires `blacklists:create`.

#### `DELETE /api/users/:id/blacklist`

Requires `blacklists:delete`.

#### `POST /api/users/consent/fingerprint`

Requires authentication.

**Request body**

```json
{ "consentGiven": true }
```

#### `POST /api/users/fingerprint`

Requires authentication.

**Request body**

```json
{ "fingerprintHash": "<sha256-hex>" }
```

Returns `403 CONSENT_REQUIRED` if consent has not been granted.

---

### Catalog & Vendors

#### `GET /api/catalog`

Requires `catalog:read`. Scope-filtered.

Optional query:

- `q` full-text search string

#### `GET /api/catalog/:id`

Requires `catalog:read`. Scope-filtered.

#### `POST /api/catalog`

Requires `catalog:manage_catalog`.

#### `PATCH /api/catalog/:id`

Requires `catalog:manage_catalog`.

#### `GET /api/vendors`

Requires `vendors:read`.

#### `POST /api/vendors`

Requires `vendors:create`.

---

### Cart

#### `GET /api/carts/active`

Requires authentication.

#### `GET /api/carts/me`

Requires authentication.

#### `POST /api/carts/items`

Requires `cart:create`.

**Request body**

```json
{
  "catalogItemId": "uuid",
  "quantity": 2
}
```

Notes:

- Validates availability.
- Enforces scope eligibility (`ITEM_OUT_OF_SCOPE` if blocked).

#### `DELETE /api/carts/items/:catalogItemId`

Requires `cart:delete`.

#### `POST /api/carts/checkout`

Requires `orders:checkout`.

Notes:

- Blacklist enforcement.
- Throttle enforcement (default max 10 attempts / 10 minutes).
- Scope re-checked in checkout (defense-in-depth).

---

### Orders

#### `GET /api/orders`

Requires `orders:read`.

- Student: own orders
- Advisor/Mentor: scope-filtered
- Admin: all

#### `GET /api/orders/:id`

Requires `orders:read` + object-level authorization.

#### `POST /api/orders/:id/notes`

Requires `order_notes:create` + object-level authorization.

#### `POST /api/orders/:id/tags`

Requires `order_tags:create` + object-level authorization.

---

### Approvals

#### `GET /api/approvals/pending`

Requires `approvals:approve`.

#### `POST /api/approvals/:orderId/approve`

Requires `approvals:approve`.

#### `POST /api/approvals/:orderId/reject`

Requires `approvals:approve`.

#### `POST /api/approvals/:orderId/decide`

Requires `approvals:approve`.

---

### Fulfillment

#### `POST /api/fulfillment/:orderId/allocate`

Requires `orders:admin`.

#### `POST /api/fulfillment/:orderId/ship`

Requires `orders:admin`.

#### `POST /api/fulfillment/:orderId/confirm-delivery`

Requires `fulfillment:confirm_receipt`.

Mentor scope checks are enforced at object level.

---

### Payment Intents

#### `POST /api/payments/intents`

Requires `payment_intents:read`.

**Request body**

```json
{ "orderId": "uuid" }
```

#### `GET /api/payments/intents/:id`

Requires `payment_intents:read`.

#### `POST /api/payments/intents/:id/confirm`

Requires `payment_intents:confirm`.

**Request body**

```json
{ "paymentReference": "WECHAT_REF_20260414_001" }
```

---

### Reconciliation

#### `GET /api/payments/reconciliation`

Requires `reconciliation:reconcile`.

#### `POST /api/payments/reconciliation/import`

Requires `reconciliation:reconcile`.

`multipart/form-data` with field `file` (CSV).

Behavior:

- CSV schema validation
- Per-row RSA signature verification
- Full import rejection on first invalid signature
- Duplicate payment intent detection/idempotency marking

#### `POST /api/payments/reconciliation/flag-unreconciled`

Requires `reconciliation:repair_exception`.

**Request body**

```json
{
  "paymentIntentId": "pi-...",
  "note": "Missing from merchant batch"
}
```

#### `POST /api/payments/reconciliation/repair`

Requires `reconciliation:repair_exception`.

**Request body**

```json
{
  "paymentIntentId": "pi-...",
  "note": "Manually reconciled by admin"
}
```

---

### Refunds

#### `POST /api/refunds`

Requires `refunds:create`.

**Request body**

```json
{
  "orderId": "uuid",
  "paymentIntentId": "uuid",
  "amount": 50,
  "currency": "CNY",
  "reason": "Customer return",
  "reasonCode": "RETURN"
}
```

Validation includes:

- payment intent exists
- intent belongs to provided `orderId`
- payment status is refundable
- cumulative refunds do not exceed paid amount

#### `GET /api/refunds/order/:orderId`

Requires `refunds:read`.

---

### RMA / After-Sales

#### `POST /api/rma/orders/:orderId`

Requires `rma:create`.

#### `POST /api/rma/:rmaId/approve`

Requires `rma:approve`.

#### `POST /api/rma/orders/:orderId/split`

Requires `orders:split`.

#### `POST /api/rma/orders/merge`

Requires `orders:merge`.

---

### Reason Codes

#### `GET /api/reason-codes`

Requires authentication.

#### `POST /api/reason-codes`

Requires `after_sales:manage_reason_codes`.

#### `PATCH /api/reason-codes/:id`

Requires `after_sales:manage_reason_codes`.

---

### Rules Engine

#### `GET /api/rules`

Requires `rules:read`.

#### `GET /api/rules/conflicts`

Requires `rules:read`.

#### `GET /api/rules/conflicts/all`

Requires `rules:read`.

#### `POST /api/rules`

Requires `rules:create`.

#### `POST /api/rules/simulations`

Requires `rule_simulations:create`.

#### `GET /api/rules/:id`

Requires `rules:read`.

#### `PATCH /api/rules/:id`

Requires `rules:update`.

#### `POST /api/rules/:id/activate`

Requires `rules:update`.

#### `POST /api/rules/:id/deactivate`

Requires `rules:update`.

---

### Notifications

#### `GET /api/notifications`

Requires `notifications:read`.

Optional query:

- `unread=true`

#### `POST /api/notifications/:id/read`

Requires `notifications:read`.

#### `PUT /api/notifications/preferences`

Requires `notifications:read`.

**Request body**

```json
{
  "milestone": "order_approved",
  "onScreen": true
}
```

---

### Search

#### `GET /api/search`

Requires authentication (`orders:read` gate + role-specific branching).

Query:

- `q` (required, min practical length 2)
- `type` optional (`orders`, `rules`, `users`)

---

### Audit

#### `GET /api/audits`

Requires `audit:view_audit`.

Query params:

- `targetId`
- `action`
- `userId`
- `page` (default 1)
- `pageSize` (default 50, max 100)

---

### Backup & Restore

#### `GET /api/backups`

Requires `backups:backup`.

#### `POST /api/backups`

Requires `backups:backup`.

Optional body:

```json
{ "destinationPath": "C:/Backups/NexusOrder" }
```

#### `GET /api/backups/:id`

Requires `backups:backup`.

#### `POST /api/restore`

Requires `restore:restore`.

**Request body**

```json
{ "backupId": "uuid" }
```

---

### Settings

#### `GET /api/settings`

Requires `backups:backup`.

#### `GET /api/settings/backup-destination`

Requires `backups:backup`.

#### `PUT /api/settings/backup-destination`

Requires `backups:backup`.

**Request body**

```json
{ "destinationPath": "D:/NexusOrderBackups" }
```

---

### Updates

All update routes require `X-Internal-Key`.

#### `POST /api/updates/import`

Requires internal key + admin session.

`multipart/form-data` with:

- `package` (file)
- `version` (string)

#### `POST /api/updates/:packageId/apply`

Requires internal key + admin session.

#### `POST /api/updates/rollback`

Requires internal key + admin session.

**Request body**

```json
{
  "updatePackageId": "uuid",
  "reason": "Manual rollback"
}
```

#### `POST /api/updates/auto-rollback`

Requires internal key only.

Used by startup coordinator when health check fails before user login.

---

## Config Reference

| Setting | Env Variable | Default | Notes |
|---------|-------------|---------|-------|
| Service port | `SERVICE_PORT` | `4433` | Local HTTPS service port |
| Service host | `SERVICE_HOST` | `127.0.0.1` | Local bind address |
| MongoDB URI | `MONGODB_URI` | `mongodb://localhost:27017/nexusorder` | Local DB |
| Session TTL (seconds) | `SESSION_TTL_SECONDS` | `28800` | Session lifetime |
| Auth max failed attempts | `AUTH_MAX_FAILED_ATTEMPTS` | `5` | Lockout threshold |
| Auth lockout minutes | `AUTH_LOCKOUT_MINUTES` | `15` | Lockout duration |
| Checkout max attempts | `CHECKOUT_MAX_ATTEMPTS` | `10` | Sliding-window throttle |
| Checkout window minutes | `CHECKOUT_WINDOW_MINUTES` | `10` | Sliding-window duration |
| Auto-cancel minutes | `ORDER_AUTO_CANCEL_MINUTES` | `30` | Unpaid order cancel timer |
| Auto-close days | `ORDER_AUTO_CLOSE_DAYS` | `14` | Delivered order close timer |
| Backup destination | `BACKUP_DESTINATION_PATH` | `./backups` | Backup output folder |
| Backup retention days | `BACKUP_RETENTION_DAYS` | `30` | Retention policy |
| Backup cron | `BACKUP_SCHEDULE_CRON` | `0 2 * * *` | Daily schedule |
| Reconciliation pubkey path | `WECHAT_MERCHANT_PUBLIC_KEY_PATH` | empty | RSA signature verification key |
| Internal API key | `INTERNAL_API_KEY` | required | Required for update routes |
| TLS cert path | `TLS_CERT_PATH` | empty | Required in production runtime |
| TLS key path | `TLS_KEY_PATH` | empty | Required in production runtime |

---

## Notes

- This specification reflects the current static implementation and route registration in the service app.
- Where runtime behavior depends on environment (TLS files, certificates, installer signing), manual verification is still required.
