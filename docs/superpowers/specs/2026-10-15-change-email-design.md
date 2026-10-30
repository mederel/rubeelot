# Change my email address (US-079) — design

## Goal

A player who signs in with an email link can move their account to a new
mailbox without losing their pseudonym, statistics or match history. The new
address takes over only once the player proves they read it; until then the
old address keeps working, and the old address is told about the change.

## Acceptance criteria and how they are met

| Criterion | Met by |
| --- | --- |
| An "Account" section in Settings lets email-link players enter a new address | "Account" panel, shown when `/api/account` returns `email` |
| The new address is used only after a link sent to it is opened, with the rules of US-075 | `email_change_token` (hashed, single use, 15 min), `POST /api/account/email/confirm` |
| Until then, the old address keeps working | `account_email` changes only on confirmation |
| The old address is notified of the change | Change notice sent by `EmailChangeService.confirm` |
| An address of another account is refused without revealing it: it receives a notice instead of a link | Same 204 answer; "already has an account" notice |
| Google and GitHub accounts do not see this option | No `email` in `/api/account`; endpoint answers 403 |

## Decisions

- **A recent sign-in is required to ask for a change.** Like deletion
  (US-081), the request needs a sign-in less than 10 minutes old
  (`AccountSessions.signedInRecently`); otherwise it answers 403
  `{"reason": "reauthenticate"}` and the client reuses the existing re-sign-in
  prompt. A stolen session cannot move the account to another mailbox.
- **Confirmation ends every session and signs in the confirming browser.**
  An email-link session is identified by its address, so sessions holding the
  old address would no longer find their account. They all end
  (`AccountSessions.invalidateAll`), which also suits a security-sensitive
  change; the browser that opened the link is signed in with the new address.
- **The link needs no sign-in to confirm.** The token proves control of the
  new mailbox and was issued to a session with a recent sign-in, so the link
  can be opened on another device.
- **The link is confirmed by a click, not by opening it.** As with sign-in
  links, the `/settings/email?token=…` view removes the token from the address
  bar and posts it only when the player presses "Confirm my new address", so
  mail scanners that prefetch links cannot spend it.
- **One pending change per account.** A new request deletes the account's
  earlier tokens; only the latest link works.
- **Same answer for a free and a taken address.** Both answer 204 and send
  one email to the new address: a link if it is free, a notice if another
  account has it. The rate limit is the sign-in limiter's
  (`SignInRateLimiter`, per address and per IP), so requests count against the
  same budget as sign-in links.
- **The current address is refused openly.** Entering the account's own
  address answers 400 "This is already your address": it reveals nothing the
  player does not know.
- **A race on confirmation is refused.** If another account took the address
  between the request and the confirmation, the confirmation answers 400 "This
  link no longer works" and nothing changes; the unique constraint on
  `account_email.email` backs the check.

## Data

`V7__email_change.sql`:

```sql
-- Pending email address changes (US-079); only the SHA-256 hash of each token is stored. Times are in UTC.
CREATE TABLE email_change_token (
    token_hash CHAR(64)     NOT NULL PRIMARY KEY,
    account_id UUID         NOT NULL REFERENCES account (id) ON DELETE CASCADE,
    new_email  VARCHAR(254) NOT NULL,
    expires_at DATETIME(6)  NOT NULL
);
CREATE INDEX email_change_token_account_idx ON email_change_token (account_id);
CREATE INDEX email_change_token_expires_idx ON email_change_token (expires_at);
```

The cascade removes a pending change when its account is deleted (US-081);
the hourly purge removes tokens expired for more than a day, as for
`sign_in_token`. The data export (US-082) already reads the address from
`account_email`, so it shows the new address once confirmed; a pending change
is not exported, as pending sign-in tokens are not.

## Components

All in `fr.beelot.application.account` unless stated.

- **`EmailChangeService`**
  - `request(UUID accountId, String newEmail, Locale locale, String ip)`
    returns `Sent` (for a free or a taken address alike), `InvalidAddress`,
    `SameAddress`, `RateLimited` or `MailFailed`.
    Normalises with `EmailAddresses`, checks the limiter, deletes the
    account's pending tokens, then either stores a token (32 random bytes,
    base64url, SHA-256 stored, lifetime `beelot.sign-in.link-lifetime`) and
    sends the link, or sends the "already has an account" notice. Records
    `EMAIL_CHANGE_REQUESTED` with the account and IP.
  - `confirm(String token, Locale locale, String ip)` returns
    `Optional<Confirmed(UUID accountId, String newEmail)>`. In one
    transaction: deletes the token row and accepts it only if this call
    deleted it and it had not expired; checks the new address is still free;
    updates `account_email`. After commit: sends the change notice to the old
    address, records `EMAIL_CHANGED`, ends every session of the account.
  - The hashing helper is shared with `SignInTokenService`.
  - The hourly purge of expired tokens lives here.
- **`EmailChangeEmail`**: the three emails in English, French and Dutch,
  built like `SignInEmail`:
  - *link*: "Confirm your new Beelot address", the link on its own line,
    works once for 15 minutes, ignore it if you did not ask;
  - *taken*: "This address already has a Beelot account", nothing was
    changed, you can sign in with this address as usual;
  - *changed* (to the old address): "Your Beelot address was changed" to the
    masked new address (first letter of the local part, then `***@domain`),
    and if it was not you, write to the privacy contact
    (`PrivacyProperties`).
- **`EmailChangeController`**
  - `POST /api/account/email` (`email` form field): 401 for a guest, 403 for
    a Google or GitHub account, 403 `reauthenticate` without a recent sign-in,
    400 with a message for an invalid or the current address, 429 when
    rate-limited, 503 when the mail cannot be sent, else 204.
  - `POST /api/account/email/confirm` (`token`, `language` form fields):
    400 "This link no longer works" when `confirm` is empty; otherwise signs
    this browser in with the new address — changes the session id, stores a
    `OneTimeTokenAuthentication` for the new address in the security context
    repository, sets `SecurityEvents.ACCOUNT_ID`, calls
    `AccountSessions.signedIn`, records `SIGN_IN` — and answers 204.
  - Both are CSRF-protected like the other state-changing endpoints; the
    confirm endpoint is open to guests in the security chain.
- **`AccountController`**: `AccountView` gains `email`, present only for an
  email-link account.
- **`SecurityEvents.Type`** gains `EMAIL_CHANGE_REQUESTED` and
  `EMAIL_CHANGED`. Neither ever holds a token or an address.

## Client

`index.html`, `app.js`, `i18n.js`, `i18n-nl.js` (English, French, Dutch).

- **"Account" panel in Settings**, shown only when `/api/account` has
  `email`: the current address, a "New email address" field and a "Send a
  confirmation link" button. After 204: "Check your new mailbox. Your current
  address keeps working until you open the link." A 403 `reauthenticate`
  opens the re-sign-in prompt used by deletion; 400, 429 and 503 show the
  server's message.
- **Confirmation view** for `/settings/email?token=…`: takes the token out of
  the address bar, shows "Confirm my new address". On 204: "Your address is
  now confirmed. Your other sessions have been signed out." and the account
  state is refreshed; on 400: "This link no longer works. Ask for a new one in
  Settings."
- **Privacy policy**: the sentence on what the address is used for adds
  "confirming a change of address and telling the old address about it".

## Error handling

- A mail failure on request answers 503 and keeps no token, so a retry starts
  clean.
- A mail failure on the change notice is logged (class name only, never the
  address) and does not undo the confirmed change.
- A failed security event insert never blocks the action (existing
  `SecurityEvents` rule).

## Testing

- `EmailChangeEmailTest`: each email in each language; the link on its own
  line; the masking of the new address.
- `EmailChangeServiceTest` / integration test against the database: free
  address sends a link, taken address sends a notice and stores no token,
  same address and invalid address refused, rate limit, a second request
  invalidates the first link, expired and spent tokens refused, the race on
  confirmation, deletion of the account removes the pending change.
- `EmailChangeIntegrationTest` (MockMvc, mocked mail): request → read link →
  confirm; the old address no longer signs in to the account and the new one
  does; earlier sessions are ended; the confirming session is signed in; the
  security events are recorded without tokens; 401/403 cases for guests,
  OAuth accounts and stale sign-ins.
- `AccountUiTest`: the Account panel is shown for an email account and
  hidden for Google and GitHub accounts.
- Manual check in a headless browser with Mailpit.

## Out of scope

- Keeping other sessions alive across a change of address.
- Adding an email address to a Google or GitHub account, or linking
  accounts.
- Undoing a change from the old address's notice; the notice points to the
  privacy contact.

## Documentation

- `docs/product-backlog.md`: US-079 marked delivered with a link to this
  spec; phase 5 line updated.
- `AGENTS.md`/README need no new setup step: no new property.
