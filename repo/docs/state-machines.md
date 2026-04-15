# State Machines

## Booking State Machine

```
Draft ──────────────────────────────┐
  │                                 │
  ▼                                 │
Held ──► Confirmed ──► Changed ─────┤
  │         │             │         │
  │         ▼             ▼         ▼
  │     ExceptionPending  Completed Cancelled
  │         │
  │         ▼
  │     Confirmed (resolved)
  │
  ▼
Expired  (terminal, via job)
```

**Transitions:**
| From | To | Trigger |
|------|----|---------|
| Draft | Held | Inventory hold created on booking creation |
| Draft | Cancelled | User cancels before holds placed |
| Held | Confirmed | Member confirms booking |
| Held | Cancelled | Member/Ops cancels |
| Held | Expired | Job: hold timeout exceeded |
| Confirmed | Changed | Booking details modified |
| Confirmed | Cancelled | Member/Ops cancels |
| Confirmed | Completed | Ops marks complete |
| Confirmed | ExceptionPending | Ops flags exception |
| Changed | Confirmed | Re-confirmed after change |
| Changed | Cancelled | Cancelled after change |
| Changed | Completed | Completed from changed state |
| ExceptionPending | Confirmed | Exception resolved |
| ExceptionPending | Cancelled | Exception results in cancellation |

**Terminal states:** `Cancelled`, `Completed`, `Expired`

---

## Evaluation State Machine

```
Draft ──► Open ──► InReview ──► Completed
  │         │         │
  └─────────┴─────────┴──► Cancelled
```

**Transitions:**
| From | To |
|------|----|
| Draft | Open, Cancelled |
| Open | InReview, Cancelled |
| InReview | Completed, Open (re-opened) |

**Terminal states:** `Completed`, `Cancelled`

---

## Assignment State Machine

```
Pending ──► InProgress ──► Submitted ──► Approved
                                    └──► Rejected
```

**Transitions:**
| From | To |
|------|----|
| Pending | InProgress |
| InProgress | Submitted |
| Submitted | Approved, Rejected |

**Terminal states:** `Approved`, `Rejected`

**Node-level permission:** Only the assigned evaluator (or Admin) can transition their own assignment.

---

## Payment Intent State Machine

```
Open ──► Captured
  │
  └──► TimedOut  (via job at expires_at)
  │
  └──► Cancelled
```

---

## Notification Delivery State

```
                  ┌──► Delivered  (dispatch succeeded)
Pending ──────────┤
                  └──► Failed     (dispatch returned error)

Pending ──► SuppressedDnd ──► Delivered  (dnd_resolver job, dispatch succeeded)
                          └──► Failed    (dnd_resolver job, dispatch failed)

Pending ──► OptedOut  (user opted out of this trigger category)
```

**State meanings:**
- `Pending` — notification record created; awaiting dispatch attempt
- `Delivered` — dispatch attempt recorded with `succeeded = true`
- `Failed` — dispatch attempt recorded with `succeeded = false`; `error_detail` contains reason
- `SuppressedDnd` — non-critical notification queued past the user's quiet-hours window
- `OptedOut` — user has opted out of this trigger category; notification recorded but not dispatched

**DND Rule:** Non-critical notifications sent during the user's local quiet-hours window (`APP__DND__START_HOUR`–`APP__DND__END_HOUR`) are placed in `SuppressedDnd` state and queued for redelivery after the window ends. Critical templates bypass DND entirely.

**Channel dispatch:** InApp delivery is immediate (DB-persisted, client-polled). Email/SMS/Push channels are modeled in the DB enum (migration 0021) and dispatch is gated at runtime: when a provider is not configured, `dispatch_to_channel` returns an error and the service automatically creates an InApp fallback notification with a successful attempt record, so the user always receives the message. To enable a non-InApp channel, wire the provider in `dispatch_to_channel` and return `Ok(())`.

**Fallback semantics:** When any non-InApp dispatch fails, a new InApp notification is created as a fallback with `Delivered` state. Both the original failed attempt and the fallback delivery are recorded in `notification_attempts` for auditability.
