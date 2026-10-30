# ADR-005: Sign in with a passkey only

- Status: Accepted; supersedes ADR-004
- Date: 2026-10-19
- Decision makers: Product and engineering

## Context

ADR-004 replaced Zitadel with email links in Spring and with Google and GitHub
as direct OAuth providers. It left three costs:

- **An SMTP processor.** Email players can sign in only if the SMTP provider
  delivers the link. Production needs a provider, a data processing agreement,
  and a sender reputation to keep. Self-hosting a mail server was considered
  and rejected: delivery to the large mailbox providers is unreliable.
- **Email addresses.** Beelot holds an address per email player, with a
  change flow (US-079), a warning email before retention deletes an account,
  and the duty to protect every address.
- **Providers.** Google and GitHub are two more processors to list in the
  privacy policy, two client registrations to keep, and two sign-in paths to
  test.

A passkey needs none of them: the browser and the player's device hold the
private key, and Beelot keeps a public key.

## Options

1. **Keep ADR-004.** Choose an SMTP provider; the costs above stay.
2. **Passkeys alongside email links**, email as recovery. The SMTP processor
   and the addresses stay, so nothing above goes away.
3. **A hosted passkey service.** One more processor, which is ADR-004's
   reason for leaving Zitadel.
4. **Passkeys only, with Spring Security's WebAuthn support.**

## Decision

**Option 4.** The design is `docs/superpowers/specs/2026-10-19-passkeys-only-design.md`.

- **Clean break.** Flyway migration V8 deletes every account by the rules of
  US-081: finished matches stay in other players' history with the seats shown
  as "Deleted player", security events with an account go, sessions end on
  deploy. There is no migration path.
- **One passkey per account**, registered at sign-up, never added, replaced or
  removed later. A passkey synced by the platform (iCloud Keychain, Google
  Password Manager) works on all the player's devices.
- **No recovery**: no link, no code, no support process. Sign-up warns before
  the passkey is created.
- **Spring Security's WebAuthn support** (`spring-security-webauthn`, with
  webauthn4j) and its JDBC repositories on MariaDB verify challenges, origins
  and signatures. Spring does not check the signature counter, so Beelot
  refuses a sign-in whose counter does not go up (synced passkeys, which
  report 0, are exempt). Using webauthn4j directly is rejected: more
  security-critical code for no gain.
- **Discoverable credentials with user verification.** "Sign in with a
  passkey" is one button; the player types no user name.
- **Sign-up is Beelot's own endpoint**, because Spring's registration needs a
  signed-in user. It uses the same relying party as Spring's sign-in. The
  WebAuthn user name is the account id; the display name is the account's
  pseudonym; the user handle is random.
- **Relying party**: the host of `BEELOT_PUBLIC_URL` is the rp id, and its
  scheme, host and port the only allowed origin.
- **Rate limits**: 30 ceremonies started an hour per IP address, sign-ups and
  sign-ins together (`beelot.passkey.limit-per-ip`).

## Consequences

- **Removed:** email-link sign-in, the token store, the email sender and its
  templates, the change of address (US-079), the warning email before
  retention deletes an account, Google and GitHub sign-in, Mailpit in
  `compose.yaml`, and the `BEELOT_MAIL_*` and
  `SPRING_SECURITY_OAUTH2_CLIENT_*` variables.
- **Added:** the `user_entities` and `user_credentials` tables, the sign-up
  endpoints, the counter check, the passkey rate limits, and a nightly purge of
  passkeys made for sign-ups never finished.
- **Every existing account is deleted** by V8. Players sign up again.
- **A lost passkey is a lost account.** A player on a new device without sync
  creates a new account; their old history stays under the old account until
  retention deletes it.
- **The public URL is fixed** before the first sign-up: a passkey works only
  for the rp id it was made for, so changing the host makes every passkey
  unusable.
- **Privacy:** Beelot holds no email address and sends no email. Hosting is
  the only processor. The data export shows the passkey's dates, never its
  public key or credential id.
- **Operations:** sign-in depends on no outside service.
- **Phase 5 stories:** US-075, US-077 and US-079 are superseded by US-084;
  US-081 to US-083 stay, without email, Google or GitHub. The choice of an SMTP
  provider is no longer needed.

## Sources

- Spring Security, [Passkeys](https://docs.spring.io/spring-security/reference/servlet/authentication/passkeys.html).
- W3C, [Web Authentication Level 3](https://www.w3.org/TR/webauthn-3/), §6.1.1 on the signature counter.
- ADR-004 for the email-link design this replaces.
