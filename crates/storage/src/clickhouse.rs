use anyhow::{Context, Result};
use clickhouse::Client;
use url::Url;

use crate::migrations;

pub fn client(url: &str, database: &str) -> Client {
    client_with_auth(url).with_database(database)
}

pub fn admin_client(url: &str) -> Client {
    client_with_auth(url)
}

fn client_with_auth(raw_url: &str) -> Client {
    let Ok(mut parsed) = Url::parse(raw_url) else {
        return Client::default().with_url(raw_url);
    };

    let user = parsed.username().to_string();
    let password = parsed.password().unwrap_or("").to_string();
    if user.is_empty() {
        return Client::default().with_url(parsed.as_str());
    }

    if !user.is_empty() {
        let _ = parsed.set_username("");
        let _ = parsed.set_password(None);
    }

    Client::default()
        .with_url(parsed.as_str())
        .with_user(user)
        .with_password(password)
}

pub async fn assert_clickhouse_ready(client: &Client) -> Result<()> {
    client
        .query("SELECT 1")
        .execute()
        .await
        .context("ClickHouse readiness check failed")
}

pub async fn run_migrations(url: &str, database: &str) -> Result<Client> {
    let admin = admin_client(url);
    admin
        .query("CREATE DATABASE IF NOT EXISTS ?")
        .bind(clickhouse::sql::Identifier(database))
        .execute()
        .await
        .with_context(|| format!("creating ClickHouse database {database}"))?;

    let client = client(url, database);
    assert_clickhouse_ready(&client).await?;

    for (name, sql) in migrations::all_ordered_sql_for_database(database) {
        client
            .query(&sql)
            .execute()
            .await
            .with_context(|| format!("running ClickHouse migration {name}"))?;
    }

    Ok(client)
}
