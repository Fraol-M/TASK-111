# API Route Matrix

All domain routes are under `/api/v1/`. The `/health` endpoint is at the application root (outside `/api/v1`).

| Method | Path | Role(s) Required | Idempotency | Notes |
|--------|------|-----------------|-----------------|-------|
| POST | /auth/login | Public | — | Returns JWT token |
| POST | /auth/logout | AuthUser | — | Revokes session |
| GET | /auth/me | AuthUser | — | Returns current user |
| POST | /users | Administrator | — | Create user |
| GET | /users | Administrator | — | List users (paginated) |
| GET | /users/{id} | Administrator or self | — | Get user |
| PATCH | /users/{id} | Administrator | — | Update user |
| PATCH | /users/{id}/status | Administrator | — | Activate/suspend (audited) |
| POST | /users/{id}/password | Self or Administrator | — | Change password |
| GET | /members/{id} | Self or privileged | — | Get member profile |
| PATCH | /members/{id}/tier | Administrator | — | Force tier update (audited) |
| POST | /members/{id}/points | Admin/Ops | Required (header) | Earn/adjust points (audited) |
| POST | /members/{id}/wallet/topup | Admin/Finance | Required (header) | Top up wallet (audited) |
| POST | /members/{id}/redeem | Member (self) | — | Redeem points |
| POST | /members/{id}/blacklist | Administrator | — | Blacklist member (audited) |
| POST | /members/{id}/freeze | Administrator | — | Freeze redemption (audited) |
| GET | /members/{id}/points/ledger | Self or privileged | — | Points history |
| GET | /members/{id}/wallet/ledger | Self, Finance, Admin | — | Wallet history |
| GET | /members/{id}/preferences | Self | — | Get notification preferences |
| PATCH | /members/{id}/preferences | Self | — | Update preferences |
| POST | /inventory | Admin/Ops | — | Create inventory item |
| GET | /inventory | Authenticated | — | List items |
| GET | /inventory/{id} | Authenticated | — | Get item |
| PATCH | /inventory/{id} | Admin/Ops | — | Update item (optimistic lock) |
| POST | /inventory/{id}/restock | Admin/Ops | — | Manual restock |
| GET | /inventory/alerts | Admin/Ops | — | Restock alerts |
| PATCH | /inventory/alerts/{id}/ack | Admin/Ops | — | Acknowledge alert |
| POST | /inventory/pickup-points | Admin/Ops | — | Create pickup point (name, address, cutoff_hours) |
| GET | /inventory/pickup-points | Authenticated | — | List pickup points (cutoff_hours visible) |
| GET | /inventory/pickup-points/{id} | Authenticated | — | Get pickup point |
| PATCH | /inventory/pickup-points/{id} | Admin/Ops | — | Update pickup point (partial; `clear_cutoff=true` resets cutoff_hours to null) |
| DELETE | /inventory/pickup-points/{id} | Admin/Ops | — | Delete pickup point (409 if referenced by inventory items) |
| POST | /inventory/zones | Admin/Ops | — | Create delivery zone (name, description, cutoff_hours) |
| GET | /inventory/zones | Authenticated | — | List delivery zones (cutoff_hours visible) |
| GET | /inventory/zones/{id} | Authenticated | — | Get delivery zone |
| PATCH | /inventory/zones/{id} | Admin/Ops | — | Update delivery zone (partial; `clear_cutoff=true` resets cutoff_hours to null) |
| DELETE | /inventory/zones/{id} | Admin/Ops | — | Delete delivery zone (409 if referenced by inventory items) |
| POST | /bookings | Member | Required (header) | Create booking (Draft→Held or Draft→Confirmed depending on inventory strategy) |
| GET | /bookings | Member=own; Admin/Ops=all | — | List bookings |
| GET | /bookings/{id} | Owner or privileged | — | Get booking |
| PATCH | /bookings/{id}/confirm | Member or Ops | — | Held→Confirmed |
| PATCH | /bookings/{id}/cancel | Member or Ops | — | Cancel booking |
| PATCH | /bookings/{id}/change | Member or Ops | — | Change booking |
| PATCH | /bookings/{id}/complete | Ops/Admin | — | Confirmed→Completed |
| PATCH | /bookings/{id}/exception | Ops/Admin | — | Flag exception |
| GET | /bookings/{id}/items | Owner or privileged | — | Booking line items |
| GET | /bookings/{id}/history | Ops/Admin | — | Status history |
| GET | /notifications | Member=own; Admin=all | — | Notification inbox |
| PATCH | /notifications/{id}/read | Self | — | Mark as read |
| GET | /notifications/templates | Ops/Admin | — | List templates |
| POST | /notifications/templates | Ops/Admin | — | Create template |
| PATCH | /notifications/templates/{id} | Ops/Admin | — | Update template |
| POST | /notifications/preview | Ops/Admin | — | Preview without persisting |
| POST | /groups | Ops/Admin | — | Create group thread |
| GET | /groups | Member=enrolled; Admin/Ops=all | — | List groups |
| GET | /groups/{id} | Member or Ops | — | Get group |
| POST | /groups/{id}/members | Ops/Admin | — | Add member to group |
| GET | /groups/{id}/members | Member or Ops | — | List members |
| DELETE | /groups/{id}/members/{uid} | Ops/Admin | — | Remove member |
| POST | /groups/{id}/messages | Thread member | — | Post message |
| GET | /groups/{id}/messages | Thread member | — | List messages |
| PATCH | /groups/{id}/messages/{mid}/read | Self | — | Mark message read |
| POST | /assets | AssetManager/Admin | — | Create asset |
| GET | /assets | Authenticated | — | List assets (cost masked) |
| GET | /assets/{id} | Authenticated | — | Get asset |
| PATCH | /assets/{id} | AssetManager/Admin | — | Update asset (optimistic lock + snapshot) |
| GET | /assets/{id}/versions | AssetManager/Admin | — | Version history |
| GET | /assets/{id}/versions/{v} | AssetManager/Admin | — | Specific version snapshot |
| POST | /assets/{id}/attachments | AssetManager/Admin | — | Upload attachment (multipart) |
| GET | /assets/{id}/attachments | AssetManager/Admin | — | List attachments |
| POST | /evaluation-cycles | Administrator | — | Create cycle |
| GET | /evaluation-cycles | Admin/Evaluator | — | List cycles |
| POST | /evaluations | Administrator | — | Create evaluation |
| GET | /evaluations/{id} | Admin/Evaluator | — | Get evaluation |
| PATCH | /evaluations/{id}/state | Administrator | — | Transition state |
| POST | /evaluations/{id}/assignments | Administrator | — | Assign evaluator |
| GET | /evaluations/{id}/assignments | Admin/Evaluator | — | List assignments |
| PATCH | /evaluations/{id}/assignments/{aid}/state | Evaluator=own; Admin=any | — | Transition assignment |
| POST | /evaluations/{id}/assignments/{aid}/actions | Evaluator (own) | — | Add evaluation action |
| POST | /payments/intents | Finance/Admin | Required (body) | Create payment intent |
| GET | /payments/intents/{id} | Finance/Admin | — | Get intent |
| POST | /payments/intents/{id}/capture | Finance/Admin | Required (body) | Capture payment |
| POST | /payments/{id}/refunds | Finance/Admin | Required (body) | Request refund |
| PATCH | /payments/refunds/{id}/approve | Finance/Admin | — | Approve refund (audited) |
| POST | /payments/adjustments | Finance/Admin | — | Create adjustment (pending state, awaits approval) |
| PATCH | /payments/adjustments/{id}/approve | Finance/Admin | — | Approve pending adjustment (maker-checker; creator cannot self-approve; audited) |
| GET | /payments | Finance/Admin | — | List payments |
| POST | /reconciliation/import | Finance | — | Upload CSV (multipart) |
| GET | /reconciliation/imports | Finance/Admin | — | List imports |
| GET | /reconciliation/imports/{id} | Finance/Admin | — | Get import details |
| GET | /reconciliation/imports/{id}/rows | Finance/Admin | — | List reconciliation rows |
| GET | /audit/logs | Administrator | — | Audit log query |
| GET | /audit/logs/{id} | Administrator | — | Single audit entry |
| GET | /health (root) | Public | — | Health check (DB ping) — outside `/api/v1` |
