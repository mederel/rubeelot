//! Passkey ceremonies with webauthn-rs. Signing up names a new account id and a pseudonym; the browser makes a
//! discoverable passkey that verifies the player. Signing in needs no user name: the passkey says whose it is. The
//! state of a ceremony in progress is kept in memory for 5 minutes, keyed by the `BEELOT_CEREMONY` cookie, and is
//! used once, whatever the answer.

use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use dashmap::DashMap;
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;
use webauthn_rs::prelude::{
    AuthenticationResult, CreationChallengeResponse, DiscoverableAuthentication, DiscoverableKey,
    Passkey, PasskeyRegistration, PublicKeyCredential, RegisterPublicKeyCredential,
    RequestChallengeResponse,
};
use webauthn_rs::{Webauthn, WebauthnBuilder};
use webauthn_rs_proto::{
    AuthenticatorSelectionCriteria, ResidentKeyRequirement, UserVerificationPolicy,
};

use super::pseudonyms::{self, Language};
use super::relying_party::RelyingParty;
use crate::clock::Clock;
use crate::config::Limits;
use crate::error::AppError;
use crate::limits::FixedWindowLimiter;
use crate::limits::window::MAX_TRACKED;
use crate::tables::OsRandom;

pub const COOKIE: &str = "BEELOT_CEREMONY";
const KEPT_FOR: Duration = Duration::from_secs(300);
/// Ceremonies in progress kept at most, whatever their age.
const MAX_CEREMONIES: usize = 10_000;
const HOUR: Duration = Duration::from_secs(3600);

/// A ceremony's state between its options and its answer.
pub enum Ceremony {
    SignUp {
        state: PasskeyRegistration,
        account: Uuid,
        name: String,
    },
    SignIn(DiscoverableAuthentication),
}

/// Whether a signature counter is valid after the stored one (WebAuthn §6.1.1): it goes up, or both are 0, as with
/// synced passkeys. A counter that doesn't go up is a sign that the passkey was cloned.
pub fn counter_goes_up(stored: u32, presented: u32) -> bool {
    (stored == 0 && presented == 0) || presented > stored
}

/// The counter of a stored passkey, which webauthn-rs keeps to itself but serializes.
pub fn stored_counter(passkey: &Passkey) -> u32 {
    serde_json::to_value(passkey)
        .ok()
        .and_then(|value| value["cred"]["counter"].as_u64())
        .and_then(|counter| u32::try_from(counter).ok())
        .unwrap_or(u32::MAX)
}

pub struct Passkeys {
    webauthn: Webauthn,
    ceremonies: DashMap<String, (Ceremony, OffsetDateTime)>,
    /// Ceremonies started per client address per hour.
    pub limiter: FixedWindowLimiter,
    clock: Arc<dyn Clock>,
}

impl Passkeys {
    pub fn new(public_url: &str, limits: &Limits, clock: Arc<dyn Clock>) -> Result<Self, String> {
        let party = RelyingParty::of(public_url)?;
        let origin = Url::parse(&party.origin).map_err(|error| error.to_string())?;
        let webauthn = WebauthnBuilder::new(&party.id, &origin)
            .and_then(|builder| builder.rp_name("Beelot").build())
            .map_err(|error| error.to_string())?;
        Ok(Self {
            webauthn,
            ceremonies: DashMap::new(),
            limiter: FixedWindowLimiter::new(
                limits.passkey_ceremonies_per_ip_per_hour,
                HOUR,
                MAX_TRACKED,
                clock.clone(),
            ),
            clock,
        })
    }

    fn keep(&self, ceremony: Ceremony) -> Result<String, AppError> {
        let now = self.clock.now();
        if self.ceremonies.len() >= MAX_CEREMONIES {
            self.ceremonies.retain(|_, (_, expires)| *expires > now);
        }
        if self.ceremonies.len() >= MAX_CEREMONIES {
            return Err(AppError::CapacityExceeded);
        }
        let mut bytes = [0u8; 24];
        getrandom::fill(&mut bytes).expect("the operating system's random generator");
        let id = URL_SAFE_NO_PAD.encode(bytes);
        self.ceremonies
            .insert(id.clone(), (ceremony, now + KEPT_FOR));
        Ok(id)
    }

    /// The ceremony's state, taken out: it is answered once.
    pub fn take(&self, id: &str) -> Option<Ceremony> {
        let (_, (ceremony, expires)) = self.ceremonies.remove(id)?;
        (expires > self.clock.now()).then_some(ceremony)
    }

    /// Options for a new discoverable passkey that verifies the player, for a new account with a pseudonym.
    pub fn sign_up_options(
        &self,
        language: Language,
    ) -> Result<(String, CreationChallengeResponse), AppError> {
        let account = Uuid::new_v4();
        let name = pseudonyms::generate(language, &mut OsRandom);
        let (mut challenge, state) = self
            .webauthn
            .start_passkey_registration(account, &account.to_string(), &name, None)
            .map_err(|error| AppError::Internal(anyhow::anyhow!("passkey options: {error}")))?;
        challenge.public_key.authenticator_selection = Some(AuthenticatorSelectionCriteria {
            authenticator_attachment: None,
            resident_key: Some(ResidentKeyRequirement::Required),
            require_resident_key: true,
            user_verification: UserVerificationPolicy::Required,
        });
        let id = self.keep(Ceremony::SignUp {
            state,
            account,
            name,
        })?;
        Ok((id, challenge))
    }

    /// The new passkey, verified against its options, with the account id and pseudonym they named.
    pub fn finish_sign_up(
        &self,
        ceremony: Option<Ceremony>,
        credential: &RegisterPublicKeyCredential,
    ) -> Option<(Uuid, String, Passkey)> {
        let Some(Ceremony::SignUp {
            state,
            account,
            name,
        }) = ceremony
        else {
            return None;
        };
        match self
            .webauthn
            .finish_passkey_registration(credential, &state)
        {
            Ok(passkey) => Some((account, name, passkey)),
            Err(error) => {
                // Never the credential: only what failed.
                tracing::debug!(%error, "sign-up refused");
                None
            }
        }
    }

    pub fn sign_in_options(&self) -> Result<(String, RequestChallengeResponse), AppError> {
        let (challenge, state) = self
            .webauthn
            .start_discoverable_authentication()
            .map_err(|error| AppError::Internal(anyhow::anyhow!("passkey options: {error}")))?;
        Ok((self.keep(Ceremony::SignIn(state))?, challenge))
    }

    /// The account and credential id the assertion names.
    pub fn identify(&self, assertion: &PublicKeyCredential) -> Option<(Uuid, Vec<u8>)> {
        self.webauthn
            .identify_discoverable_authentication(assertion)
            .ok()
            .map(|(account, credential)| (account, credential.to_vec()))
    }

    /// Verifies the assertion with the stored passkey, refusing a counter that doesn't go up.
    pub fn finish_sign_in(
        &self,
        ceremony: Option<Ceremony>,
        assertion: &PublicKeyCredential,
        passkey: &Passkey,
    ) -> Option<AuthenticationResult> {
        let Some(Ceremony::SignIn(state)) = ceremony else {
            return None;
        };
        let result = self
            .webauthn
            .finish_discoverable_authentication(assertion, state, &[DiscoverableKey::from(passkey)])
            .map_err(|error| tracing::debug!(%error, "sign-in refused"))
            .ok()?;
        if !counter_goes_up(stored_counter(passkey), result.counter()) {
            tracing::debug!("sign-in refused: the signature counter did not go up");
            return None;
        }
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    //! Ports Java's `CounterCheckingRelyingPartyOperationsTest`.

    use super::*;

    #[test]
    fn a_counter_that_goes_up_or_stays_at_zero_is_allowed() {
        for (stored, presented) in [(0, 0), (4, 5), (0, 1), (4_294_967_294, 4_294_967_295)] {
            assert!(
                counter_goes_up(stored, presented),
                "{presented} after {stored}"
            );
        }
    }

    #[test]
    fn a_counter_that_does_not_go_up_is_refused() {
        for (stored, presented) in [(4, 4), (4, 3), (4, 0)] {
            assert!(
                !counter_goes_up(stored, presented),
                "{presented} after {stored}"
            );
        }
    }
}
