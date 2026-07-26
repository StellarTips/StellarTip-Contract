# Events Catalog

Every event the StellarTip contract emits via `env.events().publish(topic, payload)`.

Soroban events have two parts:
- **Topic** – a tuple used for filtering/indexing (always `(symbol, address)` here).
- **Payload** – the data body of the event.

All symbols are `symbol_short!` values (≤ 9 characters).

---

## INIT

**Constant:** `EVENT_INIT` = `"INIT"`  
**Emitted by:** `init()`  
**When:** Contract is initialized for the first time.

| Part    | Type                          | Description                                  |
|---------|-------------------------------|----------------------------------------------|
| topic   | `(Symbol, Address)`           | `("INIT", caller)` — the new admin address   |
| payload | `(Address, u32)`              | `(fee_recipient, fee_bps)`                   |

---

## CREG — Creator Registered

**Constant:** `EVENT_CREATOR_REGISTERED` = `"CREG"`  
**Emitted by:** `register()`  
**When:** A new creator profile is successfully registered.

| Part    | Type                          | Description                                  |
|---------|-------------------------------|----------------------------------------------|
| topic   | `(Symbol, Address)`           | `("CREG", caller)` — the creator's address   |
| payload | `(Symbol, u64)`               | `(username, registered_at)` — ledger timestamp |

---

## TIP — Tip Sent

**Constant:** `EVENT_TIP_SENT` = `"TIP"`  
**Emitted by:** `tip()`  
**When:** A supporter successfully sends a tip to a creator.

| Part    | Type                               | Description                                                      |
|---------|------------------------------------|------------------------------------------------------------------|
| topic   | `(Symbol, Address)`                | `("TIP", from)` — the tipper's address                          |
| payload | `(Address, Address, i128, i128, u64)` | `(creator, token, amount, fee, index)` — creator address, token contract address, gross tip amount, platform fee deducted, tip record index |

---

## WDRW — Withdrawal

**Constant:** `EVENT_WITHDRAW` = `"WDRW"`  
**Emitted by:** `withdraw()`  
**When:** A creator withdraws tokens from their accumulated balance.

| Part    | Type                | Description                                     |
|---------|---------------------|-------------------------------------------------|
| topic   | `(Symbol, Address)` | `("WDRW", caller)` — the creator's address      |
| payload | `(Address, i128)`   | `(token, amount)` — token contract address and amount withdrawn |

---

## PUPD — Profile Updated

**Constant:** `EVENT_PROFILE_UPDATED` = `"PUPD"`  
**Emitted by:** `update_profile()`  
**When:** A creator updates their display name or bio.

| Part    | Type                | Description                                          |
|---------|---------------------|------------------------------------------------------|
| topic   | `(Symbol, Address)` | `("PUPD", caller)` — the creator's address           |
| payload | `(Symbol, String)`  | `(username, display_name)` — unchanged username and new display name |

---

## UREG — Creator Unregistered

**Constant:** `EVENT_CREATOR_UNREGISTERED` = `"UREG"`  
**Emitted by:** `unregister()`  
**When:** A creator removes their profile (requires zero balances).

| Part    | Type                | Description                                 |
|---------|---------------------|---------------------------------------------|
| topic   | `(Symbol, Address)` | `("UREG", caller)` — the creator's address  |
| payload | `()`                | Empty                                       |

---

## PAUS — Paused

**Constant:** `EVENT_PAUSED` = `"PAUS"`  
**Emitted by:** `pause()`  
**When:** The admin activates the emergency stop.

| Part    | Type                | Description                            |
|---------|---------------------|----------------------------------------|
| topic   | `(Symbol, Address)` | `("PAUS", caller)` — the admin address |
| payload | `()`                | Empty                                  |

---

## UNPA — Unpaused

**Constant:** `EVENT_UNPAUSED` = `"UNPA"`  
**Emitted by:** `unpause()`  
**When:** The admin lifts the emergency stop.

| Part    | Type                | Description                            |
|---------|---------------------|----------------------------------------|
| topic   | `(Symbol, Address)` | `("UNPA", caller)` — the admin address |
| payload | `()`                | Empty                                  |

---

## FEEC — Fee Changed

**Constant:** `EVENT_FEE_CHANGED` = `"FEEC"`  
**Emitted by:** `set_fee_percentage()`  
**When:** The admin updates the platform fee.

| Part    | Type                | Description                                         |
|---------|---------------------|-----------------------------------------------------|
| topic   | `(Symbol, Address)` | `("FEEC", caller)` — the admin address              |
| payload | `u32`               | `fee_bps` — new fee in basis points (0–10 000)      |

---

## FERC — Fee Recipient Changed

**Constant:** `EVENT_FEE_RECIPIENT_CHANGED` = `"FERC"`  
**Emitted by:** `set_fee_recipient()`  
**When:** The admin changes the address that receives platform fees.

| Part    | Type                | Description                                     |
|---------|---------------------|-------------------------------------------------|
| topic   | `(Symbol, Address)` | `("FERC", caller)` — the admin address          |
| payload | `Address`           | `fee_recipient` — the new fee recipient address |

---

## ADMC — Admin Changed

**Constant:** `EVENT_ADMIN_CHANGED` = `"ADMC"`  
**Emitted by:** `set_admin()`  
**When:** Admin privileges are transferred to a new address.

| Part    | Type                | Description                                 |
|---------|---------------------|---------------------------------------------|
| topic   | `(Symbol, Address)` | `("ADMC", caller)` — the outgoing admin     |
| payload | `Address`           | `new_admin` — the incoming admin address    |

---

## CAPMC — Max Creators Cap Changed

**Constant:** `EVENT_MAX_CREATORS_CHANGED` = `"CAPMC"`  
**Emitted by:** `set_max_creators()`  
**When:** The admin updates the global creator registration cap.

| Part    | Type                | Description                                              |
|---------|---------------------|----------------------------------------------------------|
| topic   | `(Symbol, Address)` | `("CAPMC", caller)` — the admin address                  |
| payload | `u32`               | `max_creators` — new cap (`0` = unlimited)               |

---

## CAPMT — Max Tips Cap Changed

**Constant:** `EVENT_MAX_TIPS_CHANGED` = `"CAPMT"`  
**Emitted by:** `set_max_tips_per_creator()`  
**When:** The admin updates the per-creator tip history cap.

| Part    | Type                | Description                                              |
|---------|---------------------|----------------------------------------------------------|
| topic   | `(Symbol, Address)` | `("CAPMT", caller)` — the admin address                  |
| payload | `u32`               | `max_tips` — new cap per creator (`0` = unlimited)       |

---

## MINTC — Minimum Tip Amount Changed

**Constant:** `EVENT_MIN_TIP_CHANGED` = `"MINTC"`  
**Emitted by:** `set_min_tip_amount()`  
**When:** The admin updates the minimum accepted tip amount.

| Part    | Type                | Description                                              |
|---------|---------------------|----------------------------------------------------------|
| topic   | `(Symbol, Address)` | `("MINTC", caller)` — the admin address                  |
| payload | `i128`              | `min_tip_amount` — new minimum in token base units (`0` = no minimum) |

---

## Summary Table

| Symbol  | Constant                        | Emitted by                  | Payload types                                    |
|---------|---------------------------------|-----------------------------|--------------------------------------------------|
| `INIT`  | `EVENT_INIT`                    | `init()`                    | `(Address, u32)`                                 |
| `CREG`  | `EVENT_CREATOR_REGISTERED`      | `register()`                | `(Symbol, u64)`                                  |
| `TIP`   | `EVENT_TIP_SENT`                | `tip()`                     | `(Address, Address, i128, i128, u64)`            |
| `WDRW`  | `EVENT_WITHDRAW`                | `withdraw()`                | `(Address, i128)`                                |
| `PUPD`  | `EVENT_PROFILE_UPDATED`         | `update_profile()`          | `(Symbol, String)`                               |
| `UREG`  | `EVENT_CREATOR_UNREGISTERED`    | `unregister()`              | `()`                                             |
| `PAUS`  | `EVENT_PAUSED`                  | `pause()`                   | `()`                                             |
| `UNPA`  | `EVENT_UNPAUSED`                | `unpause()`                 | `()`                                             |
| `FEEC`  | `EVENT_FEE_CHANGED`             | `set_fee_percentage()`      | `u32`                                            |
| `FERC`  | `EVENT_FEE_RECIPIENT_CHANGED`   | `set_fee_recipient()`       | `Address`                                        |
| `ADMC`  | `EVENT_ADMIN_CHANGED`           | `set_admin()`               | `Address`                                        |
| `CAPMC` | `EVENT_MAX_CREATORS_CHANGED`    | `set_max_creators()`        | `u32`                                            |
| `CAPMT` | `EVENT_MAX_TIPS_CHANGED`        | `set_max_tips_per_creator()`| `u32`                                            |
| `MINTC` | `EVENT_MIN_TIP_CHANGED`         | `set_min_tip_amount()`      | `i128`                                           |
