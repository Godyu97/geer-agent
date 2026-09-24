//! 配置层：可被任意业务模块引用。本模块不依赖其它业务模块。

use std::{
    error::Error,
    fmt, io,
    path::{Path, PathBuf},
    time::Duration,
};

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MAX_DURATION: Duration = Duration::from_secs(600);
pub(crate) const DEFAULT_RESOURCE_LIMITS: ResourceLimits = ResourceLimits {
    max_input_tokens: None,
    max_output_tokens: None,
    max_cost_usd: None,
    input_usd_per_million: None,
    output_usd_per_million: None,
    max_duration: DEFAULT_MAX_DURATION,
};
#[cfg(feature = "embed-env")]
const EMBEDDED_ENV: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/.env"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpenAiApi {
    Responses,
    ChatCompletions,
}

impl OpenAiApi {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Responses => "responses",
            Self::ChatCompletions => "chat-completions",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TraceDatabase {
    Sqlite,
    Postgres,
    Mysql,
    Mongodb,
}

#[derive(Clone)]
pub(crate) struct TraceDatabaseConfig {
    pub(crate) kind: TraceDatabase,
    pub(crate) url: String,
}

impl fmt::Debug for TraceDatabaseConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TraceDatabaseConfig")
            .field("kind", &self.kind)
            .field("url", &"[REDACTED]")
            .finish()
    }
}

impl TraceDatabaseConfig {
    fn from_values(database: Option<String>, url: Option<String>) -> Result<Option<Self>, String> {
        let database = database.as_deref().map(str::trim).unwrap_or_default();
        let url = url.as_deref().map(str::trim).unwrap_or_default();
        if database.is_empty() {
            return if url.is_empty() {
                Ok(None)
            } else {
                Err(
                    "GEER_AGENT_TRACE_DATABASE_URL 需要同时设置 GEER_AGENT_TRACE_DATABASE。"
                        .to_owned(),
                )
            };
        }
        let kind = match database {
            "sqlite" => TraceDatabase::Sqlite,
            "postgres" => TraceDatabase::Postgres,
            "mysql" => TraceDatabase::Mysql,
            "mongodb" => TraceDatabase::Mongodb,
            _ => {
                return Err(
                    "GEER_AGENT_TRACE_DATABASE 只能是 sqlite、postgres、mysql 或 mongodb。"
                        .to_owned(),
                );
            }
        };
        if url.is_empty() {
            return Err("启用 Trace 时必须设置 GEER_AGENT_TRACE_DATABASE_URL。".to_owned());
        }
        let protocol_valid = match kind {
            TraceDatabase::Sqlite => url.starts_with("sqlite:"),
            TraceDatabase::Postgres => {
                url.starts_with("postgres://") || url.starts_with("postgresql://")
            }
            TraceDatabase::Mysql => url.starts_with("mysql://"),
            TraceDatabase::Mongodb => {
                url.starts_with("mongodb://") || url.starts_with("mongodb+srv://")
            }
        };
        if !protocol_valid {
            return Err("GEER_AGENT_TRACE_DATABASE_URL 与数据库类型的协议不匹配。".to_owned());
        }
        if kind == TraceDatabase::Mongodb {
            let authority_and_path = url
                .split_once("://")
                .map(|(_, rest)| rest)
                .unwrap_or_default();
            let database_name = authority_and_path
                .split_once('/')
                .map(|(_, path)| path.split('?').next().unwrap_or_default())
                .unwrap_or_default();
            if database_name.is_empty() {
                return Err("MongoDB Trace URI 必须包含数据库名。".to_owned());
            }
        }
        Ok(Some(Self {
            kind,
            url: url.to_owned(),
        }))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Config {
    pub(crate) api_key: String,
    pub(crate) model: String,
    pub(crate) base_url: String,
    pub(crate) api: OpenAiApi,
    pub(crate) tools_enabled: bool,
    pub(crate) bash_bin: PathBuf,
    pub(crate) limits: ResourceLimits,
    pub(crate) trace_database: Option<TraceDatabaseConfig>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ResourceLimits {
    pub(crate) max_input_tokens: Option<u64>,
    pub(crate) max_output_tokens: Option<u64>,
    pub(crate) max_cost_usd: Option<f64>,
    pub(crate) input_usd_per_million: Option<f64>,
    pub(crate) output_usd_per_million: Option<f64>,
    pub(crate) max_duration: Duration,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        DEFAULT_RESOURCE_LIMITS
    }
}

impl ResourceLimits {
    fn from_values(values: [Option<String>; 6]) -> Result<Self, String> {
        let [
            input_tokens,
            output_tokens,
            max_cost,
            input_rate,
            output_rate,
            duration,
        ] = values;
        let limits = Self {
            max_input_tokens: parse_positive_u64(input_tokens, "GEER_AGENT_MAX_INPUT_TOKENS")?,
            max_output_tokens: parse_positive_u64(output_tokens, "GEER_AGENT_MAX_OUTPUT_TOKENS")?,
            max_cost_usd: parse_positive_f64(max_cost, "GEER_AGENT_MAX_COST_USD")?,
            input_usd_per_million: parse_nonnegative_f64(
                input_rate,
                "GEER_AGENT_INPUT_USD_PER_MILLION_TOKENS",
            )?,
            output_usd_per_million: parse_nonnegative_f64(
                output_rate,
                "GEER_AGENT_OUTPUT_USD_PER_MILLION_TOKENS",
            )?,
            max_duration: parse_positive_u64(duration, "GEER_AGENT_MAX_DURATION_SECONDS")?
                .map(Duration::from_secs)
                .unwrap_or(DEFAULT_MAX_DURATION),
        };
        if limits.input_usd_per_million.is_some() != limits.output_usd_per_million.is_some() {
            return Err("输入与输出 Token 单价必须一起配置。".to_owned());
        }
        if limits.max_cost_usd.is_some() && limits.input_usd_per_million.is_none() {
            return Err("GEER_AGENT_MAX_COST_USD 需要同时配置输入与输出 Token 单价。".to_owned());
        }
        Ok(limits)
    }

    pub(crate) fn requires_usage(&self) -> bool {
        self.max_input_tokens.is_some()
            || self.max_output_tokens.is_some()
            || self.max_cost_usd.is_some()
    }
}

fn parse_positive_u64(value: Option<String>, name: &str) -> Result<Option<u64>, String> {
    value
        .map(|value| {
            value
                .trim()
                .parse::<u64>()
                .map_err(|_| format!("{name} 必须是正整数。"))
                .and_then(|number| {
                    (number > 0)
                        .then_some(number)
                        .ok_or_else(|| format!("{name} 必须是正整数。"))
                })
        })
        .transpose()
}

fn parse_positive_f64(value: Option<String>, name: &str) -> Result<Option<f64>, String> {
    parse_nonnegative_f64(value, name)?
        .map(|number| {
            (number > 0.0)
                .then_some(number)
                .ok_or_else(|| format!("{name} 必须是正数。"))
        })
        .transpose()
}

fn parse_nonnegative_f64(value: Option<String>, name: &str) -> Result<Option<f64>, String> {
    value
        .map(|value| {
            value
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("{name} 必须是非负数。"))
                .and_then(|number| {
                    (number.is_finite() && number >= 0.0)
                        .then_some(number)
                        .ok_or_else(|| format!("{name} 必须是非负数。"))
                })
        })
        .transpose()
}

impl Config {
    pub(crate) fn load() -> Result<Self, Box<dyn Error>> {
        if Path::new(".env").is_file() {
            dotenvy::dotenv()?;
        }

        #[cfg(feature = "embed-env")]
        dotenvy::from_read(EMBEDDED_ENV.as_bytes())?;

        let mut config = Self::from_values(
            std::env::var("OPENAI_API_KEY").ok(),
            std::env::var("OPENAI_MODEL").ok(),
            std::env::var("OPENAI_BASE_URL").ok(),
            std::env::var("OPENAI_API").ok(),
            std::env::var("GEER_AGENT_TOOLS").ok(),
            std::env::var("GEER_AGENT_BASH_BIN").ok(),
        )
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
        config.limits = ResourceLimits::from_values([
            std::env::var("GEER_AGENT_MAX_INPUT_TOKENS").ok(),
            std::env::var("GEER_AGENT_MAX_OUTPUT_TOKENS").ok(),
            std::env::var("GEER_AGENT_MAX_COST_USD").ok(),
            std::env::var("GEER_AGENT_INPUT_USD_PER_MILLION_TOKENS").ok(),
            std::env::var("GEER_AGENT_OUTPUT_USD_PER_MILLION_TOKENS").ok(),
            std::env::var("GEER_AGENT_MAX_DURATION_SECONDS").ok(),
        ])
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
        config.trace_database = TraceDatabaseConfig::from_values(
            std::env::var("GEER_AGENT_TRACE_DATABASE").ok(),
            std::env::var("GEER_AGENT_TRACE_DATABASE_URL").ok(),
        )
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
        Ok(config)
    }

    fn from_values(
        api_key: Option<String>,
        model: Option<String>,
        base_url: Option<String>,
        api: Option<String>,
        tools_enabled: Option<String>,
        bash_bin: Option<String>,
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

        let bash_bin = bash_bin
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .map_or_else(
                || Ok(PathBuf::from("bash")),
                |value| {
                    let path = PathBuf::from(value);
                    if path.is_absolute() {
                        Ok(path)
                    } else {
                        Err("GEER_AGENT_BASH_BIN 必须是 Bash 可执行文件的绝对路径。".to_owned())
                    }
                },
            )?;

        Ok(Self {
            api_key,
            model,
            base_url,
            api,
            tools_enabled,
            bash_bin,
            limits: ResourceLimits::default(),
            trace_database: None,
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
    use super::{
        Config, DEFAULT_BASE_URL, OpenAiApi, ResourceLimits, TraceDatabase, TraceDatabaseConfig,
    };

    #[test]
    fn trace_database_config_requires_selected_backend_and_matching_url() {
        assert!(
            TraceDatabaseConfig::from_values(None, None)
                .unwrap()
                .is_none()
        );
        assert!(TraceDatabaseConfig::from_values(None, Some("sqlite::memory:".into())).is_err());
        for (database, url, expected) in [
            ("sqlite", "sqlite::memory:", TraceDatabase::Sqlite),
            (
                "postgres",
                "postgres://localhost/trace",
                TraceDatabase::Postgres,
            ),
            ("mysql", "mysql://localhost/trace", TraceDatabase::Mysql),
            (
                "mongodb",
                "mongodb://localhost/trace",
                TraceDatabase::Mongodb,
            ),
        ] {
            let config = TraceDatabaseConfig::from_values(Some(database.into()), Some(url.into()))
                .unwrap()
                .unwrap();
            assert_eq!(config.kind, expected);
            assert!(!format!("{config:?}").contains(url));
        }
        assert!(TraceDatabaseConfig::from_values(Some("other".into()), Some("x".into())).is_err());
        assert!(TraceDatabaseConfig::from_values(Some("mysql".into()), None).is_err());
        assert!(
            TraceDatabaseConfig::from_values(Some("mysql".into()), Some("sqlite::memory:".into()))
                .is_err()
        );
        assert!(
            TraceDatabaseConfig::from_values(
                Some("mongodb".into()),
                Some("mongodb://localhost/".into())
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_missing_api_key() {
        let error = Config::from_values(None, Some("gpt-test".to_owned()), None, None, None, None)
            .expect_err("缺少 key 时应报错");

        assert!(error.contains("OPENAI_API_KEY"));
    }

    #[test]
    fn rejects_missing_model() {
        let error = Config::from_values(Some("secret".to_owned()), None, None, None, None, None)
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
            None,
        )
        .expect("key 与 model 已提供");

        assert_eq!(config.api_key, "secret");
        assert_eq!(config.model, "gpt-test");
        assert_eq!(config.base_url, DEFAULT_BASE_URL);
        assert_eq!(config.api, OpenAiApi::Responses);
        assert!(config.tools_enabled);
        assert_eq!(config.bash_bin, std::path::Path::new("bash"));
    }

    #[test]
    fn accepts_an_overridden_base_url() {
        let config = Config::from_values(
            Some("secret".to_owned()),
            Some("gpt-test".to_owned()),
            Some("https://proxy.example/v1".to_owned()),
            Some("chat-completions".to_owned()),
            Some("off".to_owned()),
            Some("/usr/bin/bash".to_owned()),
        )
        .expect("自定义 endpoint 应被接受");

        assert_eq!(config.base_url, "https://proxy.example/v1");
        assert_eq!(config.api, OpenAiApi::ChatCompletions);
        assert!(!config.tools_enabled);
        assert_eq!(config.bash_bin, std::path::Path::new("/usr/bin/bash"));
    }

    #[test]
    fn rejects_unknown_api() {
        let error = Config::from_values(
            Some("secret".to_owned()),
            Some("gpt-test".to_owned()),
            None,
            Some("other".to_owned()),
            None,
            None,
        )
        .expect_err("未知接口应在进入 REPL 前报错");
        assert!(error.contains("OPENAI_API"));
    }

    #[test]
    fn rejects_relative_bash_path() {
        let error = Config::from_values(
            Some("secret".to_owned()),
            Some("gpt-test".to_owned()),
            None,
            None,
            None,
            Some("bin/bash".to_owned()),
        )
        .expect_err("自定义 Bash 路径必须为绝对路径");
        assert!(error.contains("GEER_AGENT_BASH_BIN"));
    }

    #[test]
    fn resource_limits_validate_values_and_price_pair() {
        let values = [
            Some("100".to_owned()),
            Some("50".to_owned()),
            Some("0.25".to_owned()),
            Some("1.5".to_owned()),
            Some("3".to_owned()),
            Some("30".to_owned()),
        ];
        let limits = ResourceLimits::from_values(values).expect("有效资源配置");
        assert_eq!(limits.max_input_tokens, Some(100));
        assert_eq!(limits.max_duration.as_secs(), 30);
        assert!(limits.requires_usage());
        for (index, bad) in [(0, "0"), (2, "NaN"), (3, "-1"), (5, "0")] {
            let mut values = [None, None, None, None, None, None];
            values[index] = Some(bad.to_owned());
            assert!(
                ResourceLimits::from_values(values).is_err(),
                "index={index}"
            );
        }
        let missing_rate = [None, None, Some("1".to_owned()), None, None, None];
        assert!(ResourceLimits::from_values(missing_rate).is_err());
    }
}
