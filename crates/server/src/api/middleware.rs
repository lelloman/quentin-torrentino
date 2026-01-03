//! Authentication middleware for API routes.

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use std::collections::HashMap;
use std::sync::Arc;
use torrentino_core::AuthRequest;

use crate::state::AppState;

/// Authentication middleware that validates requests using the configured authenticator.
///
/// This middleware extracts credentials from request headers and validates them
/// against the authenticator configured in AppState. If authentication fails,
/// it returns a 401 Unauthorized response.
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let authenticator = state.authenticator();

    // Skip auth check if using NoneAuthenticator
    if authenticator.method_name() == "none" {
        return Ok(next.run(request).await);
    }

    // Extract headers into HashMap for AuthRequest
    let headers: HashMap<String, String> = request
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|v| (name.as_str().to_lowercase(), v.to_string()))
        })
        .collect();

    // Get source IP (default to localhost if not available)
    let source_ip = request
        .extensions()
        .get::<std::net::SocketAddr>()
        .map(|addr| addr.ip())
        .unwrap_or_else(|| std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));

    let auth_request = AuthRequest { headers, source_ip };

    match authenticator.authenticate(&auth_request).await {
        Ok(_identity) => {
            // Authentication successful, continue to the handler
            Ok(next.run(request).await)
        }
        Err(torrentino_core::AuthError::NotAuthenticated) => {
            // No credentials provided
            Err(StatusCode::UNAUTHORIZED)
        }
        Err(torrentino_core::AuthError::InvalidCredentials(_)) => {
            // Wrong credentials
            Err(StatusCode::UNAUTHORIZED)
        }
        Err(_) => {
            // Other auth errors (service unavailable, config error)
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{header, Request},
        middleware,
        routing::get,
        Router,
    };
    use std::sync::Arc;
    use torrentino_core::{
        create_audit_system, ApiKeyAuthenticator, AuthConfig, AuthMethod, Config,
        DatabaseConfig, SqliteAuditStore, SqliteCatalog, SqliteTicketStore,
    };
    use crate::api::WsBroadcaster;
    use torrentino_core::config::ServerConfig;
    use torrentino_core::textbrain::TextBrainConfig;
    use torrentino_core::orchestrator::OrchestratorConfig;
    use tower::ServiceExt;

    async fn dummy_handler() -> &'static str {
        "OK"
    }

    fn create_test_config(auth_config: AuthConfig) -> Config {
        Config {
            auth: auth_config,
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            searcher: None,
            torrent_client: None,
            textbrain: TextBrainConfig::default(),
            orchestrator: OrchestratorConfig::default(),
            external_catalogs: None,
        }
    }

    async fn create_test_state(auth_config: AuthConfig) -> Arc<AppState> {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let authenticator: Arc<dyn torrentino_core::Authenticator> = match auth_config.method {
            AuthMethod::None => Arc::new(torrentino_core::NoneAuthenticator::new()),
            AuthMethod::ApiKey => {
                Arc::new(ApiKeyAuthenticator::new(auth_config.api_key.clone().unwrap()))
            }
        };

        let audit_store =
            Arc::new(SqliteAuditStore::new(&db_path).unwrap()) as Arc<dyn torrentino_core::AuditStore>;
        let (audit_handle, _writer) = create_audit_system(audit_store.clone(), 100);
        let ticket_store =
            Arc::new(SqliteTicketStore::new(&db_path).unwrap()) as Arc<dyn torrentino_core::TicketStore>;
        let catalog = Arc::new(SqliteCatalog::new(&db_path).unwrap()) as Arc<dyn torrentino_core::TorrentCatalog>;

        // Leak the temp_dir to keep the database around
        std::mem::forget(temp_dir);

        Arc::new(AppState::new(
            create_test_config(auth_config),
            authenticator,
            audit_handle,
            audit_store,
            ticket_store,
            None,
            None,
            catalog,
            None,
            None,
            None, // external_catalog
            WsBroadcaster::default(),
            torrentino_core::EncoderCapabilities::default(),
        ))
    }

    #[tokio::test]
    async fn test_none_auth_allows_all() {
        let state = create_test_state(AuthConfig {
            method: AuthMethod::None,
            api_key: None,
        })
        .await;

        let app = Router::new()
            .route("/test", get(dummy_handler))
            .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
            .with_state(state);

        let request = Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_api_key_auth_valid() {
        let state = create_test_state(AuthConfig {
            method: AuthMethod::ApiKey,
            api_key: Some("secret-key".to_string()),
        })
        .await;

        let app = Router::new()
            .route("/test", get(dummy_handler))
            .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
            .with_state(state);

        let request = Request::builder()
            .uri("/test")
            .header(header::AUTHORIZATION, "Bearer secret-key")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_api_key_auth_invalid() {
        let state = create_test_state(AuthConfig {
            method: AuthMethod::ApiKey,
            api_key: Some("secret-key".to_string()),
        })
        .await;

        let app = Router::new()
            .route("/test", get(dummy_handler))
            .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
            .with_state(state);

        let request = Request::builder()
            .uri("/test")
            .header(header::AUTHORIZATION, "Bearer wrong-key")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_api_key_auth_missing() {
        let state = create_test_state(AuthConfig {
            method: AuthMethod::ApiKey,
            api_key: Some("secret-key".to_string()),
        })
        .await;

        let app = Router::new()
            .route("/test", get(dummy_handler))
            .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
            .with_state(state);

        let request = Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_x_api_key_header() {
        let state = create_test_state(AuthConfig {
            method: AuthMethod::ApiKey,
            api_key: Some("secret-key".to_string()),
        })
        .await;

        let app = Router::new()
            .route("/test", get(dummy_handler))
            .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
            .with_state(state);

        let request = Request::builder()
            .uri("/test")
            .header("X-API-Key", "secret-key")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
