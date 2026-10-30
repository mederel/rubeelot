# Personal data breach procedure

What to do when personal data kept by Beelot may have been lost, disclosed,
altered or made unavailable, accidentally or unlawfully (GDPR articles 33 and
34). A breach can be an attack on the server, a leaked backup, a database
dump sent to the wrong place, or a bug that
shows one player's data to another.

## Who decides

The data controller named by `BEELOT_PRIVACY_CONTROLLER` decides whether a
breach happened, whether to notify the CNIL and whether to inform players.
Whoever notices a possible breach tells the controller at once, by the
privacy contact (`BEELOT_PRIVACY_CONTACT`) or directly, even when unsure.

## First steps

1. Contain it: revoke leaked credentials (database, hosting), close the hole, take the affected service offline if
   needed. Restarting Beelot ends every session, since sessions are kept in
   memory.
2. Preserve the evidence: keep the application logs, the `security_event`
   table and the hosting provider's logs. Do not delete or rewrite them while
   the breach is assessed; the nightly retention job only purges events older
   than 12 months.
3. Write down the time you became aware of the breach: the 72-hour deadline
   runs from it.

## Assessing the risk

Establish, as far as known:

- what data is involved: pseudonyms, sign-in dates, passkey public keys,
  IP addresses in security events, match history;
- how many players are affected;
- what could happen to them: location revealed by IP addresses, matches tied
  to a pseudonym;
- whether the data was protected (a passkey's public key cannot sign in: the
  private key never leaves the player's device; Beelot holds no email
  address or password).

A breach is notifiable unless it is unlikely to result in a risk to the
players' rights and freedoms. It must be communicated to the players when the
risk to them is high.

## Notifying the CNIL

Notify the CNIL within 72 hours of becoming aware of the breach, through its
online service: <https://notifications.cnil.fr/notifications/index>.

- Give what is known: the nature of the breach, the categories and
  approximate number of players and records, the likely consequences and the
  measures taken or planned.
- If some facts are not known yet, notify anyway and send the rest in
  follow-ups. A notification after 72 hours must say why it is late.

## Informing players

When the risk to players is high, tell the affected players without undue
delay, in plain language, in French, English and Dutch: what happened, what
it means for them, what Beelot did, what they can do (for example, be wary of
messages claiming to come from Beelot) and how to reach the privacy contact.
Beelot holds no address to write to: publish a notice on the site, for example
on the home screen.

## The breach log

Record every breach, notified or not, in a log kept by the controller: the
date and how it was found, the facts, the data and players involved, the
effects, the measures taken, and the decision on notifying the CNIL and the
players with its reasons. The CNIL may ask to see this log.
