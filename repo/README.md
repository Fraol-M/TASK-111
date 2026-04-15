# Venue Booking & Operations Management System

**Project type:** backend
**Stack:** Rust · Actix-web 4 · Diesel 2 · PostgreSQL · Docker Compose

Single-node, offline-capable backend for venue booking, inventory holds, loyalty
(points + wallet + tier), notifications with DND, groups, assets with
versioning, evaluations, payments/refunds/adjustments, reconciliation imports,
and tamper-evident audit trails.

---

## Startup

```bash
docker-compose up
# (equivalent modern form: docker compose up --build)
```

That single command builds and starts the app and PostgreSQL containers, runs
all migrations at startup, and begins serving on port `8080`. No manual DB
setup, no language-level installs, no host-side dependencies beyond Docker.

## Access

- Base URL: **http://localhost:8080**
- Health endpoint: **http://localhost:8080/health**
- API prefix: **http://localhost:8080/api/v1**

## Verification

### 1. Health check (unauthenticated)

```bash
curl http://localhost:8080/health
# {"status":"ok","db":"ok"}
```

### 2. Log in (returns JWT)

```bash
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"Test1234!"}'
# {"token":"<jwt>","expires_at":"...","user_id":"...","role":"administrator"}
```

### 3. Call an authenticated endpoint

```bash
TOKEN="<paste token from step 2>"
curl -H "Authorization: Bearer $TOKEN" http://localhost:8080/api/v1/auth/me
# {"user_id":"...","username":"admin","role":"administrator"}
```

### 4. A typical business flow

```bash
# List inventory as any authenticated user
curl -H "Authorization: Bearer $TOKEN" http://localhost:8080/api/v1/inventory

# Query audit logs (administrator-only)
curl -H "Authorization: Bearer $TOKEN" "http://localhost:8080/api/v1/audit/logs?page=1&page_size=10"
```

---

## Demo credentials

All seven role-mapped demo accounts are **auto-created** on startup — no manual
SQL, no `docker compose exec`, no extra commands. The app's bootstrap phase
reads `APP__BOOTSTRAP__SEED_DEMO_USERS` (defaults to `true` in `.env.example`)
and, right after migrations, idempotently inserts each user with
`ON CONFLICT (username) DO NOTHING`. Restarting the stack never overwrites a
pre-existing user.

All demo accounts share the password **`Test1234!`** (configurable via
`APP__BOOTSTRAP__DEMO_PASSWORD`). Log in with whichever role matches the
feature you want to exercise:

| Username | Role | What they can do |
|----------|------|-----------------|
| `admin` | `administrator` | Full admin authority (audit logs, user management, all domains) |
| `ops` | `operations_manager` | Inventory, pickup points, zones, booking completion, notification templates, groups |
| `finance` | `finance` | Payment intents, refunds, adjustments, reconciliation imports |
| `asset_mgr` | `asset_manager` | Asset CRUD, attachments, version history |
| `evaluator` | `evaluator` | Own evaluation assignments / actions |
| `reviewer` | `reviewer` | Approve/reject completed evaluations |
| `member` | `member` | Create/manage own bookings, redeem points, manage own preferences |

The `member` demo user is provisioned with a matching `members` profile +
default `member_preferences` row, so member endpoints (`/members/{id}/...`)
work immediately.

**Production deployments MUST set** `APP__BOOTSTRAP__SEED_DEMO_USERS=false`.
Demo credentials are well-known and would be a privilege-escalation vector if
left on. Rotate `APP__JWT__SECRET` and `APP__ENCRYPTION__KEY_HEX` as well
before accepting real traffic.

---

## Role & permission summary

See `docs/permission-matrix.md` for the full per-route table. Quick reference:

- **Any authenticated user** — read inventory/pickup/zones, own bookings, own notifications
- **`member`** — create/confirm/cancel/change own bookings, redeem points, manage own preferences
- **`operations_manager`** — inventory/pickup/zone CRUD, booking complete/exception, notification templates, groups
- **`finance`** — payment intents/captures, refunds, adjustments (maker-checker), reconciliation imports
- **`asset_manager`** — asset CRUD, attachments, version snapshots
- **`evaluator`** — act only on own evaluation assignments (`evaluator_id == claims.sub`)
- **`reviewer`** — approve/reject completed evaluations (narrower than evaluator)
- **`administrator`** — superset of every role, plus audit log read

All mutating finance and governance actions write to a tamper-evident hash-chained `audit_logs` table that is append-only at the database layer.

---

## Running tests

```bash
# Bring up an isolated test DB + run the full Rust test suite
docker compose -f docker-compose.test.yml up --build --abort-on-container-exit

# Or with the helper script
./run_tests.sh
```

Test suites live under `tests/`:

- `auth_flow.rs` — login → logout → 401 round-trip
- `high_risk.rs` — RBAC denials, refund cap, maker-checker, tier-recalc audit, cross-thread receipt guards, asset cost masking
- `reconciliation_integration.rs` — import success, duplicate checksum (409), empty multipart (422), oversize upload (422)
- `api_*.rs` — per-domain HTTP endpoint coverage (health, users, members, bookings, inventory, notifications, groups, assets, evaluations, payments, reconciliation)
- `booking_lifecycle.rs` — booking state machine unit tests
- `asset_versioning.rs` — before-image snapshots, cost masking, evaluation scope
- `notification_dnd.rs` — template render validation + DND queue lifecycle
- `idempotency.rs` — hash primitives

---

## Documentation

| Document | Purpose |
|---|---|
| `api-spec.md` | User-facing API specification (endpoints, auth, envelopes, errors, config) |
| `design.md` | System design, domain state, security, recovery, background jobs |
| `questions.md` | Q/Assumption/Solution decisions for non-obvious implementation choices |
| `docs/api-matrix.md` | Full route inventory with auth requirements |
| `docs/permission-matrix.md` | Per-role access table for every route |
| `docs/reviewer-checklist.md` | Static audit traceability checklist |
| `DELIVERY_PLAN.md` | Architecture, schema, jobs, and scenario guide |

## Key domains

| Domain | Entry point |
|---|---|
| Auth & sessions | `src/auth/` |
| Users & roles | `src/users/` |
| Members (points, wallet, tier) | `src/members/` |
| Bookings & inventory holds | `src/bookings/`, `src/inventory/` |
| Payments & refunds | `src/payments/` |
| Notifications | `src/notifications/` |
| Assets & evaluations | `src/assets/`, `src/evaluations/` |
| Audit logs | `src/audit/` |

---

## Environment

Copy `.env.example` to `.env` and fill in values. All `APP__*` sections
(server, jobs, booking, payment, dnd, backup, storage, notifications) are
required. Only these secrets need to be generated before first run:

```
DATABASE_URL=postgres://venue:venue@db:5432/venue_ops
APP__JWT__SECRET=<generate with: openssl rand -base64 48>
APP__ENCRYPTION__KEY_HEX=<generate with: openssl rand -hex 32>
```

See `.env.example` for the full list with defaults or `DELIVERY_PLAN.md §12` for descriptions.
