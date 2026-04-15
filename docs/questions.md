# Business Logic Questions — NexusOrder Desk

This document records non-obvious implementation decisions in the current codebase.
Each entry follows the format:

- Question: What was unclear or required a choice
- Assumption: The constraint or principle that drove the decision
- Solution: What was implemented and why

## Authentication and Sessions

### Q1: Should login check lockout before validating username and password?
Question: If lockout is checked only after password verification, repeated attempts still perform unnecessary credential work and reveal timing differences.

Assumption: Lockout should be a front-door guard and should run before any credential path.

Solution: Login checks lockout state first. If locked, it emits an auth.lockout audit event and returns 403 with lockout expiry context.

### Q2: Should failed login records be reset by time window logic or removed on success?
Question: Keeping stale failed-attempt records after successful login can carry old risk state forward.

Assumption: A successful authentication should clear prior failure history for that username.

Solution: On successful login, failed_login records are deleted. This resets the attempt counter immediately rather than waiting for TTL cleanup.

### Q3: Should session validity rely only on JWT expiry, or also check server-side session storage?
Question: JWT-only validation cannot enforce server-side revocation (logout, password change global sign-out).

Assumption: Sessions must be revocable centrally.

Solution: Session validation verifies JWT signature and expiry, then requires a matching, unexpired session row in MongoDB. Missing/expired DB session returns unauthorized.

### Q4: Should the service start with fallback secrets in production-like runs?
Question: Convenience defaults for secrets can accidentally become production behavior.

Assumption: Security-sensitive keys must be explicit and mandatory.

Solution: Config loading requires explicit values for SESSION_SECRET, FIELD_ENCRYPTION_KEY, BACKUP_ENCRYPTION_KEY, and INTERNAL_API_KEY. Startup fails if missing.

### Q5: Should password change keep active sessions alive?
Question: If old sessions remain valid after a password change, account compromise may persist.

Assumption: Password change is a trust reset event.

Solution: Password change revokes all sessions for the user and forces re-login.

## Authorization and Scope

### Q6: Should RBAC alone decide access to order details?
Question: Role permission can allow endpoint access while still requiring object-level ownership/scope checks.

Assumption: Endpoint-level permission is necessary but not sufficient for data isolation.

Solution: Orders endpoints enforce both requirePermission and object-level canAccessOrder checks. Students are owner-only; advisor/mentor access is scope-bounded; admin bypasses scope.

### Q7: Should scoped-role access allow orders with missing scope snapshots?
Question: Missing snapshots can accidentally widen access if treated as permissive.

Assumption: Missing scope metadata is security-risky and must fail closed.

Solution: For scoped roles, missing order scope snapshot denies access by default.

### Q8: Should update endpoints trust internal API key as the only gate?
Question: Internal key may be available to machine components and indirectly reachable through desktop flows.

Assumption: Interactive update actions should require both machine trust and human admin identity.

Solution: All update routes require internal key, and interactive operations (import/apply/manual rollback) additionally require a department_admin session.

## Checkout and Orders

### Q9: Should checkout throttle run before blacklist checks?
Question: Ordering these checks changes user feedback and policy semantics.

Assumption: Blacklist policy should be surfaced first because it is deterministic and non-retryable.

Solution: Checkout applies policy checks in this order: blacklist first, throttle second.

### Q10: Should out-of-scope catalog items be blocked only when adding to cart?
Question: If scope validation exists only at cart time, later bypasses or stale carts can still produce invalid orders.

Assumption: Checkout must perform defense-in-depth scope validation.

Solution: Checkout revalidates every item against scope; on violation it emits checkout.scope_violation audit event and rejects.

### Q11: Should order creation be a single submitted insert or a draft-to-submitted transition?
Question: Directly inserting submitted state reduces events but loses state transition trace semantics.

Assumption: State changes should remain explicit and auditable.

Solution: Checkout persists order in draft, emits audit, then transitions to submitted with a second audit event.

### Q12: Should notification failures block checkout success?
Question: Notification delivery is helpful but not core order integrity.

Assumption: Business transaction must succeed even if notification channel fails.

Solution: Milestone notifications are best-effort and non-blocking. Errors are swallowed after order success.

### Q13: Should order listing for advisors/mentors query all orders then filter in memory?
Question: In-memory filtering increases exposure risk and load.

Assumption: Scope filtering should happen at repository query level wherever possible.

Solution: Role-based listing uses dedicated repository methods: student by owner, scoped roles by scope query.

## Split and Merge Mutations

### Q14: Should split/merge run without a write-ahead checkpoint?
Question: Multi-step mutations can leave partial state on crash.

Assumption: Critical order mutations need crash-recovery breadcrumbs.

Solution: Both split and merge create pending checkpoints before mutation, then mark completed or failed.

### Q15: Should split allow moving all items out of parent order?
Question: Moving all items makes the original order empty and semantically invalid.

Assumption: Split must preserve meaningful parent and child orders.

Solution: Split rejects empty-result parent with SPLIT_ALL_ITEMS.

### Q16: Should merge overwrite source order history or preserve audit lineage?
Question: Hard replacement of source records can hide historical context.

Assumption: Merge should preserve provenance and explainability.

Solution: Merge creates a new merged order, copies notes/tags/items, and cancels source orders. Audit metadata stores sourceOrderIds.

### Q17: Should split/merge tax recomputation reuse prior totals?
Question: Reusing totals after item movement can create financial drift.

Assumption: Tax lines must be recomputed from actual moved/remaining items.

Solution: Split and merge recalculate subtotal and tax lines from item-level values using shared invariants helpers.

## Reconciliation and Refunds

### Q18: Should reconciliation accept valid rows and skip invalid signatures?
Question: Partial acceptance can let tampered files enter the system as mixed batches.

Assumption: Signature integrity is all-or-nothing per import file.

Solution: Reconciliation pre-validates every row signature before persisting rows. Any invalid signature fails the whole import.

### Q19: Should duplicate payment_intent rows rely only on app-level checks?
Question: Concurrent imports can race and bypass in-memory duplicate detection.

Assumption: Duplicate protection needs database-backed enforcement.

Solution: Unique index on reconciliation rows plus duplicate handling for race-time insert conflicts (code 11000).

### Q20: Should reconciliation always force order state to paid when a payment matches?
Question: If order already advanced manually, forced transition can violate state machine.

Assumption: Reconciliation transitions should be conditional and state-safe.

Solution: Matched intents advance order only from approved to paid, guarded by transition checks; otherwise they log warning and continue.

### Q21: Should exception repair allow empty admin notes?
Question: Repair without rationale weakens audit quality and future investigations.

Assumption: Manual exception handling must be explainable.

Solution: repairException requires non-empty admin note and records actor and timestamp.

### Q22: Should refunds trust client-provided order and payment relation?
Question: A mismatched paymentIntent and orderId can misapply money movements.

Assumption: Refund linkage must be verified server-side.

Solution: Refund creation validates intent.orderId matches request orderId and rejects mismatches.

### Q23: Should refund amount checks validate only this request amount?
Question: Multiple partial refunds can exceed paid amount cumulatively.

Assumption: Cap enforcement must consider historical refunds.

Solution: Refund service sums existing refunds for the payment intent and rejects if cumulative total exceeds paid amount.

## Recovery, Health, and Jobs

### Q24: Should startup proceed before pending checkpoints are scanned?
Question: Accepting traffic before recovery can compound inconsistency.

Assumption: Recovery should run before normal operations begin.

Solution: Startup runs recovery scan early; pending checkpoints are compensated or marked failed according to operation type.

### Q25: Should unknown checkpoint operation types be ignored?
Question: Silent ignore hides incompatible or partially deployed logic.

Assumption: Unknown operations must be visible and terminally marked.

Solution: Unknown checkpoint types are marked failed with recovery note for manual review.

### Q26: Should health status report only binary up/down?
Question: Binary status hides operational debt such as pending recovery work.

Assumption: Health should expose degraded state when service is up but not fully clean.

Solution: Startup health reports degraded when pending checkpoints exist, unhealthy when DB ping fails, and includes checkpointRecoveryPending flag.

### Q27: Should backup scheduling logic live outside central registry?
Question: Distributed job registration risks orphan timers and non-uniform teardown.

Assumption: Recurring jobs need centralized lifecycle management.

Solution: jobScheduler registers all jobs in one registry and provides explicit start/stop behavior.

## Backup and Restore

### Q28: Should backup output remain plain zip on disk?
Question: Plain archives expose data if filesystem is accessed.

Assumption: Backups must be encrypted at artifact level, not only transport-level.

Solution: Backup creates zip content, encrypts buffer, writes .zip.enc, and stores checksum/metadata in backups collection.

### Q29: Should restore start writing data before verifying checksum?
Question: Restoring from tampered/corrupt artifacts can poison primary data.

Assumption: Integrity verification must occur before any destructive restore step.

Solution: Restore verifies checksum first, then decrypts, then replaces collection contents.

### Q30: Should restore rebuild indexes after data rehydration?
Question: Reinserted collections may not preserve index guarantees automatically.

Assumption: Post-restore index consistency is required before declaring success.

Solution: Restore reruns index orchestration after collection rehydration.

## Updates and Rollback

### Q31: Should update status be marked applied before filesystem promotion?
Question: DB-first status updates can drift from actual runtime if promotion later fails.

Assumption: Persistence status must reflect real deployed state.

Solution: Apply flow performs checksum verification, extraction, entrypoint integrity check, and symlink promotion first; only then marks package status as applied.

### Q32: Should auto-rollback require an authenticated user session?
Question: Startup health failures can happen before any user logs in.

Assumption: Recovery path must work headlessly during boot.

Solution: Auto-rollback endpoint accepts internal-key flow without admin session and records actor identity as internal system actor.

### Q33: Should rollback rely exclusively on service availability?
Question: If service is down, API-based rollback cannot execute.

Assumption: Desktop must retain emergency local fallback to recover from bad updates.

Solution: Desktop first calls service auto-rollback API; if unreachable, it performs local current/previous symlink swap fallback.

### Q34: Should packaged desktop accept any localhost TLS cert?
Question: Trusting any localhost cert in production opens local MITM risk.

Assumption: Packaged builds should fail closed unless cert pinning is valid.

Solution: Desktop pins service cert fingerprint in packaged mode. Missing/mismatched fingerprint rejects connections.

### Q35: Should main-process local HTTP calls globally disable TLS verification?
Question: Global TLS disable creates broad security blind spots.

Assumption: Local service trust should be narrowed to pinned trust anchor.

Solution: localFetch uses CA-pinned agents via configureTrustAnchor. Dev mode relaxes TLS only for local development.

## Operational and Audit Decisions

### Q36: Should audit events be optional for security-sensitive operations?
Question: Missing audit records undermine traceability for regulated workflows.

Assumption: Authentication, reconciliation, backup/restore, and update transitions require durable activity trace.

Solution: Services emit explicit audit events for login outcomes, order lifecycle milestones, reconciliation actions, refund initiation, backup/restore, and update/rollback operations.

### Q37: Should startup in desktop continue when service health check fails?
Question: Opening UI against unhealthy backend causes undefined operator behavior.

Assumption: Startup must be fail-closed under unhealthy service conditions.

Solution: Desktop startup quits the app after failed health check and triggers rollback flow before exit.

### Q38: Should internal API key be optional in development?
Question: Optional key behavior can leak into packaged environments and weaken boundaries.

Assumption: Internal transport guard should always exist regardless of mode.

Solution: INTERNAL_API_KEY is mandatory in service config. Internal middleware always enforces x-internal-key equality.

### Q39: Should scheduled backups run silently without event context?
Question: Unattributed scheduled operations reduce accountability.

Assumption: Scheduled and manual operations must be distinguishable in records.

Solution: Backup records include triggeredBy mode and optional user IDs, allowing explicit trace of system-initiated versus user-initiated backups.
