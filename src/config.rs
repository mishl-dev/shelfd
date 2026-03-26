use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::state::ExploreSubject;

#[derive(Debug, Clone, Copy, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum LogStyle {
    Compact,
    Pretty,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppConfig {
    pub database_url: String,
    pub bind_addr: String,
    pub archive_base: String,
    pub archive_name: String,
    pub app_name: String,
    pub metadata_base_url: String,
    pub flaresolverr_url: String,
    pub flaresolverr_session: String,
    pub public_base_url: Option<String>,
    pub rust_log: String,
    pub log_style: LogStyle,
    pub search_cache_ttl_secs: i64,
    pub book_cache_ttl_secs: i64,
    pub link_cache_ttl_secs: i64,
    pub link_failure_ttl_secs: i64,
    pub explore_cache_ttl_secs: i64,
    pub cover_negative_ttl_secs: i64,
    pub search_result_limit: usize,
    pub explore_page_size: usize,
    pub cover_lookup_limit: usize,
    pub inline_info_concurrency: usize,
    pub cover_lookup_concurrency: usize,
    pub search_prewarm_count: usize,
    pub upstream_retry_attempts: usize,
    pub upstream_retry_backoff_ms: u64,
    pub cleanup_interval_secs: u64,
    pub explore_subjects_raw: String,
}

#[derive(Debug, Parser)]
#[command(
    name = "shelfd",
    version,
    about = "Self-hosted OPDS bridge for book archives"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start the OPDS server.
    Serve(ServeArgs),
    /// Print the effective runtime config as JSON.
    PrintConfig(ServeArgs),
}

#[derive(Debug, Clone, Args, Default)]
pub struct ServeArgs {
    /// Override the SQLite database URL.
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: Option<String>,
    /// Override the bind address, for example 0.0.0.0:7451.
    #[arg(long, env = "BIND_ADDR")]
    pub bind_addr: Option<String>,
    /// Override the archive base URL.
    #[arg(long, env = "ARCHIVE_BASE")]
    pub archive_base: Option<String>,
    /// Override the archive display name used in OPDS feeds.
    #[arg(long, env = "ARCHIVE_NAME")]
    pub archive_name: Option<String>,
    /// Override the app name used in OPDS feeds.
    #[arg(long, env = "APP_NAME")]
    pub app_name: Option<String>,
    /// Override the metadata provider base URL.
    #[arg(long, env = "METADATA_BASE_URL")]
    pub metadata_base_url: Option<String>,
    /// Override the FlareSolverr base URL.
    #[arg(long, env = "FLARESOLVERR_URL")]
    pub flaresolverr_url: Option<String>,
    /// Override the FlareSolverr session name.
    #[arg(long, env = "FLARESOLVERR_SESSION")]
    pub flaresolverr_session: Option<String>,
    /// Override the public base URL used in OPDS links.
    #[arg(long, env = "PUBLIC_BASE_URL")]
    pub public_base_url: Option<String>,
    /// Override the log filter, for example info,shelfd=debug.
    #[arg(long, env = "RUST_LOG")]
    pub rust_log: Option<String>,
    /// Choose the terminal log formatter.
    #[arg(long, env = "LOG_STYLE", value_enum)]
    pub log_style: Option<LogStyle>,
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn parse_env<T>(key: &str, default: T) -> Result<T>
where
    T: std::str::FromStr + Copy,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    match std::env::var(key) {
        Ok(value) => value
            .parse()
            .with_context(|| format!("failed to parse env var {key}={value}")),
        Err(_) => Ok(default),
    }
}

pub fn load_config(args: &ServeArgs) -> Result<AppConfig> {
    Ok(AppConfig {
        database_url: args
            .database_url
            .clone()
            .unwrap_or_else(|| env_or("DATABASE_URL", "sqlite://opds.db?mode=rwc")),
        bind_addr: args
            .bind_addr
            .clone()
            .unwrap_or_else(|| env_or("BIND_ADDR", "0.0.0.0:7070")),
        archive_base: args
            .archive_base
            .clone()
            .unwrap_or_else(|| env_or("ARCHIVE_BASE", "http://localhost:8080")),
        archive_name: args
            .archive_name
            .clone()
            .unwrap_or_else(|| env_or("ARCHIVE_NAME", "Archive")),
        app_name: args
            .app_name
            .clone()
            .unwrap_or_else(|| env_or("APP_NAME", "shelfd")),
        metadata_base_url: args
            .metadata_base_url
            .clone()
            .unwrap_or_else(|| env_or("METADATA_BASE_URL", "https://openlibrary.org")),
        flaresolverr_url: args
            .flaresolverr_url
            .clone()
            .unwrap_or_else(|| env_or("FLARESOLVERR_URL", "http://localhost:8191")),
        flaresolverr_session: args
            .flaresolverr_session
            .clone()
            .unwrap_or_else(|| env_or("FLARESOLVERR_SESSION", "opds-session")),
        public_base_url: args
            .public_base_url
            .clone()
            .or_else(|| std::env::var("PUBLIC_BASE_URL").ok())
            .map(|value| value.trim_end_matches('/').to_owned())
            .filter(|value| !value.is_empty()),
        rust_log: args
            .rust_log
            .clone()
            .unwrap_or_else(|| env_or("RUST_LOG", "info,shelfd=debug,tower_http=warn")),
        log_style: args.log_style.unwrap_or_else(|| {
            std::env::var("LOG_STYLE")
                .ok()
                .and_then(|value| match value.to_ascii_lowercase().as_str() {
                    "pretty" => Some(LogStyle::Pretty),
                    "compact" => Some(LogStyle::Compact),
                    _ => None,
                })
                .unwrap_or(LogStyle::Compact)
        }),
        search_cache_ttl_secs: parse_env("SEARCH_CACHE_TTL_SECS", 1800)?,
        book_cache_ttl_secs: parse_env("BOOK_CACHE_TTL_SECS", 86400)?,
        link_cache_ttl_secs: parse_env("LINK_CACHE_TTL_SECS", 86_400)?,
        link_failure_ttl_secs: parse_env("LINK_FAILURE_TTL_SECS", 900)?,
        explore_cache_ttl_secs: parse_env("EXPLORE_CACHE_TTL_SECS", 21_600)?,
        cover_negative_ttl_secs: parse_env("COVER_NEGATIVE_TTL_SECS", 86_400)?,
        search_result_limit: parse_env("SEARCH_RESULT_LIMIT", 12)?,
        explore_page_size: parse_env("EXPLORE_PAGE_SIZE", 50)?,
        cover_lookup_limit: parse_env("COVER_LOOKUP_LIMIT", 8)?,
        inline_info_concurrency: parse_env("INLINE_INFO_CONCURRENCY", 6)?,
        cover_lookup_concurrency: parse_env("COVER_LOOKUP_CONCURRENCY", 4)?,
        search_prewarm_count: parse_env("SEARCH_PREWARM_COUNT", 3)?,
        upstream_retry_attempts: parse_env("UPSTREAM_RETRY_ATTEMPTS", 2)?,
        upstream_retry_backoff_ms: parse_env("UPSTREAM_RETRY_BACKOFF_MS", 150)?,
        cleanup_interval_secs: parse_env("CACHE_CLEANUP_INTERVAL_SECS", 3600)?,
        explore_subjects_raw: env_or(
            "EXPLORE_SUBJECTS",
            concat!(
                // Arts
                "architecture,art_instruction,art_history,dance,design,fashion,film,",
                "graphic_design,music,music_theory,painting,photography,",
                // Animals
                "bears,cats,kittens,dogs,puppies,",
                // Fiction
                "fantasy,historical_fiction,horror,humor,literature,magic,",
                "mystery_and_detective_stories,plays,poetry,romance,science_fiction,",
                "short_stories,thriller,young_adult_fiction,",
                // Science & Mathematics
                "biology,chemistry,mathematics,physics,programming,",
                // Business & Finance
                "management,entrepreneurship,business_economics,business_success,finance,",
                // Children's
                "kids_books,stories_in_rhyme,baby_books,bedtime_books,picture_books,",
                // History
                "ancient_civilization,archaeology,anthropology,world_war_ii,",
                "social_life_and_customs,",
                // Health & Wellness
                "cooking,cookbooks,mental_health,exercise,nutrition,self_help,",
                // Biography
                "autobiographies,politics_and_government,women,kings_and_rulers,",
                "composers,artists,",
                // Social Sciences
                "religion,political_science,psychology,",
                // Places
                "brazil,india,indonesia,united_states,",
                // Textbooks
                "geography,algebra,education,science,english_language,computer_science",
            ),
        ),
    })
}

pub fn init_tracing(config: &AppConfig) -> Result<()> {
    let env_filter = EnvFilter::try_new(&config.rust_log)
        .with_context(|| format!("invalid RUST_LOG filter: {}", config.rust_log))?;
    let builder = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false);

    match config.log_style {
        LogStyle::Compact => builder.compact().init(),
        LogStyle::Pretty => builder.pretty().init(),
    }

    Ok(())
}

pub fn print_startup_summary(config: &AppConfig) {
    info!(
        "{} starting\n  bind: {}\n  opds: http://{}/opds\n  flaresolverr: {}\n  archive: {} ({})\n  metadata: {}\n  explore subjects: {}\n  log style: {:?}",
        config.app_name,
        config.bind_addr,
        display_host_for_summary(&config.bind_addr),
        config.flaresolverr_url,
        config.archive_name,
        config.archive_base,
        config.metadata_base_url,
        config.explore_subjects_raw,
        config.log_style
    );
}

fn display_host_for_summary(bind_addr: &str) -> String {
    bind_addr
        .strip_prefix("0.0.0.0:")
        .map(|port| format!("127.0.0.1:{port}"))
        .unwrap_or_else(|| bind_addr.to_owned())
}

pub fn parse_explore_subjects(value: &str) -> Vec<ExploreSubject> {
    let mut subjects: Vec<_> = value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|slug| ExploreSubject {
            slug: slug.to_owned(),
            name: prettify_subject_name(slug),
        })
        .collect();

    if subjects.is_empty() {
        subjects.push(ExploreSubject {
            slug: "science_fiction".to_owned(),
            name: "Science Fiction".to_owned(),
        });
    }

    subjects
}

fn prettify_subject_name(slug: &str) -> String {
    match slug {
        "mystery_and_detective_stories" => "Mystery".to_owned(),
        "young_adult_fiction" => "Young Adult Fiction".to_owned(),
        "world_war_ii" => "World War II".to_owned(),
        "kids_books" => "Kids Books".to_owned(),
        "stories_in_rhyme" => "Stories in Rhyme".to_owned(),
        "social_life_and_customs" => "Social Life and Customs".to_owned(),
        "self_help" => "Self-Help".to_owned(),
        "bedtime_books" => "Bedtime Books".to_owned(),
        "baby_books" => "Baby Books".to_owned(),
        "picture_books" => "Picture Books".to_owned(),
        "short_stories" => "Short Stories".to_owned(),
        "business_economics" => "Business Economics".to_owned(),
        "business_success" => "Business Success".to_owned(),
        "english_language" => "English Language".to_owned(),
        "computer_science" => "Computer Science".to_owned(),
        "graphic_design" => "Graphic Design".to_owned(),
        "music_theory" => "Music Theory".to_owned(),
        "art_instruction" => "Art Instruction".to_owned(),
        "art_history" => "Art History".to_owned(),
        "historical_fiction" => "Historical Fiction".to_owned(),
        "young_adult" => "Young Adult".to_owned(),
        "ancient_civilization" => "Ancient Civilization".to_owned(),
        "political_science" => "Political Science".to_owned(),
        "politics_and_government" => "Politics and Government".to_owned(),
        "kings_and_rulers" => "Kings and Rulers".to_owned(),
        _ => slug
            .split('_')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}
