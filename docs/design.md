# System Design — NexusOrder Desk

## Document Control

- Product: NexusOrder Desk
- Scope: Desktop + local service architecture and runtime design
- Audience: Engineering, reviewers, QA, operations
- Source of truth: current repository implementation

---

## 1. Purpose

This document describes how NexusOrder Desk is designed and operated as a local-first, enterprise-style order workflow platform. It explains:

- Runtime architecture and component boundaries
- Data and persistence strategy
- Security model and trust boundaries
- Consistency, recovery, and rollback behavior
- Operational concerns (jobs, observability, lifecycle)

The design goal is deterministic behavior under strict governance: role-based access, auditable mutations, recoverable multi-step workflows, and safe update/restore procedures.

---

## 2. System Context

NexusOrder Desk is deployed as two cooperating local components:

- Desktop shell (Electron)
- Local HTTPS service (Express + MongoDB)

The desktop process manages app lifecycle, user-facing windows, and integration responsibilities (service start, notification polling, update/rollback trigger path). The service process owns all business rules, persistence, and API contracts.

### 2.1 High-Level Responsibilities

- Desktop
  - Starts and supervises local backend service
  - Provides native window/tray lifecycle
  - Polls and bridges notifications to renderer
  - Performs startup health checks and triggers auto-rollback flow if required
- Service
  - Exposes `/api/*` domain APIs over localhost HTTPS
  - Enforces authn/authz + object-scope constraints
  - Orchestrates domain workflows (orders, approvals, fulfillment, payments, rules)
  - Persists data in MongoDB with startup index verification
  - Runs scheduled jobs (order state timers, backups)

---

## 3. Architectural Style

The backend follows a modular monolith design:

- Single deployable process
- Feature modules grouped by domain (`modules/*`)
- Shared middleware for cross-cutting concerns
- Shared package contracts (`shared-types`, `shared-rbac`, `shared-validation`, `shared-logging`)

Why this style:

- Strong consistency for tightly coupled business flows
- Lower operational overhead than microservices
- Easier static traceability for acceptance and audit review

---

## 4. Runtime Topology

### 4.1 Processes

- Electron main process
- Service Node.js process
- MongoDB process (local, required dependency)

### 4.2 Network/Transport

- Service endpoint: `https://127.0.0.1:4433`
- API prefix: `/api`
- Cookie session model for authenticated user actions
- Internal key header for machine-level update routes

### 4.3 Boot Sequence (Nominal)

1. Desktop main initializes lifecycle manager and service manager.
2. Service starts and loads configuration.
3. Service connects MongoDB and verifies/creates indexes.
4. Service executes startup recovery scanner for pending checkpoints.
5. Service starts schedulers/jobs.
6. Desktop verifies service health and initializes renderer windows.

If packaged startup detects post-update health failure, desktop invokes auto-rollback API flow.

---

## 5. Backend Composition

### 5.1 Cross-Cutting Layers

- Config: centralized env/config loading and validation
- Middleware:
  - Request correlation and structured logging
  - Security headers and CORS policy
  - Cookie parsing and session auth
  - RBAC permission checks
  - Centralized error handling
- Persistence:
  - Mongo connection lifecycle
  - Startup index orchestration
- Audit:
  - Event emission for sensitive domain actions

### 5.2 Domain Modules (Representative)

- Auth and session
- Users and blacklist
- Catalog and vendors
- Cart and checkout
- Orders, approvals, fulfillment
- Payment intents, reconciliation, refunds
- After-sales / RMA / reason codes
- Rules engine (authoring, versioning, simulation/conflict checks)
- Notifications
- Backup and restore
- Updates and rollback
- System and settings

Modules are routed from a central app composition point, keeping shared controls uniform while preserving domain isolation.

---

## 6. Data and Persistence Design

### 6.1 Storage Model

- Primary datastore: MongoDB
- Data grouped by domain collections
- Event-like collections for audit/recovery/rollback traces

### 6.2 Index Strategy

Indexes are established at startup by active persistence code to enforce:

- Uniqueness on critical identifiers (examples: usernames, order numbers, payment intent IDs)
- Query support for high-frequency filters (state, user, scoped fields, created/occurred timestamps)
- TTL behavior for ephemeral records (sessions, lockout windows where applicable)

The index phase is idempotent and executed before service is considered ready.

### 6.3 Durability and Integrity Patterns

- Versioned updates and constrained state transitions on domain entities
- Audit event emission for sensitive actions
- Checkpoint logs for critical multi-step mutations
- Restore flow checksum verification before data replacement

---

## 7. Domain State and Workflow Model

### 7.1 Order Lifecycle

Order state progression is controlled via explicit state machine logic. Invalid transitions are rejected with business-rule errors.

Design intent:

- Keep transition rules centralized
- Prevent ad-hoc state mutation from route handlers
- Support schedule-driven transitions (auto-cancel/auto-close)

### 7.2 After-Sales Lifecycle

After-sales/RMA transitions are separately modeled from primary order state, enabling controlled post-fulfillment resolution without collapsing primary workflow semantics.

### 7.3 Rule Engine Interaction

Rules are stored, versioned, and can be simulated/conflict-checked, supporting controlled policy rollout and auditability.

---

## 8. Security Architecture

### 8.1 Authentication

- Session cookie: `nexusorder_session`
- Login establishes session context (user ID, role, scope)
- Protected routes require valid session middleware

### 8.2 Authorization

Two layers are applied:

- RBAC permission gate (`requirePermission(...)`)
- Object-level scope enforcement in domain services (user/organization boundary checks)

### 8.3 Update Route Hardening

Update endpoints require internal key baseline. Interactive update actions additionally require an authenticated admin session, reducing risk from IPC-accessible machine credentials alone.

### 8.4 Crypto and Integrity

- Encrypted backup archives (`.zip.enc`)
- Hash-based checksum validation for backup restore and staged update apply
- Integrity gate requiring expected service entrypoint before update promotion

---

## 9. Recovery and Consistency Model

### 9.1 Checkpointing

Critical multi-step operations write `pending` checkpoints before mutation.

On success:

- Checkpoint marked `completed`

On error:

- Checkpoint marked `failed` with reason

### 9.2 Startup Recovery

On service startup, recovery scanner processes pending checkpoints and applies operation-specific compensation or failure marking.

Design outcomes:

- Reduces risk of silent partial commits after crash/interruption
- Produces explicit recovery/audit traces
- Preserves deterministic operator review path for non-auto-resolvable cases

---

## 10. Backup and Restore Design

### 10.1 Backup

- Enumerates DB collections
- Streams each collection as NDJSON into archive
- Encrypts archive and writes file to destination
- Records backup metadata/status/checksum in `backups`
- Emits backup audit event

### 10.2 Restore

- Validates backup existence/status
- Verifies encrypted file checksum
- Decrypts and rehydrates collections from NDJSON payloads
- Re-runs index setup after restore
- Uses checkpoint + restore event status tracking
- Emits restore audit event

This design prioritizes verifiability and post-incident traceability over minimal code path length.

---

## 11. Update and Rollback Design

### 11.1 Import

- Stores uploaded package in staged build path
- Computes and stores checksum + metadata

### 11.2 Apply

- Writes checkpoint
- Re-verifies staged checksum
- Extracts package to versioned directory
- Verifies expected runtime entrypoint
- Promotes build symlink (`current`/`previous`) only after checks pass
- Marks DB package status as `applied` after promotion succeeds

### 11.3 Rollback

- Manual rollback API and health-check-triggered auto-rollback are supported
- Symlink swap returns runtime to previous build
- Rollback event recorded
- Update package status marked `rolled_back`
- Audit event emitted

This avoids DB/runtime drift by ordering persistence transitions after filesystem promotion success.

---

## 12. Scheduling and Background Work

Service scheduler starts background jobs during startup. Core jobs include:

- Order scheduler operations (time-bound state actions)
- Backup scheduler operations

Job behavior is deterministic and tied to service lifecycle, simplifying deployment assumptions for single-node local runtime.

---

## 13. Observability and Diagnostics

### 13.1 Logging

- Structured logging with module-aware logger construction
- Correlation IDs propagated through request lifecycle

### 13.2 Health

- Health endpoints exposed under system module
- Used by desktop startup checks and operational diagnostics

### 13.3 Auditability

Sensitive actions emit auditable events (examples: backup created, restore performed, update imported/applied/rolled_back, recovery actions).

---

## 14. Desktop Design Notes

Desktop main process is intentionally thin in business logic and acts as orchestration shell:

- Window/tray lifecycle manager
- Service process control and readiness checks
- Notification bridge/poller integration
- Startup failure mitigation via update auto-rollback trigger

Renderer clients consume service APIs and local bridges; authoritative validation and policy enforcement remain in service layer.

---

## 15. Failure Modes and Mitigations

### 15.1 Service Crash Mid-Operation

Mitigation: checkpoint + startup recovery scanner.

### 15.2 Corrupt/Modified Backup File

Mitigation: checksum mismatch blocks restore.

### 15.3 Corrupt/Incomplete Update Payload

Mitigation: checksum and extracted entrypoint integrity gates block apply.

### 15.4 Post-Update Startup Regression

Mitigation: desktop-initiated auto-rollback + rollback event recording.

### 15.5 Unauthorized Sensitive Action Attempt

Mitigation: auth middleware + RBAC + object-scope checks + internal key constraints (updates).

---

## 16. Testing and Verification Strategy (Design-Level)

The repository includes unit and integration test structures for service domains. Design verification targets:

- Route authn/authz enforcement
- Scope/permission boundary behavior
- State-machine transition validity
- Recovery and rollback critical flows
- Backup/restore/update integrity checks

Given safety-critical paths, mutation-heavy modules should keep integration coverage as primary confidence source.

---

## 17. Trade-offs

- Modular monolith vs distributed services
  - Chosen for stronger local consistency and simpler operations
- Local process orchestration vs external platform orchestration
  - Chosen for desktop-first deployment constraints
- Rich recovery/audit instrumentation vs implementation complexity
  - Chosen for governance and forensic traceability requirements

---

## 18. Future Design Considerations

- Hardened key/certificate lifecycle automation for production packaging
- More explicit idempotency keys for externally retried commands
- Extended recovery playbooks for complex cross-collection compensations
- Broader chaos-style fault injection around update/restore paths

---

## 19. Summary

NexusOrder Desk is designed as a governed local platform: secure-by-default transport and session model, explicit authorization controls, deterministic domain state handling, and robust operational safeguards (checkpoint recovery, encrypted backups with checksum verification, and controlled update rollback). The architecture intentionally prioritizes safety, auditability, and predictable recovery behavior.
