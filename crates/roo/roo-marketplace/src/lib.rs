#![allow(dead_code)]
#![allow(clippy::all)]
#![deny(clippy::dbg_macro, clippy::todo)]
pub mod marketplace;
pub mod remote_config_loader;
pub mod simple_installer;
pub mod types;

pub use marketplace::MarketplaceManager;
pub use remote_config_loader::{RemoteConfigError, RemoteConfigLoader};
pub use simple_installer::{
    InstallError, InstallOptions, InstallResult, InstallTarget, SimpleInstaller,
};
pub use types::{
    InstallationMetadata, MarketplaceError, MarketplaceFilter, MarketplaceItem,
    MarketplaceItemType, MarketplaceItemsResponse,
};
