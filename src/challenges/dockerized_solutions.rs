use std::collections::HashMap;
use std::io::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::fs;

use uuid::Uuid;
use warp::Filter;
use warp::path;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct BlobInfo {
    pub digest: String,
    pub size: u64,
    pub media_type: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Manifest {
    schema_version: String,
    media_type: String,
    config: String,
    layers: Vec<BlobInfo>,
}

struct Repository {
    name: String,
    tags: HashMap<String, String>, // map tag => manifest digest
}

struct BlobStore {
    base_path: PathBuf,
    blobs: RwLock<HashMap<String, BlobInfo>>,
}

impl BlobStore {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            base_path,
            blobs: RwLock::new(HashMap::new()),
        }
    }

    pub async fn store_blob(
        &self,
        repo: &str,
        digest: &str,
        data: &[u8],
        media_type: &str,
    ) -> Result<BlobInfo> {
        let repo_path = self.base_path.join(repo).join("blobs/sha256");
        fs::create_dir_all(&repo_path).await?;

        let filename = digest.strip_prefix("sha256:").unwrap_or(digest);
        let blob_path = repo_path.join(filename);
        fs::write(&blob_path, data).await?;

        let info = BlobInfo {
            digest: digest.to_string(),
            size: data.len() as u64,
            media_type: media_type.to_string(),
            path: blob_path.clone(),
        };

        self.blobs
            .write()
            .unwrap()
            .insert(digest.to_string(), info.clone());

        Ok(info)
    }

    pub async fn get_blob(&self, repo: &str, digest: &str) -> Result<Vec<u8>> {
        let filename = digest.strip_prefix("sha256:").unwrap_or(digest);
        let blob_path = self
            .base_path
            .join(repo)
            .join("blobs/sha256")
            .join(filename);
        Ok(fs::read(&blob_path).await?)
    }
}

#[derive(Clone, Debug)]
struct FileStorage {
    root: PathBuf,
}

impl FileStorage {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn upload_path(&self, uuid: &str) -> PathBuf {
        self.root.join("uploads").join(uuid)
    }

    async fn init_upload(&self) -> Result<String> {
        let uuid = Uuid::new_v4().to_string();
        let path = self.upload_path(&uuid);

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::File::create(&path).await?;
        Ok(uuid)
    }

    async fn append_chunk(&self, uuid: &str, data: &[u8]) -> Result<()> {
        let path = self.upload_path(uuid);

        fs::write(path, data).await?;

        Ok(())
    }
}

fn build_upload_route(
    storage: FileStorage,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let upload_route = path!("v2" / String / "blobs" / "uploads");

    let storage = Arc::new(storage);
    let storage_filter = warp::any().map(move || Arc::clone(&storage));

    let upload_handler = |repo: String, storage: Arc<FileStorage>| async move {
        println!("--- Incoming POST Request ---");
        println!("Repository: {}", repo);

        let uuid = match storage.init_upload().await {
            Ok(uuid) => uuid,
            Err(e) => {
                return Err(warp::reject::reject());
            }
        };

        Ok::<_, warp::Rejection>(warp::reply::with_status(
            warp::reply::json(&HashMap::from([
                ("Location", format!("/v2/{}/blobs/uploads/{}", repo, uuid)),
                ("Docker-Upload-UUID", uuid),
                ("Range", "0-0".to_owned()),
            ])),
            warp::http::StatusCode::ACCEPTED,
        ))
    };

    return warp::post()
        .and(upload_route)
        .and(storage_filter)
        .and_then(upload_handler);
}

fn build_upload_chunk_route(
    storage: FileStorage,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let upload_chunk_route = path!("v2" / String / "blobs" / "uploads" / String);

    let chunk_query_param = warp::query::<HashMap<String, String>>();

    let storage = Arc::new(storage);
    let storage_filter = warp::any().map(move || Arc::clone(&storage));

    let upload_chunk_handler = |repo: String,
                                uuid: String,
                                query: HashMap<String, String>,
                                body: warp::hyper::body::Bytes,
                                storage: Arc<FileStorage>| async move {
        println!("--- Incoming PATCH Request ---");
        println!("Repository: {}", repo);
        println!("Upload UUID: {}", uuid);
        println!("Received chunk size: {} bytes", body.len());
        println!("Query: {:?}", query);

        if let Err(e) = storage.append_chunk(&uuid, &body).await {
            eprintln!("Error appending chunk: {:?}", e);
            return Err(warp::reject::reject());
        }

        println!("Chunk appended successfully");

        let digest = query.get("digest");

        if digest.is_some(){
            println!("Digest: {}", digest.unwrap());
        }

        

        Ok::<_, warp::Rejection>(warp::reply::with_status(
            warp::reply::json(&HashMap::from([
                ("Location", format!("/v2/{}/blobs/uploads/{}", repo, uuid)),
                ("Docker-Upload-UUID", uuid),
                ("Range", "0-0".to_owned()),
            ])),
            warp::http::StatusCode::ACCEPTED,
        ))
    };

    return warp::patch()
        .and(upload_chunk_route)
        .and(chunk_query_param)
        .and(warp::body::bytes())
        .and(storage_filter)
        .and_then(upload_chunk_handler);
}

#[tokio::main]
pub async fn run() {
    let storage = FileStorage::new("./data/docker-registry");

    let upload_route = build_upload_route(storage.clone());
    let upload_chunk_route = build_upload_chunk_route(storage);

    let app_routes = upload_route.or(upload_chunk_route);

    println!("Starting server on http://127.0.0.1:3030");
    warp::serve(app_routes).run(([127, 0, 0, 1], 3030)).await;
}
