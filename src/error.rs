// Copied from the hot-lib-reloader crate

#[derive(thiserror::Error, Debug)]
pub enum HotReloaderError {
    #[error("Unable to copy library file: {0}")]
    LibraryCopyError(#[from] std::io::Error),
    #[error("Unable to load library: {0}")]
    LibraryLoadError(#[from] libloading::Error),
    #[error("The hot reloadable library has not been loaded. Has it not been built yet?")]
    LibraryNotLoaded,
}
