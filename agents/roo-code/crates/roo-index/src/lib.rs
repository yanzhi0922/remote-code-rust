//! # roo-index
//!
//! Code indexing subsystem for Roo Code Rust.
//!
//! Provides file indexing, embedding, and search capabilities for code
//! navigation and semantic code search.
//!
//! ## Architecture
//!
//! - [`cache_manager`] — File hash caching for incremental indexing
//! - [`config_manager`] — Configuration management and validation
//! - [`state_manager`] — Indexing state machine and progress tracking
//! - [`embedder`] — Embedding provider trait and implementations
//! - [`processor`] — File parsing and code block extraction
//! - [`search_service`] — Vector search over the code index
//! - [`service_factory`] — Factory for creating indexing services
//! - [`orchestrator`] — Top-level indexing workflow coordination
//! - [`manager`] — Main code index manager (BM25-based)
//! - [`types`] — Shared types and error definitions

pub mod cache_manager;
pub mod config_manager;
pub mod embedder;
pub mod manager;
pub mod orchestrator;
pub mod processor;
pub mod search_service;
pub mod service_factory;
pub mod state_manager;
pub mod types;

pub use cache_manager::CacheManager;
pub use config_manager::CodeIndexConfigManager;
pub use embedder::{Embedder, EmbeddingResponse, NoopEmbedder};
pub use manager::CodeIndexManager;
pub use orchestrator::CodeIndexOrchestrator;
pub use processor::{CodeBlock, CodeParser, FileProcessor, SimpleCodeParser};
pub use search_service::{CodeIndexSearchService, InMemoryVectorStore, VectorStore};
pub use service_factory::CodeIndexServiceFactory;
pub use state_manager::{CodeIndexStateManager, IndexingState, IndexingStatus};
pub use types::{CodeIndexConfig, IndexError, IndexStats, VectorStoreSearchResult};
