use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub struct AppError {
    status: StatusCode,
    source: anyhow::Error,
}

impl AppError {
    pub fn not_found(msg: String) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            source: anyhow::anyhow!(msg),
        }
    }

    #[allow(dead_code)]
    pub fn internal(source: impl Into<anyhow::Error>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            source: source.into(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self.status {
            StatusCode::NOT_FOUND => {
                tracing::warn!("not found: {:?}", self.source);
            }
            _ => {
                tracing::error!("handler error: {:?}", self.source);
            }
        }
        (self.status, self.source.to_string()).into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(e: E) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            source: e.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
