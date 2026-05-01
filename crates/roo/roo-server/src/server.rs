//! Main server struct.
//!
//! Source: `src/core/webview/ClineProvider.ts` — ClineProvider class
//!
//! The `Server` struct is the top-level entry point for the JSON-RPC server.
//! It ties together the transport, router, and handler layers.

use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{error, info, instrument};

use roo_app::App;

use crate::error::ServerResult;
use crate::handler::Handler;
use crate::router::Router;
use crate::transport::Transport;

// ---------------------------------------------------------------------------
// Server error codes (application-specific, -32000 to -32099)
// ---------------------------------------------------------------------------

/// Server error code range start (per JSON-RPC spec, -32000 to -32099 are reserved for implementation-defined errors)
pub const SERVER_ERROR_START: i64 = -32000;

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// The Roo Code JSON-RPC server.
///
/// This is the main entry point for running the Roo Code server. It manages
/// the lifecycle of the server, including initialization, request processing,
/// and shutdown.
///
/// # Example
///
/// ```rust,ignore
/// use roo_server::Server;
/// use roo_app::{App, AppConfig};
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let app = App::new(AppConfig::default());
///     let server = Server::new(app);
///     server.serve_stdio().await?;
///     Ok(())
/// }
/// ```
pub struct Server {
    app: Arc<RwLock<App>>,
    initialized: Arc<RwLock<bool>>,
    shut_down: Arc<RwLock<bool>>,
}

impl Server {
    /// Create a new server wrapping the given App.
    pub fn new(app: App) -> Self {
        Self {
            app: Arc::new(RwLock::new(app)),
            initialized: Arc::new(RwLock::new(false)),
            shut_down: Arc::new(RwLock::new(false)),
        }
    }

    /// Get a reference to the underlying App.
    pub async fn app(&self) -> tokio::sync::RwLockReadGuard<'_, App> {
        self.app.read().await
    }

    /// Check if the server has been initialized.
    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }

    /// Check if the server has been shut down.
    pub async fn is_shut_down(&self) -> bool {
        *self.shut_down.read().await
    }

    /// Run the server using stdin/stdout transport.
    ///
    /// This is the primary way to run the server for IPC communication.
    /// It reads Content-Length framed JSON-RPC messages from stdin and
    /// writes responses to stdout.
    #[instrument(skip(self))]
    pub async fn serve_stdio(&self) -> ServerResult<()> {
        info!("Starting Roo Code server (stdio transport)");

        // Register all known provider factories (idempotent — safe to call
        // multiple times, though the first call is the one that matters).
        crate::register_providers();

        let handler = Handler::from_arc(self.app.clone());
        let router = Router::new(handler);
        let mut transport = crate::transport::StdioTransport::new();

        self.run_message_loop(&router, &mut transport).await
    }

    /// Run the server using a custom transport.
    ///
    /// This allows using different transport implementations (e.g., TCP,
    /// WebSocket) while reusing the same handler and router logic.
    #[instrument(skip(self, transport))]
    pub async fn serve_with_transport<T: Transport>(&self, transport: &mut T) -> ServerResult<()> {
        info!("Starting Roo Code server (custom transport)");
        let handler = Handler::from_arc(self.app.clone());
        let router = Router::new(handler);
        self.run_message_loop(&router, transport).await
    }

    /// Core message loop — read messages, route them, write responses.
    async fn run_message_loop<T: Transport>(
        &self,
        router: &Router,
        transport: &mut T,
    ) -> ServerResult<()> {
        loop {
            // Check if we've been shut down.
            if *self.shut_down.read().await {
                info!("Server shut down, exiting message loop");
                return Ok(());
            }

            // Read a message from the transport.
            let message = match transport.receive().await {
                Ok(Some(msg)) => msg,
                Ok(None) => {
                    // EOF — client disconnected.
                    info!("Client disconnected");
                    return Ok(());
                }
                Err(e) => {
                    error!(error = %e, "Failed to read message");
                    // Try to continue after transient errors.
                    continue;
                }
            };

            // Route the message to the appropriate handler.
            let response = router.route(&message).await;

            // Write the response.
            if let Err(e) = transport.send(&response).await {
                error!(error = %e, "Failed to write response");
            }
        }
    }
}
