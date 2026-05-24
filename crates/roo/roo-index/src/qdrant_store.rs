//! Qdrant vector store client implementation.
//!
//! Corresponds to `qdrant-client.ts` in the TypeScript source.
//!
//! Uses the Qdrant REST API via reqwest for vector storage operations
//! including collection management, upsert, search, and delete.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::search_service::VectorStore;
use crate::types::{IndexError, VectorStoreSearchResult};

/// Namespace UUID used for generating deterministic point IDs via UUID v5.
/// Must match `QDRANT_CODE_BLOCK_NAMESPACE` in the TypeScript constants.
const QDRANT_CODE_BLOCK_NAMESPACE: Uuid = Uuid::from_u128(0xf47ac10b_58cc_4372_a567_0e02b2c3d479);

/// HNSW parameter `m` — number of edges per node in the HNSW graph.
const HNSW_M: usize = 64;
/// HNSW parameter `ef_construct` — search depth during index construction.
const HNSW_EF_CONSTRUCT: usize = 512;
/// HNSW `ef` used at query time.
const SEARCH_HNSW_EF: usize = 128;
/// Number of path-segment payload indexes to create (0..=4).
const PATH_SEGMENT_INDEX_COUNT: usize = 5;

// ---------------------------------------------------------------------------
// JSON request / response types for the Qdrant REST API
// ---------------------------------------------------------------------------

#[derive(Serialize, Debug)]
struct CreateCollectionRequest {
    vectors: VectorParams,
    hnsw_config: HnswConfig,
}

#[derive(Serialize, Debug)]
struct VectorParams {
    size: usize,
    distance: String,
    on_disk: bool,
}

#[derive(Serialize, Debug)]
struct HnswConfig {
    m: usize,
    ef_construct: usize,
    on_disk: bool,
}

#[derive(Serialize, Debug)]
struct UpsertRequest {
    points: Vec<Point>,
    wait: bool,
}

#[derive(Serialize, Debug)]
struct Point {
    id: PointId,
    vector: Vec<f64>,
    payload: serde_json::Value,
}

#[derive(Serialize, Debug)]
#[serde(untagged)]
enum PointId {
    Uuid(String),
}

#[derive(Serialize, Debug)]
struct QueryRequest {
    query: Vec<f64>,
    filter: serde_json::Value,
    score_threshold: f64,
    limit: usize,
    params: SearchParams,
    with_payload: PayloadSelector,
}

#[derive(Serialize, Debug)]
struct SearchParams {
    hnsw_ef: usize,
    exact: bool,
}

#[derive(Serialize, Debug)]
struct PayloadSelector {
    include: Vec<String>,
}

#[derive(Serialize, Debug)]
struct DeleteByFilterRequest {
    filter: serde_json::Value,
    wait: bool,
}

#[derive(Serialize, Debug)]
struct CreateFieldIndexRequest {
    field_name: String,
    field_schema: String,
}

#[derive(Deserialize, Debug)]
struct CollectionInfoResponse {
    result: Option<CollectionInfoResult>,
}

#[derive(Deserialize, Debug)]
struct CollectionInfoResult {
    points_count: Option<u64>,
    config: Option<CollectionConfig>,
}

#[derive(Deserialize, Debug)]
struct CollectionConfig {
    params: Option<CollectionParams>,
}

#[derive(Deserialize, Debug)]
struct CollectionParams {
    vectors: Option<VectorsConfig>,
}

#[derive(Deserialize, Debug)]
struct VectorsConfig {
    size: Option<usize>,
}

#[derive(Deserialize, Debug)]
struct QueryResponse {
    result: Vec<QueryPoint>,
}

#[derive(Deserialize, Debug)]
struct QueryPoint {
    payload: Option<serde_json::Value>,
    score: Option<f64>,
}

#[derive(Deserialize, Debug)]
struct RetrieveResponse {
    result: Vec<RetrievedPoint>,
}

#[derive(Deserialize, Debug)]
struct RetrievedPoint {
    payload: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// QdrantVectorStore
// ---------------------------------------------------------------------------

/// Qdrant implementation of the [`VectorStore`] trait.
///
/// Communicates with a Qdrant server over its REST API using reqwest.
/// Each workspace gets its own collection derived from an SHA-256 hash of
/// the workspace path.
pub struct QdrantVectorStore {
    http: Client,
    base_url: String,
    collection_name: String,
    vector_size: usize,
    api_key: Option<String>,
}

impl QdrantVectorStore {
    /// Creates a new Qdrant vector store.
    ///
    /// # Arguments
    /// * `workspace_path` — Root path of the workspace (used to derive the collection name).
    /// * `url` — URL of the Qdrant server (e.g. `http://localhost:6333`).
    /// * `vector_size` — Dimensionality of the embedding vectors.
    /// * `api_key` — Optional API key for authenticated Qdrant instances.
    pub fn new(
        workspace_path: &str,
        url: &str,
        vector_size: usize,
        api_key: Option<String>,
    ) -> Self {
        let normalized_url = Self::parse_qdrant_url(url);
        let collection_name = Self::derive_collection_name(workspace_path);

        Self {
            http: Client::builder()
                .user_agent("Roo-Code")
                .build()
                .unwrap_or_default(),
            base_url: normalized_url,
            collection_name,
            vector_size,
            api_key,
        }
    }

    // -- URL parsing helpers ------------------------------------------------

    /// Parses and normalises a Qdrant server URL, handling bare hostnames,
    /// missing protocols, and explicit ports.
    fn parse_qdrant_url(url: &str) -> String {
        if url.trim().is_empty() {
            return "http://localhost:6333".to_string();
        }

        let trimmed = url.trim();

        // No protocol at all — treat as hostname.
        if !trimmed.starts_with("http://")
            && !trimmed.starts_with("https://")
            && !trimmed.contains("://")
        {
            return Self::parse_hostname(trimmed);
        }

        // Try to validate as a URL.
        match reqwest::Url::parse(trimmed) {
            Ok(_) => trimmed.to_string(),
            Err(_) => Self::parse_hostname(trimmed),
        }
    }

    fn parse_hostname(hostname: &str) -> String {
        if hostname.contains(':') {
            if hostname.starts_with("http") {
                hostname.to_string()
            } else {
                format!("http://{hostname}")
            }
        } else {
            format!("http://{hostname}")
        }
    }

    /// Derives a deterministic collection name from a workspace path:
    /// `ws-<first-16-chars-of-sha256>`.
    fn derive_collection_name(workspace_path: &str) -> String {
        let hash = Sha256::digest(workspace_path.as_bytes());
        let hex = hex::encode(hash);
        format!("ws-{}", &hex[..16])
    }

    // -- HTTP helpers -------------------------------------------------------

    fn request_builder(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), path);
        let mut req = self.http.request(method, &url);
        if let Some(ref key) = self.api_key {
            req = req.header("api-key", key.as_str());
        }
        req
    }

    /// Runs an async future synchronously using [`tokio::task::block_in_place`]
    /// so that we can implement the synchronous [`VectorStore`] trait.
    fn block_on_async<F, T>(future: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
            Err(_) => tokio::runtime::Runtime::new()
                .expect("failed to create tokio runtime")
                .block_on(future),
        }
    }

    // -- Collection management ----------------------------------------------

    async fn get_collection_info_async(
        &self,
    ) -> Result<Option<CollectionInfoResponse>, IndexError> {
        let path = format!("/collections/{}", self.collection_name);
        let resp = self
            .request_builder(reqwest::Method::GET, &path)
            .send()
            .await
            .map_err(|e| IndexError::GeneralError(format!("Qdrant GET collection failed: {e}")))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!(
                "Qdrant getCollection returned {status}: {body} (collection={})",
                self.collection_name
            );
            return Ok(None);
        }

        resp.json::<CollectionInfoResponse>()
            .await
            .map(Some)
            .map_err(|e| IndexError::GeneralError(format!("Failed to parse collection info: {e}")))
    }

    async fn create_collection_async(&self) -> Result<(), IndexError> {
        let path = format!("/collections/{}", self.collection_name);
        let body = CreateCollectionRequest {
            vectors: VectorParams {
                size: self.vector_size,
                distance: "Cosine".to_string(),
                on_disk: true,
            },
            hnsw_config: HnswConfig {
                m: HNSW_M,
                ef_construct: HNSW_EF_CONSTRUCT,
                on_disk: true,
            },
        };

        let resp = self
            .request_builder(reqwest::Method::PUT, &path)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                IndexError::GeneralError(format!("Qdrant createCollection failed: {e}"))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(IndexError::GeneralError(format!(
                "Qdrant createCollection returned {status}: {text}"
            )));
        }
        Ok(())
    }

    async fn delete_collection_async(&self) -> Result<(), IndexError> {
        let path = format!("/collections/{}", self.collection_name);
        let resp = self
            .request_builder(reqwest::Method::DELETE, &path)
            .send()
            .await
            .map_err(|e| {
                IndexError::GeneralError(format!("Qdrant deleteCollection failed: {e}"))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(IndexError::GeneralError(format!(
                "Qdrant deleteCollection returned {status}: {text}"
            )));
        }
        Ok(())
    }

    async fn collection_exists_async(&self) -> Result<bool, IndexError> {
        let info = self.get_collection_info_async().await?;
        Ok(info.is_some())
    }

    // -- Payload indexes ----------------------------------------------------

    async fn create_payload_indexes_async(&self) {
        // Index on 'type' field.
        if let Err(e) = self.create_field_index_async("type", "keyword").await {
            let msg = e.to_string().to_lowercase();
            if !msg.contains("already exists") {
                warn!(
                    "Could not create payload index for 'type' on {}: {e}",
                    self.collection_name
                );
            }
        }

        // Indexes on pathSegments.{0..4}.
        for i in 0..PATH_SEGMENT_INDEX_COUNT {
            let field = format!("pathSegments.{i}");
            if let Err(e) = self.create_field_index_async(&field, "keyword").await {
                let msg = e.to_string().to_lowercase();
                if !msg.contains("already exists") {
                    warn!(
                        "Could not create payload index for '{field}' on {}: {e}",
                        self.collection_name
                    );
                }
            }
        }
    }

    async fn create_field_index_async(
        &self,
        field_name: &str,
        field_schema: &str,
    ) -> Result<(), IndexError> {
        let path = format!("/collections/{}/index", self.collection_name);
        let body = CreateFieldIndexRequest {
            field_name: field_name.to_string(),
            field_schema: field_schema.to_string(),
        };

        let resp = self
            .request_builder(reqwest::Method::PUT, &path)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                IndexError::GeneralError(format!("Qdrant createFieldIndex failed: {e}"))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(IndexError::GeneralError(format!(
                "Qdrant createFieldIndex returned {status}: {text}"
            )));
        }
        Ok(())
    }

    // -- Search -------------------------------------------------------------

    async fn search_async(
        &self,
        query_vector: &[f64],
        directory_prefix: Option<&str>,
        min_score: f64,
        max_results: usize,
    ) -> Result<Vec<VectorStoreSearchResult>, IndexError> {
        let filter = Self::build_search_filter(directory_prefix);

        let request = QueryRequest {
            query: query_vector.to_vec(),
            filter,
            score_threshold: min_score,
            limit: max_results,
            params: SearchParams {
                hnsw_ef: SEARCH_HNSW_EF,
                exact: false,
            },
            with_payload: PayloadSelector {
                include: vec![
                    "filePath".to_string(),
                    "codeChunk".to_string(),
                    "startLine".to_string(),
                    "endLine".to_string(),
                    "pathSegments".to_string(),
                    "content".to_string(),
                    "file_path".to_string(),
                    "line_number".to_string(),
                ],
            },
        };

        let path = format!("/collections/{}/points/query", self.collection_name);
        let resp = self
            .request_builder(reqwest::Method::POST, &path)
            .json(&request)
            .send()
            .await
            .map_err(|e| IndexError::GeneralError(format!("Qdrant query failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(IndexError::GeneralError(format!(
                "Qdrant query returned {status}: {text}"
            )));
        }

        let query_resp: QueryResponse = resp.json().await.map_err(|e| {
            IndexError::GeneralError(format!("Failed to parse query response: {e}"))
        })?;

        let results = query_resp
            .result
            .into_iter()
            .filter_map(|point| {
                let payload = point.payload?;
                if !Self::is_payload_valid(&payload) {
                    return None;
                }
                Some(VectorStoreSearchResult {
                    file_path: payload
                        .get("filePath")
                        .or_else(|| payload.get("file_path"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    line_number: payload
                        .get("startLine")
                        .or_else(|| payload.get("line_number"))
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32),
                    content: payload
                        .get("codeChunk")
                        .or_else(|| payload.get("content"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    score: point.score.unwrap_or(0.0),
                    start_line: payload
                        .get("startLine")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32),
                    end_line: payload
                        .get("endLine")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32),
                    code_chunk: payload
                        .get("codeChunk")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                })
            })
            .collect();

        Ok(results)
    }

    /// Builds the Qdrant filter JSON for search requests.
    /// Always excludes metadata points. Optionally filters by directory prefix
    /// using `pathSegments` fields.
    fn build_search_filter(directory_prefix: Option<&str>) -> serde_json::Value {
        let mut must: Vec<serde_json::Value> = Vec::new();
        let must_not: Vec<serde_json::Value> = vec![json!({
            "key": "type",
            "match": { "value": "metadata" }
        })];

        if let Some(prefix) = directory_prefix {
            let normalized = prefix.replace('\\', "/");
            let normalized = normalized.trim_end_matches('/');
            // Normalise: treat "." or "./" as "no filter".
            if !normalized.is_empty() && normalized != "." && normalized != "./" {
                let cleaned = if let Some(stripped) = normalized.strip_prefix("./") {
                    stripped
                } else {
                    normalized
                };
                let segments: Vec<&str> = cleaned.split('/').filter(|s| !s.is_empty()).collect();
                for (i, segment) in segments.iter().enumerate() {
                    must.push(json!({
                        "key": format!("pathSegments.{i}"),
                        "match": { "value": *segment }
                    }));
                }
            }
        }

        if must.is_empty() {
            json!({ "must_not": must_not })
        } else {
            json!({ "must": must, "must_not": must_not })
        }
    }

    /// Checks whether a payload contains the minimum required fields.
    fn is_payload_valid(payload: &serde_json::Value) -> bool {
        let required = ["filePath", "codeChunk", "startLine", "endLine"];
        required.iter().all(|key| payload.get(key).is_some())
    }

    // -- Upsert -------------------------------------------------------------

    async fn upsert_async(
        &self,
        ids: &[String],
        vectors: &[Vec<f64>],
        payloads: &[serde_json::Value],
    ) -> Result<(), IndexError> {
        let points: Vec<Point> = ids
            .iter()
            .zip(vectors.iter())
            .zip(payloads.iter())
            .map(|((id, vector), payload)| {
                let enriched_payload = if let Some(file_path) = payload
                    .get("filePath")
                    .or_else(|| payload.get("file_path"))
                    .and_then(|v| v.as_str())
                {
                    let segments: Vec<&str> = file_path
                        .split(['/', '\\'])
                        .filter(|s| !s.is_empty())
                        .collect();
                    let path_segments: serde_json::Map<String, serde_json::Value> = segments
                        .iter()
                        .enumerate()
                        .map(|(i, seg)| (i.to_string(), json!(seg)))
                        .collect();
                    let mut merged = payload.clone();
                    if let Some(obj) = merged.as_object_mut() {
                        obj.insert(
                            "pathSegments".to_string(),
                            serde_json::Value::Object(path_segments),
                        );
                    }
                    merged
                } else {
                    payload.clone()
                };

                Point {
                    id: PointId::Uuid(id.clone()),
                    vector: vector.clone(),
                    payload: enriched_payload,
                }
            })
            .collect();

        let path = format!("/collections/{}/points", self.collection_name);
        let body = UpsertRequest { points, wait: true };

        let resp = self
            .request_builder(reqwest::Method::PUT, &path)
            .json(&body)
            .send()
            .await
            .map_err(|e| IndexError::GeneralError(format!("Qdrant upsert failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(IndexError::GeneralError(format!(
                "Qdrant upsert returned {status}: {text}"
            )));
        }
        Ok(())
    }

    // -- Delete by prefix ---------------------------------------------------

    async fn delete_by_prefix_async(&self, prefix: &str) -> Result<(), IndexError> {
        // Check collection exists first.
        if !self.collection_exists_async().await? {
            warn!(
                "Skipping deletion — collection \"{}\" does not exist",
                self.collection_name
            );
            return Ok(());
        }

        let normalized = prefix.replace('\\', "/");
        let segments: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();

        let must_conditions: Vec<serde_json::Value> = segments
            .iter()
            .enumerate()
            .map(|(i, seg)| {
                json!({
                    "key": format!("pathSegments.{i}"),
                    "match": { "value": *seg }
                })
            })
            .collect();

        let filter = json!({ "must": must_conditions });

        let path = format!("/collections/{}/points/delete", self.collection_name);
        let body = DeleteByFilterRequest { filter, wait: true };

        let resp = self
            .request_builder(reqwest::Method::POST, &path)
            .json(&body)
            .send()
            .await
            .map_err(|e| IndexError::GeneralError(format!("Qdrant delete failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(IndexError::GeneralError(format!(
                "Qdrant delete returned {status}: {text}"
            )));
        }
        Ok(())
    }

    // -- Metadata (indexing complete / incomplete) --------------------------

    /// Deterministic metadata point ID generated from `"__indexing_metadata__"`
    /// using UUID v5 with the Qdrant code-block namespace.
    fn metadata_point_id() -> Uuid {
        Uuid::new_v5(&QDRANT_CODE_BLOCK_NAMESPACE, b"__indexing_metadata__")
    }

    /// Marks the indexing process as complete by upserting a metadata point.
    pub fn mark_indexing_complete(&self) -> Result<(), IndexError> {
        Self::block_on_async(async { self.mark_indexing_complete_async().await })
    }

    async fn mark_indexing_complete_async(&self) -> Result<(), IndexError> {
        let id = Self::metadata_point_id();
        let point = Point {
            id: PointId::Uuid(id.to_string()),
            vector: vec![0.0; self.vector_size],
            payload: json!({
                "type": "metadata",
                "indexing_complete": true,
                "completed_at": chrono::Utc::now().timestamp_millis(),
            }),
        };

        let path = format!("/collections/{}/points", self.collection_name);
        let body = UpsertRequest {
            points: vec![point],
            wait: true,
        };

        let resp = self
            .request_builder(reqwest::Method::PUT, &path)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                IndexError::GeneralError(format!("Qdrant mark_indexing_complete failed: {e}"))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(IndexError::GeneralError(format!(
                "Qdrant mark_indexing_complete returned {status}: {text}"
            )));
        }
        debug!("Marked indexing as complete for {}", self.collection_name);
        Ok(())
    }

    /// Marks the indexing process as incomplete (in progress).
    pub fn mark_indexing_incomplete(&self) -> Result<(), IndexError> {
        Self::block_on_async(async { self.mark_indexing_incomplete_async().await })
    }

    async fn mark_indexing_incomplete_async(&self) -> Result<(), IndexError> {
        let id = Self::metadata_point_id();
        let point = Point {
            id: PointId::Uuid(id.to_string()),
            vector: vec![0.0; self.vector_size],
            payload: json!({
                "type": "metadata",
                "indexing_complete": false,
                "started_at": chrono::Utc::now().timestamp_millis(),
            }),
        };

        let path = format!("/collections/{}/points", self.collection_name);
        let body = UpsertRequest {
            points: vec![point],
            wait: true,
        };

        let resp = self
            .request_builder(reqwest::Method::PUT, &path)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                IndexError::GeneralError(format!("Qdrant mark_indexing_incomplete failed: {e}"))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(IndexError::GeneralError(format!(
                "Qdrant mark_indexing_incomplete returned {status}: {text}"
            )));
        }
        debug!(
            "Marked indexing as incomplete (in progress) for {}",
            self.collection_name
        );
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// VectorStore trait implementation
// ---------------------------------------------------------------------------

impl VectorStore for QdrantVectorStore {
    fn initialize(&self) -> Result<bool, IndexError> {
        Self::block_on_async(async { self.initialize_async().await })
    }

    fn search(
        &self,
        query_vector: &[f64],
        directory_prefix: Option<&str>,
        min_score: f64,
        max_results: usize,
    ) -> Result<Vec<VectorStoreSearchResult>, IndexError> {
        Self::block_on_async(async {
            self.search_async(query_vector, directory_prefix, min_score, max_results)
                .await
        })
    }

    fn has_indexed_data(&self) -> Result<bool, IndexError> {
        Self::block_on_async(async { self.has_indexed_data_async().await })
    }

    fn upsert(
        &self,
        ids: &[String],
        vectors: &[Vec<f64>],
        payloads: &[serde_json::Value],
    ) -> Result<(), IndexError> {
        Self::block_on_async(async { self.upsert_async(ids, vectors, payloads).await })
    }

    fn delete_by_prefix(&self, prefix: &str) -> Result<(), IndexError> {
        Self::block_on_async(async { self.delete_by_prefix_async(prefix).await })
    }
}

// ---------------------------------------------------------------------------
// Private async implementations
// ---------------------------------------------------------------------------

impl QdrantVectorStore {
    async fn initialize_async(&self) -> Result<bool, IndexError> {
        let collection_info = self.get_collection_info_async().await?;

        let created = match collection_info {
            None => {
                // Collection does not exist — create it.
                self.create_collection_async().await?;
                true
            }
            Some(info) => {
                // Collection exists — check vector size.
                let existing_size = info
                    .result
                    .as_ref()
                    .and_then(|r| r.config.as_ref())
                    .and_then(|c| c.params.as_ref())
                    .and_then(|p| p.vectors.as_ref())
                    .and_then(|v| v.size);

                match existing_size {
                    Some(size) if size == self.vector_size => false,
                    Some(size) => {
                        // Dimension mismatch — recreate.
                        warn!(
                            "Collection {} exists with vector size {size}, but expected {}. \
                             Recreating collection.",
                            self.collection_name, self.vector_size
                        );
                        self.delete_collection_async().await?;
                        self.create_collection_async().await?;
                        true
                    }
                    None => {
                        // Unknown configuration — assume it's fine.
                        debug!(
                            "Could not determine existing vector size for {}, assuming correct",
                            self.collection_name
                        );
                        false
                    }
                }
            }
        };

        // Always (re-)create payload indexes (idempotent).
        self.create_payload_indexes_async().await;

        Ok(created)
    }

    async fn has_indexed_data_async(&self) -> Result<bool, IndexError> {
        let info = match self.get_collection_info_async().await? {
            Some(info) => info,
            None => return Ok(false),
        };

        let points_count = info
            .result
            .as_ref()
            .and_then(|r| r.points_count)
            .unwrap_or(0);

        if points_count == 0 {
            return Ok(false);
        }

        // Check if the indexing completion marker exists.
        let metadata_id = Self::metadata_point_id();
        let path = format!(
            "/collections/{}/points/{}",
            self.collection_name, metadata_id
        );

        let resp = self
            .request_builder(reqwest::Method::GET, &path)
            .send()
            .await;

        match resp {
            Ok(resp) if resp.status().is_success() => {
                let retrieved: RetrieveResponse = resp.json().await.map_err(|e| {
                    IndexError::GeneralError(format!("Failed to parse retrieve response: {e}"))
                })?;

                if let Some(point) = retrieved.result.first()
                    && let Some(ref payload) = point.payload
                    && let Some(complete) =
                        payload.get("indexing_complete").and_then(|v| v.as_bool())
                {
                    return Ok(complete);
                }

                // Backward compatibility: no marker — fall back to checking points_count > 0.
                debug!(
                    "No indexing metadata marker found for {}. \
                     Using backward compatibility mode (points_count > 0).",
                    self.collection_name
                );
                Ok(points_count > 0)
            }
            _ => {
                // Could not retrieve marker — fall back.
                Ok(points_count > 0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_qdrant_url_empty() {
        assert_eq!(
            QdrantVectorStore::parse_qdrant_url(""),
            "http://localhost:6333"
        );
    }

    #[test]
    fn test_parse_qdrant_url_whitespace() {
        assert_eq!(
            QdrantVectorStore::parse_qdrant_url("   "),
            "http://localhost:6333"
        );
    }

    #[test]
    fn test_parse_qdrant_url_full() {
        assert_eq!(
            QdrantVectorStore::parse_qdrant_url("http://localhost:6333"),
            "http://localhost:6333"
        );
    }

    #[test]
    fn test_parse_qdrant_url_https() {
        assert_eq!(
            QdrantVectorStore::parse_qdrant_url("https://qdrant.example.com:443"),
            "https://qdrant.example.com:443"
        );
    }

    #[test]
    fn test_parse_qdrant_url_hostname_only() {
        assert_eq!(
            QdrantVectorStore::parse_qdrant_url("my-qdrant-host"),
            "http://my-qdrant-host"
        );
    }

    #[test]
    fn test_parse_qdrant_url_hostname_with_port() {
        assert_eq!(
            QdrantVectorStore::parse_qdrant_url("my-qdrant-host:6333"),
            "http://my-qdrant-host:6333"
        );
    }

    #[test]
    fn test_derive_collection_name() {
        let name = QdrantVectorStore::derive_collection_name("/my/workspace");
        assert!(name.starts_with("ws-"));
        assert_eq!(name.len(), 19); // "ws-" + 16 hex chars
    }

    #[test]
    fn test_derive_collection_name_deterministic() {
        let a = QdrantVectorStore::derive_collection_name("/same/path");
        let b = QdrantVectorStore::derive_collection_name("/same/path");
        assert_eq!(a, b);
    }

    #[test]
    fn test_derive_collection_name_different_paths() {
        let a = QdrantVectorStore::derive_collection_name("/path/a");
        let b = QdrantVectorStore::derive_collection_name("/path/b");
        assert_ne!(a, b);
    }

    #[test]
    fn test_metadata_point_id_deterministic() {
        let a = QdrantVectorStore::metadata_point_id();
        let b = QdrantVectorStore::metadata_point_id();
        assert_eq!(a, b);
    }

    #[test]
    fn test_build_search_filter_no_prefix() {
        let filter = QdrantVectorStore::build_search_filter(None);
        assert!(filter.get("must_not").is_some());
        let must_not = filter.get("must_not").unwrap().as_array().unwrap();
        assert_eq!(must_not.len(), 1);
        assert_eq!(must_not[0]["key"].as_str().unwrap(), "type");
    }

    #[test]
    fn test_build_search_filter_with_prefix() {
        let filter = QdrantVectorStore::build_search_filter(Some("src/components"));
        let must = filter.get("must").unwrap().as_array().unwrap();
        assert_eq!(must.len(), 2);
        assert_eq!(must[0]["key"].as_str().unwrap(), "pathSegments.0");
        assert_eq!(must[0]["match"]["value"].as_str().unwrap(), "src");
        assert_eq!(must[1]["key"].as_str().unwrap(), "pathSegments.1");
        assert_eq!(must[1]["match"]["value"].as_str().unwrap(), "components");
    }

    #[test]
    fn test_build_search_filter_dot_prefix() {
        // "." and "./" should not add must conditions (search entire workspace).
        let filter = QdrantVectorStore::build_search_filter(Some("."));
        assert!(filter.get("must").is_none());

        let filter = QdrantVectorStore::build_search_filter(Some("./"));
        assert!(filter.get("must").is_none());
    }

    #[test]
    fn test_is_payload_valid() {
        let valid = serde_json::json!({
            "filePath": "src/main.rs",
            "codeChunk": "fn main() {}",
            "startLine": 1,
            "endLine": 1
        });
        assert!(QdrantVectorStore::is_payload_valid(&valid));

        let invalid = serde_json::json!({
            "filePath": "src/main.rs"
        });
        assert!(!QdrantVectorStore::is_payload_valid(&invalid));
    }

    #[test]
    fn test_new_constructor() {
        let store = QdrantVectorStore::new("/test/workspace", "http://localhost:6333", 1536, None);
        assert!(store.collection_name.starts_with("ws-"));
        assert_eq!(store.vector_size, 1536);
    }

    #[test]
    fn test_new_constructor_with_api_key() {
        let store = QdrantVectorStore::new(
            "/test/workspace",
            "http://localhost:6333",
            768,
            Some("my-secret-key".to_string()),
        );
        assert_eq!(store.api_key, Some("my-secret-key".to_string()));
        assert_eq!(store.vector_size, 768);
    }
}
