//! Beelot as the relying party of passkeys: the host players reach it at, which a passkey is bound to, and the one
//! origin its pages are served from.

use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelyingParty {
    pub id: String,
    pub origin: String,
}

impl RelyingParty {
    /// The relying party of `BEELOT_PUBLIC_URL`, such as `https://beelot.fr`; its path is ignored. Browsers send the
    /// origin, and check the id, in lower case.
    pub fn of(public_url: &str) -> Result<RelyingParty, String> {
        let invalid = || format!("Not an absolute http(s) URL: {public_url}");
        let url = Url::parse(public_url).map_err(|_| invalid())?;
        let scheme = url.scheme().to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return Err(invalid());
        }
        let host = url.host_str().ok_or_else(invalid)?.to_ascii_lowercase();
        let origin = match url.port() {
            Some(port) => format!("{scheme}://{host}:{port}"),
            None => format!("{scheme}://{host}"),
        };
        Ok(RelyingParty { id: host, origin })
    }
}

#[cfg(test)]
mod tests {
    //! Tests of the relying party.

    use super::*;

    fn party(id: &str, origin: &str) -> RelyingParty {
        RelyingParty {
            id: id.into(),
            origin: origin.into(),
        }
    }

    #[test]
    fn relying_party_from_public_url() {
        assert_eq!(
            RelyingParty::of("http://localhost:8080"),
            Ok(party("localhost", "http://localhost:8080"))
        );
        assert_eq!(
            RelyingParty::of("https://beelot.fr/"),
            Ok(party("beelot.fr", "https://beelot.fr"))
        );
        assert_eq!(
            RelyingParty::of("https://beelot.fr:443"),
            Ok(party("beelot.fr", "https://beelot.fr"))
        );
        assert_eq!(
            RelyingParty::of("HTTPS://Beelot.FR"),
            Ok(party("beelot.fr", "https://beelot.fr"))
        );
        assert_eq!(
            RelyingParty::of("http://beelot.fr:80/play"),
            Ok(party("beelot.fr", "http://beelot.fr"))
        );
        assert!(RelyingParty::of("beelot.fr").is_err());
        assert!(RelyingParty::of("ftp://beelot.fr").is_err());
    }
}
