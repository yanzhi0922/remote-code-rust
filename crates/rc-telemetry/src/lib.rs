use anyhow::Result;
use tracing_subscriber::EnvFilter;

/// # Errors
/// Returns an error if the tracing subscriber cannot be initialized.
pub fn install_tracing(service_name: &str, json: bool) -> Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("{service_name}=info")));
    let builder = tracing_subscriber::fmt().with_env_filter(env_filter);
    if json {
        let _ = builder.json().try_init();
    } else {
        let _ = builder.with_target(false).try_init();
    }
    Ok(())
}
