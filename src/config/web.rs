#[cfg(feature = "web")]
use std::env;
use std::io;
use uuid::Uuid;

pub(crate) struct WebConfig {
    pub port: u16,
    pub token: String,
    pub generated: bool,
}

impl WebConfig {
    #[cfg(feature = "web")]
    pub fn load() -> io::Result<Self> {
        let read = |name| match env::var(name) {
            Ok(value) => Ok(Some(value)),
            Err(env::VarError::NotPresent) => Ok(None),
            Err(error) => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{name}: {error}"),
            )),
        };
        Self::parse(
            read("GEER_AGENT_WEB_PORT")?.as_deref(),
            read("GEER_AGENT_WEB_TOKEN")?.as_deref(),
        )
    }

    fn parse(port: Option<&str>, token: Option<&str>) -> io::Result<Self> {
        let port = match port {
            None => 9928,
            Some(value) => value
                .trim()
                .parse::<u16>()
                .ok()
                .filter(|port| *port != 0)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "GEER_AGENT_WEB_PORT 必须是 1..65535 的整数。",
                    )
                })?,
        };
        let token = token.map(str::trim).filter(|token| !token.is_empty());
        Ok(Self {
            port,
            token: token.map(str::to_owned).unwrap_or_else(|| {
                format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
            }),
            generated: token.is_none(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_fixed_token_and_port_validation() {
        let first = WebConfig::parse(None, None).unwrap();
        let second = WebConfig::parse(None, Some("  ")).unwrap();
        assert_eq!(first.port, 9928);
        assert!(first.generated && second.generated);
        assert_ne!(first.token, second.token);
        let fixed = WebConfig::parse(Some("9930"), Some("secret")).unwrap();
        assert_eq!(fixed.port, 9930);
        assert_eq!(fixed.token, "secret");
        assert!(!fixed.generated);
        for port in ["", "0", "65536", "-1", "abc", "1.2"] {
            assert!(WebConfig::parse(Some(port), None).is_err());
        }
        for port in ["1", "65535"] {
            assert!(WebConfig::parse(Some(port), None).is_ok());
        }
    }
}
