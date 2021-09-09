use actix_web::http::StatusCode;
use actix_web::ResponseError;
use actix_web::{web, HttpResponse};
use anyhow::Context;
use sqlx::PgPool;

use uuid::Uuid;
#[derive(serde::Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}

#[derive(thiserror::Error)]
pub enum SubscriptionConfirmError {
    #[error("Unauthorized access")]
    UnAuthorizedError,
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for SubscriptionConfirmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

#[tracing::instrument(name = "Confirm a pending subscriber", skip(parameters, pool))]
pub async fn confirm(
    parameters: web::Query<Parameters>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, SubscriptionConfirmError> {
    let id = get_subscriber_id_from_token(&pool, &parameters.subscription_token)
        .await
        .context("failed to get the subscriber id from the database")?;
    let subscriber_id = id.ok_or_else(|| SubscriptionConfirmError::UnAuthorizedError)?;
    confirm_subscriber(&pool, subscriber_id)
        .await
        .context("failed to confirm the subscriber in the database")?;

    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(name = "Mark subscriber as confirmed", skip(subscriber_id, pool))]
pub async fn confirm_subscriber(pool: &PgPool, subscriber_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE subscriptions SET status = 'confirmed' WHERE id = $1"#,
        subscriber_id,
    )
    .execute(pool)
    .await
    .map_err(|e| e)?;
    Ok(())
}

#[tracing::instrument(name = "Get subscriber_id from token", skip(subscription_token, pool))]
pub async fn get_subscriber_id_from_token(
    pool: &PgPool,
    subscription_token: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        r#"SELECT subscriber_id FROM subscription_tokens WHERE subscription_token = $1"#,
        subscription_token,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e)?;
    Ok(result.map(|r| r.subscriber_id))
}

impl ResponseError for SubscriptionConfirmError {
    fn status_code(&self) -> StatusCode {
        match self {
            SubscriptionConfirmError::UnAuthorizedError => StatusCode::UNAUTHORIZED,
            SubscriptionConfirmError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
