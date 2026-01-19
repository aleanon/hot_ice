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
pub enum HotIceError {
    #[error("Could not find function library")]
    LibraryNotFound,
    #[error("Could not load function: {0}")]
    FunctionNotFound(&'static str),
    #[error("Hot function call paniced: {0}")]
    FunctionPaniced(&'static str),
    #[error("Unable to acquire lock on reloader")]
    LockAcquisitionError,
    #[error("Failed to downcast Message: {0}")]
    MessageDowncastError(String),
    #[error("State type mismatch")]
    StateTypeMismatch,
    #[error("Failed to serialize state: {0}")]
    FailedToSerializeState(String),
    #[error("Failed to deserialize state: {0}")]
    FailedToDeserializeState(String),
    #[error("Failed to acquire lock on state")]
    StateLockAcquisitionError,
}

impl<T> From<std::sync::PoisonError<T>> for HotIceError {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        HotIceError::StateLockAcquisitionError
    }
}

pub struct HotResult<T>(pub Result<T, HotIceError>);

impl<T> From<Result<T, HotIceError>> for HotResult<T> {
    fn from(result: Result<T, HotIceError>) -> Self {
        HotResult(result)
    }
}
