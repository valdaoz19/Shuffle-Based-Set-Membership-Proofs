use std::net::SocketAddr;
use std::{fs, path::Path};

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use aml_api::{build_app, new_shared_state};

fn main() -> anyhow::Result<()> {
    load_dotenv_defaults(".env")?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run())
}

async fn run() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aml_api=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let state = new_shared_state().await?;
    let app = build_app(state);

    let addr = bind_addr()?;
    tracing::info!("AML ZKP PoC listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn bind_addr() -> anyhow::Result<SocketAddr> {
    if let Ok(addr) = std::env::var("AML_BIND_ADDR") {
        return Ok(addr.parse()?);
    }
    let port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(8880);
    Ok(SocketAddr::from(([0, 0, 0, 0], port)))
}

fn load_dotenv_defaults(path: impl AsRef<Path>) -> anyhow::Result<()> {
    let path = path.as_ref();
    let Ok(contents) = fs::read_to_string(path) else {
        return Ok(());
    };

    for (idx, raw_line) in contents.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            anyhow::bail!("invalid .env line {}: missing '='", idx + 1);
        };
        let key = key.trim();
        if key.is_empty() {
            anyhow::bail!("invalid .env line {}: empty key", idx + 1);
        }
        if std::env::var_os(key).is_some() {
            continue;
        }

        let value = parse_env_value(value.trim());
        // SAFETY: `.env` is loaded synchronously before the Tokio runtime is
        // started and before any application threads are spawned.
        unsafe {
            std::env::set_var(key, value);
        }
    }

    Ok(())
}

fn parse_env_value(value: &str) -> String {
    let value = strip_inline_comment(value).trim();
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
        {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

fn strip_inline_comment(value: &str) -> &str {
    let mut quote = None;
    let mut previous = '\0';
    for (idx, ch) in value.char_indices() {
        match ch {
            '"' | '\'' if previous != '\\' => {
                quote = if quote == Some(ch) {
                    None
                } else if quote.is_none() {
                    Some(ch)
                } else {
                    quote
                };
            }
            '#' if quote.is_none() => return &value[..idx],
            _ => {}
        }
        previous = ch;
    }
    value
}

#[cfg(test)]
mod tests {
    use super::parse_env_value;

    #[test]
    fn parses_env_values_with_quotes_and_comments() {
        assert_eq!(parse_env_value("plain"), "plain");
        assert_eq!(parse_env_value("plain # comment"), "plain");
        assert_eq!(parse_env_value("\"quoted # value\""), "quoted # value");
        assert_eq!(parse_env_value("'single quoted'"), "single quoted");
    }
}
