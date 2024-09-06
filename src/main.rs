use gs_slack_bot::server::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter("gs_slack_bot=debug,slack_morphism=debug,gsctl=debug")
        .with_timer(tracing_subscriber::fmt::time::ChronoLocal::rfc_3339())
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    run_slack_server().await?;

    Ok(())
}
