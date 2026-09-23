//! 配置层：可被任意业务模块引用。本模块不依赖其它业务模块。

use std::{error::Error, io, path::Path};

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
#[cfg(feature = "embed-env")]
const EMBEDDED_ENV: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/.env"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenAiApi {
    Responses,
    ChatCompletions,
}

#[derive(Debug, Clone)]
pub(crate) struct Config {
    pub(crate) api_key: String,
    pub(crate) model: String,
    pub(crate) base_url: String,
    pub(crate) api: OpenAiApi,
    pub(crate) tools_enabled: bool,
}

impl Config {
    pub(crate) fn load() -> Result<Self, Box<dyn Error>> {
        if Path::new(".env").is_file() {
            dotenvy::dotenv()?;
        }

        #[cfg(feature = "embed-env")]
        dotenvy::from_read(EMBEDDED_ENV.as_bytes())?;

        Self::from_values(
            std::env::var("OPENAI_API_KEY").ok(),
            std::env::var("OPENAI_MODEL").ok(),
            std::env::var("OPENAI_BASE_URL").ok(),
            std::env::var("OPENAI_API").ok(),
            std::env::var("GEER_AGENT_TOOLS").ok(),
        )
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message).into())
    }

    fn from_values(
        api_key: Option<String>,
        model: Option<String>,
        base_url: Option<String>,
        api: Option<String>,
        tools_enabled: Option<String>,
    ) -> Result<Self, String> {
        let api_key = required_value(api_key, "OPENAI_API_KEY")?;
        let model = required_value(model, "OPENAI_MODEL")?;
        let base_url = base_url
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_owned());
        let api = match api.as_deref().map(str::trim) {
            None | Some("") | Some("responses") => OpenAiApi::Responses,
            Some("chat-completions") => OpenAiApi::ChatCompletions,
            Some(other) => {
                return Err(format!(
                    "不支持的 OPENAI_API 值：{other}。可选 responses 或 chat-completions。"
                ));
            }
        };

        let tools_enabled = match tools_enabled.as_deref().map(str::trim) {
            None | Some("") | Some("on") => true,
            Some("off") => false,
            Some(other) => {
                return Err(format!(
                    "不支持的 GEER_AGENT_TOOLS 值：{other}。可选 on 或 off。"
                ));
            }
        };

        Ok(Self {
            api_key,
            model,
            base_url,
            api,
            tools_enabled,
        })
    }
}

fn required_value(value: Option<String>, name: &str) -> Result<String, String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("缺少必需配置 {name}，请在 .env 或进程环境变量中设置。"))
}

#[cfg(test)]
mod tests {
    use super::{Config, DEFAULT_BASE_URL, OpenAiApi};

    #[test]
    fn rejects_missing_api_key() {
        let error = Config::from_values(None, Some("gpt-test".to_owned()), None, None, None)
            .expect_err("缺少 key 时应报错");

        assert!(error.contains("OPENAI_API_KEY"));
    }

    #[test]
    fn rejects_missing_model() {
        let error = Config::from_values(Some("secret".to_owned()), None, None, None, None)
            .expect_err("缺少 model 时应报错");

        assert!(error.contains("OPENAI_MODEL"));
    }

    #[test]
    fn uses_openai_base_url_by_default_and_trims_values() {
        let config = Config::from_values(
            Some(" secret ".to_owned()),
            Some(" gpt-test ".to_owned()),
            Some("  ".to_owned()),
            None,
            None,
        )
        .expect("key 与 model 已提供");

        assert_eq!(config.api_key, "secret");
        assert_eq!(config.model, "gpt-test");
        assert_eq!(config.base_url, DEFAULT_BASE_URL);
        assert_eq!(config.api, OpenAiApi::Responses);
        assert!(config.tools_enabled);
    }

    #[test]
    fn accepts_an_overridden_base_url() {
        let config = Config::from_values(
            Some("secret".to_owned()),
            Some("gpt-test".to_owned()),
            Some("https://proxy.example/v1".to_owned()),
            Some("chat-completions".to_owned()),
            Some("off".to_owned()),
        )
        .expect("自定义 endpoint 应被接受");

        assert_eq!(config.base_url, "https://proxy.example/v1");
        assert_eq!(config.api, OpenAiApi::ChatCompletions);
        assert!(!config.tools_enabled);
    }

    #[test]
    fn rejects_unknown_api() {
        let error = Config::from_values(
            Some("secret".to_owned()),
            Some("gpt-test".to_owned()),
            None,
            Some("other".to_owned()),
            None,
        )
        .expect_err("未知接口应在进入 REPL 前报错");
        assert!(error.contains("OPENAI_API"));
    }
}
