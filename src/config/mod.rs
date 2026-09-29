//! 配置层：可被任意业务模块引用。本模块不依赖其它业务模块。

mod path;

pub(crate) use path::{bash_arg, from_msys, msys_style, plain_path};

use std::{
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MAX_DURATION: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UiMode {
    Auto,
    Gui,
    Tui,
    Repl,
}

impl UiMode {
    pub(crate) fn parse(value: Option<&str>) -> Result<Self, String> {
        match value.map(str::trim) {
            None | Some("") | Some("auto") => Ok(Self::Auto),
            Some("gui") => Ok(Self::Gui),
            Some("tui") => Ok(Self::Tui),
            Some("repl") => Ok(Self::Repl),
            Some(_) => Err("GEER_AGENT_UI 只能是 auto、gui、tui 或 repl。".to_owned()),
        }
    }
}
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

#[derive(Clone, PartialEq, Eq)]
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
    fn from_values(
        database: Option<String>,
        url: Option<String>,
        program_dir: &Path,
    ) -> Result<Self, String> {
        let database = database
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("sqlite");
        let kind = match database {
            "sqlite" => TraceDatabase::Sqlite,
            "postgres" => TraceDatabase::Postgres,
            "mysql" => TraceDatabase::Mysql,
            "mongodb" => TraceDatabase::Mongodb,
            _ => {
                return Err(
                    "GEER_AGENT_DATABASE 只能是 sqlite、postgres、mysql 或 mongodb。".to_owned(),
                );
            }
        };
        let url = match url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(url) => url.to_owned(),
            None if kind == TraceDatabase::Sqlite => default_database_url(program_dir)?,
            None => return Err("选择非 SQLite 数据库时必须设置 GEER_AGENT_DATABASE_URL。".into()),
        };
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
            return Err("GEER_AGENT_DATABASE_URL 与数据库类型的协议不匹配。".to_owned());
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
                return Err("MongoDB URI 必须包含数据库名。".to_owned());
            }
        }
        Ok(Self { kind, url })
    }
}

fn default_database_url(program_dir: &Path) -> Result<String, String> {
    let path = program_dir.join(".db").join("geer.sqlite");
    let path = path
        .to_str()
        .ok_or("默认 SQLite 路径包含非 UTF-8 字符，无法生成数据库 URL。")?;
    // SQLite URL 会解码百分号并用问号分隔参数，目录名需先转义。
    let path = path
        .replace('%', "%25")
        .replace('?', "%3F")
        .replace('#', "%23");
    Ok(format!("sqlite://{path}?mode=rwc"))
}

#[derive(Default)]
struct DatabaseInputs {
    common_kind: Option<String>,
    common_url: Option<String>,
    trace_enabled: Option<String>,
    session_enabled: Option<String>,
}

impl DatabaseInputs {
    fn from_env() -> Self {
        Self {
            common_kind: std::env::var("GEER_AGENT_DATABASE").ok(),
            common_url: std::env::var("GEER_AGENT_DATABASE_URL").ok(),
            trace_enabled: std::env::var("GEER_AGENT_TRACE").ok(),
            session_enabled: std::env::var("GEER_AGENT_SESSION_PERSISTENCE").ok(),
        }
    }

    fn resolve(
        self,
        program_dir: &Path,
    ) -> Result<(Option<TraceDatabaseConfig>, Option<TraceDatabaseConfig>), String> {
        let database =
            TraceDatabaseConfig::from_values(self.common_kind, self.common_url, program_dir)?;
        Ok((
            parse_on_off(self.trace_enabled, "GEER_AGENT_TRACE")?.then(|| database.clone()),
            parse_on_off(self.session_enabled, "GEER_AGENT_SESSION_PERSISTENCE")?
                .then_some(database),
        ))
    }
}

fn parse_on_off(value: Option<String>, name: &str) -> Result<bool, String> {
    match value.as_deref().map(str::trim) {
        None | Some("") | Some("on") => Ok(true),
        Some("off") => Ok(false),
        _ => Err(format!("{name} 只能是 on 或 off。")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CompactionConfig {
    pub(crate) context_window_tokens: u64,
    pub(crate) auto: bool,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            context_window_tokens: 272_000,
            auto: true,
        }
    }
}

impl CompactionConfig {
    fn from_values(window: Option<String>, auto: Option<String>) -> Result<Self, String> {
        let context_window_tokens =
            parse_positive_u64(window, "GEER_AGENT_CONTEXT_WINDOW_TOKENS")?.unwrap_or(272_000);
        if context_window_tokens < 1_024 {
            return Err("GEER_AGENT_CONTEXT_WINDOW_TOKENS 不得小于 1024。".to_owned());
        }
        let auto = match auto.as_deref().map(str::trim) {
            None | Some("") | Some("on") => true,
            Some("off") => false,
            _ => return Err("GEER_AGENT_AUTO_COMPACT 只能是 on 或 off。".to_owned()),
        };
        Ok(Self {
            context_window_tokens,
            auto,
        })
    }

    pub(crate) fn trigger_tokens(self) -> u64 {
        self.context_window_tokens.saturating_mul(9) / 10
    }

    pub(crate) fn recent_tokens(self) -> u64 {
        20_000.min(self.context_window_tokens / 4)
    }

    pub(crate) fn summary_tokens(self) -> u64 {
        4_096.min(self.context_window_tokens / 16)
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
    pub(crate) session_database: Option<TraceDatabaseConfig>,
    pub(crate) compaction: CompactionConfig,
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

/// 未设置 `GEER_AGENT_BASH_BIN` 时选择 Bash。
/// Windows 的 PATH 常先命中 System32 里的 WSL `bash.exe`，所以先用
/// `C:\Program Files\Git\bin\bash.exe`，再试其它常见 Git 安装位置，PATH 中的 `bash` 最后。
/// 非 Windows 直接使用 PATH 中的 `bash`。
pub(crate) fn default_bash_bin() -> PathBuf {
    bash_candidates()
        .into_iter()
        .find(|path| !path.is_absolute() || path.is_file())
        .unwrap_or_else(|| PathBuf::from("bash"))
}

fn bash_candidates() -> Vec<PathBuf> {
    if !cfg!(windows) {
        return vec![PathBuf::from("bash")];
    }

    let mut paths = vec![PathBuf::from(r"C:\Program Files\Git\bin\bash.exe")];
    paths.extend(
        ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"]
            .into_iter()
            .filter_map(std::env::var_os)
            .map(PathBuf::from)
            .chain(std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("Programs")))
            .map(|dir| dir.join("Git").join("bin").join("bash.exe")),
    );
    paths.push(PathBuf::from("bash"));
    paths
}

fn user_home() -> Option<PathBuf> {
    #[cfg(windows)]
    let name = "USERPROFILE";
    #[cfg(not(windows))]
    let name = "HOME";
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

fn local_project_dir(executable: &Path) -> Option<PathBuf> {
    #[cfg(feature = "embed-env")]
    {
        let _ = executable;
        None
    }

    #[cfg(not(feature = "embed-env"))]
    {
        let project_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let target_dir = option_env!("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| project_dir.join("target"));
        let target_dir = if target_dir.is_absolute() {
            target_dir
        } else {
            project_dir.join(target_dir)
        };
        executable.starts_with(target_dir).then_some(project_dir)
    }
}

fn existing_env(dir: &Path) -> io::Result<Option<PathBuf>> {
    let file = dir.join(".env");
    match fs::metadata(&file) {
        Ok(_) => Ok(Some(file)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn selected_env(
    exe_dir: &Path,
    project_dir: Option<&Path>,
    home: Option<&Path>,
) -> io::Result<(PathBuf, Option<PathBuf>)> {
    if let Some(file) = existing_env(exe_dir)? {
        return Ok((exe_dir.to_path_buf(), Some(file)));
    }
    if let Some(project_dir) = project_dir
        && let Some(file) = existing_env(project_dir)?
    {
        return Ok((project_dir.to_path_buf(), Some(file)));
    }

    let home = home.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "无法确定用户主目录，不能定位 ~/.geer-agent/.env",
        )
    })?;
    let program_dir = home.join(".geer-agent");
    let file = existing_env(&program_dir)?;
    Ok((program_dir, file))
}

pub(crate) fn load_environment() -> Result<PathBuf, Box<dyn Error>> {
    let executable = std::env::current_exe()?;
    let exe_dir = executable
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "无法确定可执行文件所在目录"))?;
    let project_dir = local_project_dir(&executable);
    let home = user_home();
    let (program_dir, env_file) = selected_env(exe_dir, project_dir.as_deref(), home.as_deref())?;
    if let Some(env_file) = env_file {
        dotenvy::from_path(env_file)?;
    }

    #[cfg(feature = "embed-env")]
    dotenvy::from_read(EMBEDDED_ENV.as_bytes())?;

    Ok(program_dir)
}

impl Config {
    pub(crate) fn load() -> Result<Self, Box<dyn Error>> {
        let program_dir = load_environment()?;

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
        (config.trace_database, config.session_database) = DatabaseInputs::from_env()
            .resolve(&program_dir)
            .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
        config.compaction = CompactionConfig::from_values(
            std::env::var("GEER_AGENT_CONTEXT_WINDOW_TOKENS").ok(),
            std::env::var("GEER_AGENT_AUTO_COMPACT").ok(),
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
                || Ok(default_bash_bin()),
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
            session_database: None,
            compaction: CompactionConfig::default(),
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
        CompactionConfig, Config, DEFAULT_BASE_URL, DatabaseInputs, OpenAiApi, ResourceLimits,
        TraceDatabase, TraceDatabaseConfig, UiMode, default_database_url, local_project_dir,
        selected_env,
    };

    #[test]
    fn ui_mode_has_safe_default_and_rejects_unknown_values() {
        assert_eq!(UiMode::parse(None), Ok(UiMode::Auto));
        assert_eq!(UiMode::parse(Some(" gui ")), Ok(UiMode::Gui));
        assert_eq!(UiMode::parse(Some("tui")), Ok(UiMode::Tui));
        assert_eq!(UiMode::parse(Some("repl")), Ok(UiMode::Repl));
        assert!(UiMode::parse(Some("window")).is_err());
    }

    fn program_dir() -> std::path::PathBuf {
        std::env::temp_dir().join("geer-config-tests")
    }

    #[test]
    fn selects_executable_env_before_home_and_ignores_working_dir() {
        let root = std::env::temp_dir().join(format!("geer-config-{}", uuid::Uuid::new_v4()));
        let exe_dir = root.join("bin");
        let home = root.join("home");
        std::fs::create_dir_all(&exe_dir).unwrap();
        std::fs::create_dir_all(home.join(".geer-agent")).unwrap();
        std::fs::write(exe_dir.join(".env"), "OPENAI_MODEL=exe\n").unwrap();
        std::fs::write(home.join(".geer-agent/.env"), "OPENAI_MODEL=home\n").unwrap();
        std::fs::write(root.join(".env"), "OPENAI_MODEL=cwd\n").unwrap();

        assert_eq!(
            selected_env(&exe_dir, None, Some(&home)).unwrap(),
            (exe_dir.clone(), Some(exe_dir.join(".env")))
        );
        std::fs::remove_file(exe_dir.join(".env")).unwrap();
        assert_eq!(
            selected_env(&exe_dir, None, Some(&home)).unwrap(),
            (
                home.join(".geer-agent"),
                Some(home.join(".geer-agent/.env"))
            )
        );
        std::fs::remove_file(home.join(".geer-agent/.env")).unwrap();
        assert_eq!(
            selected_env(&exe_dir, None, Some(&home)).unwrap(),
            (home.join(".geer-agent"), None)
        );
        assert!(selected_env(&exe_dir, None, None).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_cargo_build_can_use_project_env() {
        let root = std::env::temp_dir().join(format!("geer-project-env-{}", uuid::Uuid::new_v4()));
        let exe_dir = root.join("target/debug");
        let home = root.join("home");
        std::fs::create_dir_all(&exe_dir).unwrap();
        std::fs::create_dir_all(home.join(".geer-agent")).unwrap();
        std::fs::write(root.join(".env"), "OPENAI_MODEL=project\n").unwrap();
        std::fs::write(home.join(".geer-agent/.env"), "OPENAI_MODEL=home\n").unwrap();

        assert_eq!(
            selected_env(&exe_dir, Some(&root), Some(&home)).unwrap(),
            (root.clone(), Some(root.join(".env")))
        );
        let built_executable = std::env::current_exe().unwrap();
        #[cfg(not(feature = "embed-env"))]
        assert!(local_project_dir(&built_executable).is_some());
        #[cfg(feature = "embed-env")]
        assert!(local_project_dir(&built_executable).is_none());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn database_defaults_and_shared_override() {
        let (trace, session) = DatabaseInputs::default().resolve(&program_dir()).unwrap();
        assert_eq!(trace, session);
        assert_eq!(
            trace.unwrap().url,
            default_database_url(&program_dir()).unwrap()
        );

        let (trace, session) = DatabaseInputs {
            common_kind: Some("sqlite".into()),
            common_url: Some("sqlite://custom.sqlite?mode=rwc".into()),
            ..Default::default()
        }
        .resolve(&program_dir())
        .unwrap();
        assert_eq!(trace, session);
        assert_eq!(trace.unwrap().url, "sqlite://custom.sqlite?mode=rwc");
    }

    #[test]
    fn default_database_url_keeps_special_directory_characters() {
        let program_dir = program_dir().join("100%?#");
        let url = default_database_url(&program_dir).unwrap();
        let options: sea_orm::sqlx::sqlite::SqliteConnectOptions = url.parse().unwrap();
        assert_eq!(options.get_filename(), program_dir.join(".db/geer.sqlite"));
    }

    #[test]
    fn database_independent_switches() {
        let (trace, session) = DatabaseInputs {
            trace_enabled: Some("off".into()),
            ..Default::default()
        }
        .resolve(&program_dir())
        .unwrap();
        assert!(trace.is_none() && session.is_some());
        let (trace, session) = DatabaseInputs {
            session_enabled: Some("off".into()),
            ..Default::default()
        }
        .resolve(&program_dir())
        .unwrap();
        assert!(trace.is_some() && session.is_none());
    }

    #[test]
    fn database_rejects_invalid_explicit_values() {
        assert!(
            DatabaseInputs {
                common_kind: Some("postgres".into()),
                ..Default::default()
            }
            .resolve(&program_dir())
            .unwrap_err()
            .contains("GEER_AGENT_DATABASE_URL")
        );
        assert!(
            DatabaseInputs {
                common_url: Some("postgres://localhost/db".into()),
                ..Default::default()
            }
            .resolve(&program_dir())
            .is_err()
        );
        assert!(
            DatabaseInputs {
                trace_enabled: Some("no".into()),
                ..Default::default()
            }
            .resolve(&program_dir())
            .is_err()
        );
        assert!(
            DatabaseInputs {
                common_kind: Some("unknown".into()),
                ..Default::default()
            }
            .resolve(&program_dir())
            .is_err()
        );
    }

    #[test]
    fn compaction_defaults_and_overrides() {
        let defaults = CompactionConfig::from_values(None, None).unwrap();
        assert_eq!(defaults.context_window_tokens, 272_000);
        assert_eq!(defaults.trigger_tokens(), 244_800);
        assert_eq!(defaults.recent_tokens(), 20_000);
        assert_eq!(defaults.summary_tokens(), 4_096);
        assert!(defaults.auto);

        let small = CompactionConfig::from_values(Some("4096".into()), Some("off".into())).unwrap();
        assert_eq!(small.trigger_tokens(), 3_686);
        assert_eq!(small.recent_tokens(), 1_024);
        assert_eq!(small.summary_tokens(), 256);
        assert!(!small.auto);
        assert!(CompactionConfig::from_values(Some("1023".into()), None).is_err());
        assert!(CompactionConfig::from_values(Some("invalid".into()), None).is_err());
        assert!(CompactionConfig::from_values(None, Some("yes".into())).is_err());
    }

    #[test]
    fn database_config_accepts_supported_backends_and_requires_matching_url() {
        assert_eq!(
            TraceDatabaseConfig::from_values(None, None, &program_dir())
                .unwrap()
                .url,
            default_database_url(&program_dir()).unwrap()
        );
        assert_eq!(
            TraceDatabaseConfig::from_values(None, Some("sqlite::memory:".into()), &program_dir())
                .unwrap()
                .kind,
            TraceDatabase::Sqlite
        );
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
            let config = TraceDatabaseConfig::from_values(
                Some(database.into()),
                Some(url.into()),
                &program_dir(),
            )
            .unwrap();
            assert_eq!(config.kind, expected);
            assert!(!format!("{config:?}").contains(url));
        }
        assert!(
            TraceDatabaseConfig::from_values(
                Some("other".into()),
                Some("x".into()),
                &program_dir()
            )
            .is_err()
        );
        assert!(
            TraceDatabaseConfig::from_values(Some("mysql".into()), None, &program_dir()).is_err()
        );
        assert!(
            TraceDatabaseConfig::from_values(
                Some("mysql".into()),
                Some("sqlite::memory:".into()),
                &program_dir()
            )
            .is_err()
        );
        assert!(
            TraceDatabaseConfig::from_values(
                Some("mongodb".into()),
                Some("mongodb://localhost/".into()),
                &program_dir()
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
        assert_eq!(config.bash_bin, super::default_bash_bin());
    }

    #[test]
    fn default_bash_prefers_git_bash_then_path_on_windows() {
        let candidates = super::bash_candidates();
        if cfg!(windows) {
            assert_eq!(
                candidates.first().map(std::path::PathBuf::as_path),
                Some(std::path::Path::new(r"C:\Program Files\Git\bin\bash.exe"))
            );
            assert_eq!(
                candidates.last().map(std::path::PathBuf::as_path),
                Some(std::path::Path::new("bash"))
            );
            let chosen = super::default_bash_bin();
            let preferred = std::path::Path::new(r"C:\Program Files\Git\bin\bash.exe");
            if preferred.is_file() {
                assert_eq!(chosen, preferred);
            }
        } else {
            assert_eq!(candidates, vec![std::path::PathBuf::from("bash")]);
            assert_eq!(super::default_bash_bin(), std::path::Path::new("bash"));
        }
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
