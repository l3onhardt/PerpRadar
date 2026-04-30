use anyhow::{Context, Result};
use clickhouse::Client;

use crate::migrations;

pub fn client(url: &str, database: &str) -> Client {
    Client::default().with_url(url).with_database(database)
}

pub fn admin_client(url: &str) -> Client {
    Client::default().with_url(url)
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

    for (name, sql) in migrations::all_ordered_sql() {
        client
            .query(sql)
            .execute()
            .await
            .with_context(|| format!("running ClickHouse migration {name}"))?;
    }

    Ok(client)
}
