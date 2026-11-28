//! Model files for language-model units (spec 0017), served from this coordinator's own
//! origin so the site's Content-Security-Policy keeps `connect-src 'self'`. Only files the
//! catalog lists are served; the browser checks each one's SHA-256 before using it, so
//! neither this server nor a mirror can change what a device runs.

use crate::{AppState, api::AppError};
use axum::{
    body::Body,
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
};
use futures_util::stream;
use protocol::templates::ai::{Model, ModelFile, catalog};
use tokio::io::AsyncReadExt;

/// The catalog entry `path` names: `{model id}/{file path}`.
pub fn lookup(path: &str) -> Option<(&'static Model, &'static ModelFile)> {
    catalog().models.iter().find_map(|model| {
        let file = path.strip_prefix(model.id.as_str())?.strip_prefix('/')?;
        Some((model, model.files.iter().find(|f| f.path == file)?))
    })
}

const CHUNK: usize = 1 << 20;

/// `GET /api/models/{model id}/{file}`.
#[utoipa::path(
    operation_id = "models_file",
    get,
    path = "/api/models/{path}",
    tag = "files",
    params(("path" = String, Path, description = "Path in the catalog")),
    responses((status = 200, description = "A model file of the catalog; the browser checks its SHA-256", body = Vec<u8>, content_type = "application/octet-stream"), (status = 404, description = "Not found, or not visible to the caller", body = protocol::ApiError)),
    security(())
)]
pub async fn file(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Result<Response, AppError> {
    let (model, entry) = lookup(&path).ok_or(AppError::NotFound)?;
    if model.test && !state.config.test_models {
        return Err(AppError::NotFound);
    }
    // Both parts come from the catalog, never from the request.
    let on_disk = state.config.models_dir.join(&model.id).join(&entry.path);
    let Ok(file) = tokio::fs::File::open(&on_disk).await else {
        tracing::warn!(path = %on_disk.display(), "model file missing; run `just models`");
        return Err(AppError::NotFound);
    };
    let len = file.metadata().await.map_err(|_| AppError::NotFound)?.len();
    let body = Body::from_stream(stream::unfold(file, |mut file| async move {
        let mut buf = vec![0; CHUNK];
        match file.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => {
                buf.truncate(n);
                Some((Ok::<_, std::io::Error>(buf), file))
            }
            Err(e) => Some((Err(e), file)),
        }
    }));
    let content_type = if entry.path.ends_with(".json") {
        "application/json"
    } else {
        "application/octet-stream"
    };
    Ok((
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (header::CONTENT_LENGTH, len.to_string()),
            // The browser keeps checked copies in its own cache (Cache Storage).
            (header::CACHE_CONTROL, "no-cache".to_string()),
        ],
        body,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_catalog_files_are_found() {
        let (model, file) =
            lookup("HuggingFaceTB/SmolLM2-360M-Instruct/onnx/model_q4f16.onnx").unwrap();
        assert_eq!(model.id, "HuggingFaceTB/SmolLM2-360M-Instruct");
        assert_eq!(file.path, "onnx/model_q4f16.onnx");
        for path in [
            "HuggingFaceTB/SmolLM2-360M-Instruct/onnx/model.onnx",
            "HuggingFaceTB/SmolLM2-360M-Instruct/../../etc/passwd",
            "HuggingFaceTB/SmolLM2-360M-Instruct",
            "HuggingFaceTB/SmolLM2-360M-Instructconfig.json",
            "someone/else/config.json",
            "",
        ] {
            assert!(lookup(path).is_none(), "{path}");
        }
    }
}
