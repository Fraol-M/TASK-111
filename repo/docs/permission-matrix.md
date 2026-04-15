# Permission Matrix

All routes require a valid JWT Bearer token unless marked **Public**.  
`self` = authenticated user ID must match the resource owner.

| Route | Method | Member | Finance | OpsManager | AssetManager | Evaluator | Administrator |
|-------|--------|--------|---------|------------|--------------|-----------|---------------|
| `/auth/login` | POST | Public | Public | Public | Public | Public | Public |
| `/auth/logout` | POST | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `/auth/me` | GET | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `/users` | POST | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ |
| `/users` | GET | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ |
| `/users/{id}` | GET | self | ✗ | ✗ | ✗ | ✗ | ✓ |
| `/users/{id}` | PATCH | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ |
| `/users/{id}/status` | PATCH | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ |
| `/users/{id}/password` | POST | self | ✗ | ✗ | ✗ | ✗ | ✓ |
| `/members/{id}` | GET | self | ✓ | ✓ | ✗ | ✗ | ✓ |
| `/members/{id}/points` | POST | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/members/{id}/redeem` | POST | self | ✗ | ✗ | ✗ | ✗ | ✗ |
| `/members/{id}/points/ledger` | GET | self | ✓ | ✓ | ✗ | ✗ | ✓ |
| `/members/{id}/wallet/topup` | POST | ✗ | ✓ | ✗ | ✗ | ✗ | ✓ |
| `/members/{id}/wallet/ledger` | GET | self | ✓ | ✗ | ✗ | ✗ | ✓ |
| `/members/{id}/freeze` | POST | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ |
| `/members/{id}/preferences` | GET | self | ✗ | ✗ | ✗ | ✗ | ✓ |
| `/members/{id}/preferences` | PATCH | self | ✗ | ✗ | ✗ | ✗ | ✓ |
| `/inventory` | POST | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/inventory` | GET | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `/inventory/{id}` | GET | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `/inventory/{id}` | PATCH | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/inventory/{id}/restock` | POST | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/inventory/alerts` | GET | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/inventory/alerts/{id}/ack` | PATCH | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/bookings` | POST | ✓ | ✗ | ✗ | ✗ | ✗ | ✓ |
| `/bookings` | GET | own | ✗ | all | ✗ | ✗ | all |
| `/bookings/{id}` | GET | own | ✓ | ✓ | ✗ | ✗ | ✓ |
| `/bookings/{id}/confirm` | PATCH | own | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/bookings/{id}/cancel` | PATCH | own | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/bookings/{id}/change` | PATCH | own | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/bookings/{id}/complete` | PATCH | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/bookings/{id}/exception` | PATCH | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/bookings/{id}/items` | GET | own | ✓ | ✓ | ✗ | ✗ | ✓ |
| `/bookings/{id}/history` | GET | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/notifications` | GET | own | ✗ | ✗ | ✗ | ✗ | all |
| `/notifications/{id}/read` | PATCH | own | ✗ | ✗ | ✗ | ✗ | ✗ |
| `/notifications/templates` | GET | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/notifications/templates` | POST | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/notifications/templates/{id}` | PATCH | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/notifications/preview` | POST | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/groups` | POST | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/groups` | GET | enrolled | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/groups/{id}` | GET | enrolled | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/groups/{id}/members` | GET | enrolled | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/groups/{id}/members` | POST | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/groups/{id}/members/{uid}` | DELETE | ✗ | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/groups/{id}/messages` | POST | enrolled | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/groups/{id}/messages` | GET | enrolled | ✗ | ✓ | ✗ | ✗ | ✓ |
| `/groups/{id}/messages/{mid}/read` | PATCH | own | ✗ | ✗ | ✗ | ✗ | ✓ |
| `/assets` | POST | ✗ | ✗ | ✗ | ✓ | ✗ | ✓ |
| `/assets` | GET | ✓* | ✓ | ✓ | ✓ | ✓ | ✓ |
| `/assets/{id}` | GET | ✓* | ✓ | ✓ | ✓ | ✓ | ✓ |
| `/assets/{id}` | PATCH | ✗ | ✗ | ✗ | ✓ | ✗ | ✓ |
| `/assets/{id}/versions` | GET | ✗ | ✗ | ✗ | ✓ | ✗ | ✓ |
| `/assets/{id}/attachments` | POST | ✗ | ✗ | ✗ | ✓ | ✗ | ✓ |
| `/assets/{id}/attachments` | GET | ✗ | ✗ | ✗ | ✓ | ✗ | ✓ |
| `/evaluation-cycles` | POST | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ |
| `/evaluation-cycles` | GET | ✗ | ✗ | ✗ | ✗ | ✓ | ✓ |
| `/evaluations` | POST | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ |
| `/evaluations/{id}` | GET | ✗ | ✗ | ✗ | ✗ | assigned | ✓ |
| `/evaluations/{id}/state` | PATCH | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ |
| `/evaluations/{id}/assignments` | POST | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ |
| `/evaluations/{id}/assignments` | GET | ✗ | ✗ | ✗ | ✗ | own | ✓ |
| `/evaluations/{id}/assignments/{aid}/state` | PATCH | ✗ | ✗ | ✗ | ✗ | own | ✓ |
| `/evaluations/{id}/assignments/{aid}/actions` | POST | ✗ | ✗ | ✗ | ✗ | own | ✓ |
| `/payments/intents` | POST | ✗ | ✓ | ✗ | ✗ | ✗ | ✓ |
| `/payments/intents/{id}` | GET | ✗ | ✓ | ✗ | ✗ | ✗ | ✓ |
| `/payments/intents/{id}/capture` | POST | ✗ | ✓ | ✗ | ✗ | ✗ | ✓ |
| `/payments/{id}/refunds` | POST | ✗ | ✓ | ✗ | ✗ | ✗ | ✓ |
| `/payments/refunds/{id}/approve` | PATCH | ✗ | ✓ | ✗ | ✗ | ✗ | ✓ |
| `/payments/adjustments` | POST | ✗ | ✓ | ✗ | ✗ | ✗ | ✓ |
| `/payments/adjustments/{id}/approve` | PATCH | ✗ | ✓ | ✗ | ✗ | ✗ | ✓ |
| `/payments` | GET | ✗ | ✓ | ✗ | ✗ | ✗ | ✓ |
| `/reconciliation/import` | POST | ✗ | ✓ | ✗ | ✗ | ✗ | ✓ |
| `/reconciliation/imports` | GET | ✗ | ✓ | ✗ | ✗ | ✗ | ✓ |
| `/reconciliation/imports/{id}` | GET | ✗ | ✓ | ✗ | ✗ | ✗ | ✓ |
| `/reconciliation/imports/{id}/rows` | GET | ✗ | ✓ | ✗ | ✗ | ✗ | ✓ |
| `/audit/logs` | GET | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ |
| `/audit/logs/{id}` | GET | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ |

**Notes:**  
- `✓*` = authenticated user can see assets but procurement_cost is masked (Finance/Admin see full value)  
- `own` = must match resource ownership (e.g., booking.member_id == claims.sub)  
- `enrolled` = must be a member of the group  
- `assigned` = must be assigned as evaluator  
- `all` = can see all records, not just own
