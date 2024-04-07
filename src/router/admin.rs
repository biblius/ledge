use crate::{
    auth::{Auth, AuthError},
    error::KnawledgeError,
    Documents,
};
use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, get_service, post},
    Router,
};
use axum_extra::{headers::Cookie, TypedHeader};
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};

pub(super) fn admin_router(documents: Documents, auth: Auth) -> Router {
    let auth = Arc::new(auth);

    let router_static = Router::new()
        .nest_service("/", ServeDir::new("public/admin"))
        .with_state(documents.clone())
        .layer(middleware::from_fn_with_state(auth.clone(), session_check));

    let router_admin = Router::new()
        .route("/sync", get(sync))
        .layer(middleware::from_fn_with_state(auth.clone(), session_check));

    let router_auth = admin_auth_router(auth);

    let router = router_admin
        .merge(router_static)
        .with_state(documents)
        .merge(router_auth);

    Router::new().nest("/admin", router)
}

fn admin_auth_router(auth: Arc<Auth>) -> Router {
    Router::new()
        .route(
            "/login",
            get_service(ServeFile::new("public/admin/login.html")),
        )
        .route("/login", post(login))
        .with_state(auth)
}

async fn login(
    auth: axum::extract::State<Arc<Auth>>,
    password: axum::extract::Json<String>,
) -> Result<Response, KnawledgeError> {
    let result = auth.verify_password(&password);

    if !result {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    let session = auth.create_session().await?;
    let cookie = auth.create_session_cookie(session.id);

    Ok((StatusCode::OK, [(header::SET_COOKIE, cookie.to_string())]).into_response())
}

async fn sync(state: axum::extract::State<Documents>) -> Result<impl IntoResponse, KnawledgeError> {
    state.sync().await?;
    Ok(())
}

async fn session_check(
    auth: axum::extract::State<Arc<Auth>>,
    cookie: TypedHeader<Cookie>,
    req: Request,
    next: Next,
) -> Result<impl IntoResponse, KnawledgeError> {
    let cookie = cookie.0.get("SID");

    let Some(cookie) = cookie else {
        return Err(AuthError::NoSession.into());
    };

    let Ok(session_id) = uuid::Uuid::parse_str(cookie) else {
        return Err(AuthError::NoSession.into());
    };

    let valid_exists = auth.session_check(session_id).await?;

    if !valid_exists {
        return Err(AuthError::NoSession.into());
    }

    let response = next.run(req).await;

    Ok(response)
}
