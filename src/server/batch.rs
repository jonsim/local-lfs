//! Git LFS batch negotiation for the basic transfer adapter.
//! The batch response points at this server's raw object PUT and GET routes;
//! no object bytes are transferred in the batch request itself.

use super::http::StatusCode;
use super::store::{ObjectStore, StoreError};

#[derive(Deserialize)]
struct BatchRequest {
    operation: String,
    #[serde(default)]
    transfers: Option<Vec<String>>,
    #[serde(default)]
    hash_algo: Option<String>,
    objects: Vec<RequestedObject>,
    // Optional ref information and future fields are ignored for this local,
    // unauthenticated store; neither affects object identity.
}

#[derive(Deserialize)]
struct RequestedObject {
    oid: String,
    size: u64,
}

#[derive(Serialize)]
struct BatchResponse {
    transfer: &'static str,
    hash_algo: &'static str,
    objects: Vec<BatchObject>,
}

#[derive(Serialize)]
struct BatchObject {
    oid: String,
    size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    actions: Option<Actions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ObjectError>,
}

#[derive(Serialize)]
struct Actions {
    #[serde(skip_serializing_if = "Option::is_none")]
    upload: Option<Action>,
    #[serde(skip_serializing_if = "Option::is_none")]
    download: Option<Action>,
}

#[derive(Serialize)]
struct Action {
    href: String,
}

#[derive(Serialize)]
struct ObjectError {
    code: u16,
    message: &'static str,
}

#[derive(Serialize)]
struct ErrorResponse<'a> {
    message: &'a str,
}

pub struct BatchFailure {
    pub status: StatusCode,
    pub message: &'static str,
}

impl BatchFailure {
    fn new(status: StatusCode, message: &'static str) -> Self {
        Self { status, message }
    }
}

/// Build a batch response for one operation and a list of claimed objects.
/// Missing downloads are reported per object while the whole batch stays 200.
pub fn process(body: &[u8], store: &ObjectStore, base_url: &str) -> Result<Vec<u8>, BatchFailure> {
    let request: BatchRequest = serde_json::from_slice(body)
        .map_err(|_| BatchFailure::new(StatusCode::BadRequest, "Invalid batch JSON"))?;
    if request.operation != "upload" && request.operation != "download" {
        return Err(BatchFailure::new(
            StatusCode::BadRequest,
            "Invalid batch operation",
        ));
    }
    if let Some(transfers) = &request.transfers {
        if !transfers.iter().any(|transfer| transfer == "basic") {
            return Err(BatchFailure::new(
                StatusCode::UnprocessableEntity,
                "Basic transfer is required",
            ));
        }
    }
    if request.hash_algo.as_deref().unwrap_or("sha256") != "sha256" {
        return Err(BatchFailure::new(
            StatusCode::Conflict,
            "Only SHA-256 object IDs are supported",
        ));
    }

    let mut objects = Vec::with_capacity(request.objects.len());
    for object in request.objects {
        // Matching an existing object's size matters: the client will verify
        // the transferred byte count against the size it sent in this batch.
        let result = store.open(&object.oid);
        let (actions, error) = match result {
            Ok((_, actual_size)) if actual_size != object.size => {
                (None, Some(object_error(422, "Object size does not match")))
            }
            Ok(_) if request.operation == "upload" => (None, None),
            Ok(_) => (Some(download_action(base_url, &object.oid)), None),
            Err(StoreError::NotFound) if request.operation == "upload" => {
                (Some(upload_action(base_url, &object.oid)), None)
            }
            Err(StoreError::NotFound) => (None, Some(object_error(404, "Object does not exist"))),
            Err(StoreError::InvalidOid) => (None, Some(object_error(422, "Invalid object ID"))),
            Err(_) => (None, Some(object_error(500, "Object store error"))),
        };
        objects.push(BatchObject {
            oid: object.oid,
            size: object.size,
            actions,
            error,
        });
    }

    serde_json::to_vec(&BatchResponse {
        transfer: "basic",
        hash_algo: "sha256",
        objects,
    })
    .map_err(|_| {
        BatchFailure::new(
            StatusCode::InternalServerError,
            "Could not encode batch response",
        )
    })
}

/// Error bodies use the same vendor JSON media type as successful batches.
pub fn error_body(message: &str) -> Vec<u8> {
    serde_json::to_vec(&ErrorResponse { message })
        .expect("serializing a string-only batch error cannot fail")
}

fn upload_action(base_url: &str, oid: &str) -> Actions {
    Actions {
        upload: Some(Action {
            href: format!("{}/objects/{}", base_url, oid),
        }),
        download: None,
    }
}

fn download_action(base_url: &str, oid: &str) -> Actions {
    Actions {
        upload: None,
        download: Some(Action {
            href: format!("{}/objects/{}", base_url, oid),
        }),
    }
}

fn object_error(code: u16, message: &'static str) -> ObjectError {
    ObjectError { code, message }
}
