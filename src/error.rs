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

#[derive(thiserror::Error, Debug)]
pub enum HotFunctionError {
    #[error("Could not find function library")]
    LibraryNotFound,
    #[error("Could not load function")]
    FunctionNotFound(&'static str),
    #[error("Hot function call paniced")]
    FunctionPaniced(&'static str),
    #[error("Unable to acquire lock on reloader")]
    LockAcquisitionError,
}
