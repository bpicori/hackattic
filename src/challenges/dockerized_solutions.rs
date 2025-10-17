use bytes::Bytes;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tokio::fs;
use uuid::Uuid;
use warp::{Filter, http::StatusCode, reply};

// Single storage struct to handle all registry operations
#[derive(Clone)]
struct RegistryStorage {
    root: PathBuf,
    uploads: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    blobs: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    manifests: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl RegistryStorage {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            uploads: Arc::new(RwLock::new(HashMap::new())),
            blobs: Arc::new(RwLock::new(HashMap::new())),
            manifests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn init_upload(&self) -> String {
        let uuid = Uuid::new_v4().to_string();
        self.uploads
            .write()
            .unwrap()
            .insert(uuid.clone(), Vec::new());
        uuid
    }

    fn append_to_upload(&self, uuid: &str, data: &[u8]) -> Result<(), String> {
        let mut uploads = self.uploads.write().unwrap();
        if let Some(buffer) = uploads.get_mut(uuid) {
            buffer.extend_from_slice(data);
            Ok(())
        } else {
            Err("Upload not found".to_string())
        }
    }

    async fn complete_upload(&self, uuid: &str, digest: &str, repo: &str) -> Result<(), String> {
        let data = {
            let mut uploads = self.uploads.write().unwrap();
            uploads.remove(uuid).ok_or("Upload not found")?
        };

        self.blobs
            .write()
            .unwrap()
            .insert(digest.to_string(), data.clone());

        let blob_dir = self.root.join(repo).join("blobs").join("sha256");
        fs::create_dir_all(&blob_dir)
            .await
            .map_err(|e| e.to_string())?;

        let filename = digest.strip_prefix("sha256:").unwrap_or(digest);
        let blob_path = blob_dir.join(filename);
        fs::write(&blob_path, &data)
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn get_blob(&self, digest: &str) -> Option<Vec<u8>> {
        self.blobs.read().unwrap().get(digest).cloned()
    }

    fn blob_exists(&self, digest: &str) -> bool {
        self.blobs.read().unwrap().contains_key(digest)
    }

    fn store_manifest(&self, repo: &str, reference: &str, data: Vec<u8>) {
        let key = format!("{}:{}", repo, reference);
        self.manifests.write().unwrap().insert(key, data);
    }

    fn get_manifest(&self, repo: &str, reference: &str) -> Option<Vec<u8>> {
        let key = format!("{}:{}", repo, reference);
        self.manifests.read().unwrap().get(&key).cloned()
    }
}

// API handlers - organized under RegistryApi struct
struct RegistryApi;

impl RegistryApi {
    fn with_storage(
        storage: RegistryStorage,
    ) -> impl Filter<Extract = (RegistryStorage,), Error = std::convert::Infallible> + Clone {
        warp::any().map(move || storage.clone())
    }

    fn version_check() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("v2").and(warp::get()).map(|| {
            reply::with_header(
                reply::json(&serde_json::json!({})),
                "Docker-Distribution-API-Version",
                "registry/2.0",
            )
        })
    }

    fn start_upload(
        storage: RegistryStorage,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("v2" / String / "blobs" / "uploads")
            .and(warp::post())
            .and(Self::with_storage(storage))
            .map(|repo: String, storage: RegistryStorage| {
                println!("POST /v2/{}/blobs/uploads/", repo);
                let uuid = storage.init_upload();
                let location = format!("/v2/{}/blobs/uploads/{}", repo, uuid);

                reply::with_status(
                    reply::with_header(
                        reply::with_header("", "Location", location),
                        "Docker-Upload-UUID",
                        uuid,
                    ),
                    StatusCode::ACCEPTED,
                )
            })
    }

    fn upload_chunk(
        storage: RegistryStorage,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("v2" / String / "blobs" / "uploads" / String)
            .and(warp::patch())
            .and(warp::body::bytes())
            .and(Self::with_storage(storage))
            .map(
                |repo: String, uuid: String, body: Bytes, storage: RegistryStorage| {
                    println!(
                        "PATCH /v2/{}/blobs/uploads/{} ({} bytes)",
                        repo,
                        uuid,
                        body.len()
                    );

                    match storage.append_to_upload(&uuid, &body) {
                        Ok(_) => {
                            let location = format!("/v2/{}/blobs/uploads/{}", repo, uuid);
                            reply::with_status(
                                reply::with_header("", "Location", location),
                                StatusCode::ACCEPTED,
                            )
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            reply::with_status(
                                reply::with_header("", "Location", ""),
                                StatusCode::NOT_FOUND,
                            )
                        }
                    }
                },
            )
    }

    fn complete_upload(
        storage: RegistryStorage,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("v2" / String / "blobs" / "uploads" / String)
            .and(warp::put())
            .and(warp::query::<HashMap<String, String>>())
            .and(warp::body::bytes())
            .and(Self::with_storage(storage))
            .and_then(
                |repo: String,
                 uuid: String,
                 query: HashMap<String, String>,
                 body: Bytes,
                 storage: RegistryStorage| async move {
                    println!("PUT /v2/{}/blobs/uploads/{}", repo, uuid);

                    if !body.is_empty() {
                        if let Err(e) = storage.append_to_upload(&uuid, &body) {
                            eprintln!("Error: {}", e);
                        }
                    }

                    if let Some(digest) = query.get("digest") {
                        match storage.complete_upload(&uuid, digest, &repo).await {
                            Ok(_) => {
                                let location = format!("/v2/{}/blobs/{}", repo, digest);
                                Ok::<_, warp::Rejection>(reply::with_status(
                                    reply::with_header(
                                        reply::with_header("", "Location", location),
                                        "Docker-Content-Digest",
                                        digest.clone(),
                                    ),
                                    StatusCode::CREATED,
                                ))
                            }
                            Err(e) => {
                                eprintln!("Error: {}", e);
                                Ok::<_, warp::Rejection>(reply::with_status(
                                    reply::with_header(
                                        reply::with_header("", "Location", ""),
                                        "Docker-Content-Digest",
                                        "",
                                    ),
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                ))
                            }
                        }
                    } else {
                        Ok::<_, warp::Rejection>(reply::with_status(
                            reply::with_header(
                                reply::with_header("", "Location", ""),
                                "Docker-Content-Digest",
                                "",
                            ),
                            StatusCode::BAD_REQUEST,
                        ))
                    }
                },
            )
    }

    fn check_blob(
        storage: RegistryStorage,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("v2" / String / "blobs" / String)
            .and(warp::head())
            .and(Self::with_storage(storage))
            .map(|repo: String, digest: String, storage: RegistryStorage| {
                println!("HEAD /v2/{}/blobs/{}", repo, digest);

                if storage.blob_exists(&digest) {
                    reply::with_status(
                        reply::with_header("", "Docker-Content-Digest", digest),
                        StatusCode::OK,
                    )
                } else {
                    reply::with_status(
                        reply::with_header("", "Docker-Content-Digest", ""),
                        StatusCode::NOT_FOUND,
                    )
                }
            })
    }

    fn get_blob(
        storage: RegistryStorage,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("v2" / String / "blobs" / String)
            .and(warp::get())
            .and(Self::with_storage(storage))
            .map(|repo: String, digest: String, storage: RegistryStorage| {
                println!("GET /v2/{}/blobs/{}", repo, digest);

                if let Some(data) = storage.get_blob(&digest) {
                    reply::with_status(
                        reply::with_header(data, "Docker-Content-Digest", digest),
                        StatusCode::OK,
                    )
                } else {
                    reply::with_status(
                        reply::with_header(Vec::new(), "Docker-Content-Digest", ""),
                        StatusCode::NOT_FOUND,
                    )
                }
            })
    }

    fn put_manifest(
        storage: RegistryStorage,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("v2" / String / "manifests" / String)
            .and(warp::put())
            .and(warp::body::bytes())
            .and(Self::with_storage(storage))
            .map(
                |repo: String, reference: String, body: Bytes, storage: RegistryStorage| {
                    println!("PUT /v2/{}/manifests/{}", repo, reference);
                    storage.store_manifest(&repo, &reference, body.to_vec());
                    reply::with_status("", StatusCode::CREATED)
                },
            )
    }

    fn get_manifest(
        storage: RegistryStorage,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("v2" / String / "manifests" / String)
            .and(warp::get())
            .and(Self::with_storage(storage))
            .map(
                |repo: String, reference: String, storage: RegistryStorage| {
                    println!("GET /v2/{}/manifests/{}", repo, reference);

                    if let Some(data) = storage.get_manifest(&repo, &reference) {
                        reply::with_status(data, StatusCode::OK)
                    } else {
                        reply::with_status(Vec::new(), StatusCode::NOT_FOUND)
                    }
                },
            )
    }
}

#[tokio::main]
pub async fn run() {
    let storage = RegistryStorage::new(PathBuf::from("./data/registry_data"));

    let routes = RegistryApi::version_check()
        .or(RegistryApi::start_upload(storage.clone()))
        .or(RegistryApi::upload_chunk(storage.clone()))
        .or(RegistryApi::complete_upload(storage.clone()))
        .or(RegistryApi::check_blob(storage.clone()))
        .or(RegistryApi::get_blob(storage.clone()))
        .or(RegistryApi::put_manifest(storage.clone()))
        .or(RegistryApi::get_manifest(storage));

    println!("Starting Docker Registry on http://0.0.0.0:3030");
    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}
