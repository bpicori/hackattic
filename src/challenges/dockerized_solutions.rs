use bytes::Bytes;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use uuid::Uuid;
use warp::{Filter, http::StatusCode, reply};

const REGISTRY_DATA_DIR: &str = "./data/registry_data";
const PORT: u16 = 3030;

// ------ STORAGE
#[derive(Clone)]
struct RegistryStorage {
    root: PathBuf,
}

impl RegistryStorage {
    fn new(root: PathBuf) -> Self {
        Self { root }
    }

    async fn init_upload(&self) -> Result<String, String> {
        let uuid = Uuid::new_v4().to_string();
        let upload_dir = self.root.join("uploads");
        fs::create_dir_all(&upload_dir)
            .await
            .map_err(|e| e.to_string())?;

        let upload_path = upload_dir.join(&uuid);
        fs::write(&upload_path, &[])
            .await
            .map_err(|e| e.to_string())?;

        Ok(uuid)
    }

    async fn append_to_upload(&self, uuid: &str, data: &[u8]) -> Result<(), String> {
        let upload_path = self.root.join("uploads").join(uuid);

        if !upload_path.exists() {
            return Err("Upload not found".to_string());
        }

        let mut existing_data = fs::read(&upload_path).await.map_err(|e| e.to_string())?;
        existing_data.extend_from_slice(data);

        fs::write(&upload_path, &existing_data)
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    async fn complete_upload(&self, uuid: &str, digest: &str, repo: &str) -> Result<(), String> {
        let upload_path = self.root.join("uploads").join(uuid);

        let data = fs::read(&upload_path)
            .await
            .map_err(|_| "Upload not found".to_string())?;

        let blob_dir = self.root.join(repo).join("blobs").join("sha256");
        fs::create_dir_all(&blob_dir)
            .await
            .map_err(|e| e.to_string())?;

        let filename = digest.strip_prefix("sha256:").unwrap_or(digest);
        let blob_path = blob_dir.join(filename);
        fs::write(&blob_path, &data)
            .await
            .map_err(|e| e.to_string())?;

        // Clean up upload file
        let _ = fs::remove_file(&upload_path).await;

        Ok(())
    }

    async fn get_blob(&self, digest: &str) -> Option<Vec<u8>> {
        // Try to find the blob in any repository
        let repos_dir = &self.root;

        let filename = digest.strip_prefix("sha256:").unwrap_or(digest);

        // Search in all repo directories
        if let Ok(mut entries) = fs::read_dir(repos_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry.path().is_dir() {
                    let blob_path = entry.path().join("blobs").join("sha256").join(filename);
                    if let Ok(data) = fs::read(&blob_path).await {
                        return Some(data);
                    }
                }
            }
        }

        None
    }

    async fn blob_exists(&self, digest: &str) -> bool {
        self.get_blob(digest).await.is_some()
    }

    async fn store_manifest(
        &self,
        repo: &str,
        reference: &str,
        data: Vec<u8>,
        content_type: String,
    ) -> Result<(), String> {
        let manifest_dir = self.root.join(repo).join("manifests");
        fs::create_dir_all(&manifest_dir)
            .await
            .map_err(|e| e.to_string())?;

        let manifest_path = manifest_dir.join(&reference);
        let content_type_path = manifest_dir.join(format!("{}.content_type", reference));

        fs::write(&manifest_path, &data)
            .await
            .map_err(|e| e.to_string())?;
        fs::write(&content_type_path, content_type.as_bytes())
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    async fn get_manifest(&self, repo: &str, reference: &str) -> Option<(Vec<u8>, String)> {
        let manifest_dir = self.root.join(repo).join("manifests");
        let manifest_path = manifest_dir.join(&reference);
        let content_type_path = manifest_dir.join(format!("{}.content_type", reference));

        let data = fs::read(&manifest_path).await.ok()?;
        let content_type = fs::read_to_string(&content_type_path)
            .await
            .unwrap_or_else(|_| "application/vnd.docker.distribution.manifest.v2+json".to_string());

        Some((data, content_type))
    }
}

// ------ API
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
            .and_then(|repo: String, storage: RegistryStorage| async move {
                println!("POST /v2/{}/blobs/uploads/", repo);
                match storage.init_upload().await {
                    Ok(uuid) => {
                        let location = format!("/v2/{}/blobs/uploads/{}", repo, uuid);
                        Ok::<_, warp::Rejection>(reply::with_status(
                            reply::with_header(
                                reply::with_header("", "Location", location),
                                "Docker-Upload-UUID",
                                uuid,
                            ),
                            StatusCode::ACCEPTED,
                        ))
                    }
                    Err(e) => {
                        eprintln!("Error initializing upload: {}", e);
                        Ok::<_, warp::Rejection>(reply::with_status(
                            reply::with_header(
                                reply::with_header("", "Location", ""),
                                "Docker-Upload-UUID",
                                "",
                            ),
                            StatusCode::INTERNAL_SERVER_ERROR,
                        ))
                    }
                }
            })
    }

    fn upload_chunk(
        storage: RegistryStorage,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("v2" / String / "blobs" / "uploads" / String)
            .and(warp::patch())
            .and(warp::body::bytes())
            .and(Self::with_storage(storage))
            .and_then(
                |repo: String, uuid: String, body: Bytes, storage: RegistryStorage| async move {
                    println!(
                        "PATCH /v2/{}/blobs/uploads/{} ({} bytes)",
                        repo,
                        uuid,
                        body.len()
                    );

                    match storage.append_to_upload(&uuid, &body).await {
                        Ok(_) => {
                            let location = format!("/v2/{}/blobs/uploads/{}", repo, uuid);
                            Ok::<_, warp::Rejection>(reply::with_status(
                                reply::with_header("", "Location", location),
                                StatusCode::ACCEPTED,
                            ))
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            Ok::<_, warp::Rejection>(reply::with_status(
                                reply::with_header("", "Location", ""),
                                StatusCode::NOT_FOUND,
                            ))
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
                        if let Err(e) = storage.append_to_upload(&uuid, &body).await {
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
            .and_then(
                |repo: String, digest: String, storage: RegistryStorage| async move {
                    println!("HEAD /v2/{}/blobs/{}", repo, digest);

                    if storage.blob_exists(&digest).await {
                        Ok::<_, warp::Rejection>(reply::with_status(
                            reply::with_header("", "Docker-Content-Digest", digest),
                            StatusCode::OK,
                        ))
                    } else {
                        Ok::<_, warp::Rejection>(reply::with_status(
                            reply::with_header("", "Docker-Content-Digest", ""),
                            StatusCode::NOT_FOUND,
                        ))
                    }
                },
            )
    }

    fn get_blob(
        storage: RegistryStorage,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("v2" / String / "blobs" / String)
            .and(warp::get())
            .and(Self::with_storage(storage))
            .and_then(
                |repo: String, digest: String, storage: RegistryStorage| async move {
                    println!("GET /v2/{}/blobs/{}", repo, digest);

                    if let Some(data) = storage.get_blob(&digest).await {
                        Ok::<_, warp::Rejection>(reply::with_status(
                            reply::with_header(data, "Docker-Content-Digest", digest),
                            StatusCode::OK,
                        ))
                    } else {
                        Ok::<_, warp::Rejection>(reply::with_status(
                            reply::with_header(Vec::new(), "Docker-Content-Digest", ""),
                            StatusCode::NOT_FOUND,
                        ))
                    }
                },
            )
    }

    fn put_manifest(
        storage: RegistryStorage,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("v2" / String / "manifests" / String)
            .and(warp::put())
            .and(warp::header::optional::<String>("content-type"))
            .and(warp::body::bytes())
            .and(Self::with_storage(storage))
            .and_then(
                |repo: String,
                 reference: String,
                 content_type: Option<String>,
                 body: Bytes,
                 storage: RegistryStorage| async move {
                    println!("PUT /v2/{}/manifests/{}", repo, reference);

                    // Use the provided content-type or default to Docker manifest v2
                    let content_type = content_type.unwrap_or_else(|| {
                        "application/vnd.docker.distribution.manifest.v2+json".to_string()
                    });
                    println!("Content-Type: {}", content_type);

                    // Calculate SHA256 digest of the manifest
                    let mut hasher = Sha256::new();
                    hasher.update(&body);
                    let digest = format!("sha256:{:x}", hasher.finalize());

                    println!("Manifest digest: {}", digest);

                    match storage
                        .store_manifest(&repo, &reference, body.to_vec(), content_type.clone())
                        .await
                    {
                        Ok(_) => Ok::<_, warp::Rejection>(reply::with_status(
                            reply::with_header(
                                reply::with_header(
                                    reply::with_header("", "Docker-Content-Digest", digest),
                                    "Location",
                                    format!("/v2/{}/manifests/{}", repo, reference),
                                ),
                                "Content-Type",
                                content_type,
                            ),
                            StatusCode::CREATED,
                        )),
                        Err(e) => {
                            eprintln!("Error storing manifest: {}", e);
                            Ok::<_, warp::Rejection>(reply::with_status(
                                reply::with_header(
                                    reply::with_header(
                                        reply::with_header("", "Docker-Content-Digest", ""),
                                        "Location",
                                        "",
                                    ),
                                    "Content-Type",
                                    "application/octet-stream",
                                ),
                                StatusCode::INTERNAL_SERVER_ERROR,
                            ))
                        }
                    }
                },
            )
    }

    fn get_manifest(
        storage: RegistryStorage,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("v2" / String / "manifests" / String)
            .and(warp::get())
            .and(Self::with_storage(storage))
            .and_then(
                |repo: String, reference: String, storage: RegistryStorage| async move {
                    println!("GET /v2/{}/manifests/{}", repo, reference);

                    if let Some((data, content_type)) =
                        storage.get_manifest(&repo, &reference).await
                    {
                        // Calculate digest for the response header
                        let mut hasher = Sha256::new();
                        hasher.update(&data);
                        let digest = format!("sha256:{:x}", hasher.finalize());

                        println!("Returning manifest with Content-Type: {}", content_type);

                        Ok::<_, warp::Rejection>(reply::with_status(
                            reply::with_header(
                                reply::with_header(data, "Docker-Content-Digest", digest),
                                "Content-Type",
                                content_type,
                            ),
                            StatusCode::OK,
                        ))
                    } else {
                        Ok::<_, warp::Rejection>(reply::with_status(
                            reply::with_header(
                                reply::with_header(Vec::new(), "Docker-Content-Digest", ""),
                                "Content-Type",
                                "application/octet-stream",
                            ),
                            StatusCode::NOT_FOUND,
                        ))
                    }
                },
            )
    }
}

// ----- MAIN
#[tokio::main]
pub async fn run() {
    let storage = RegistryStorage::new(PathBuf::from(REGISTRY_DATA_DIR));

    let routes = RegistryApi::version_check()
        .or(RegistryApi::start_upload(storage.clone()))
        .or(RegistryApi::upload_chunk(storage.clone()))
        .or(RegistryApi::complete_upload(storage.clone()))
        .or(RegistryApi::check_blob(storage.clone()))
        .or(RegistryApi::get_blob(storage.clone()))
        .or(RegistryApi::put_manifest(storage.clone()))
        .or(RegistryApi::get_manifest(storage));

    println!("Starting Docker Registry on http://0.0.0.0:{}", PORT);
    warp::serve(routes).run(([0, 0, 0, 0], PORT)).await;
}
