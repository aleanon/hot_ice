// FFI symbol names exported by the cdylib via `#[unsafe(no_mangle)]`.
//
// Each name has a random-looking suffix (e.g. `_lskdjfa3lkfjasdf`) to avoid
// collisions with user-defined symbols. The cdylib and host binary must agree
// on these exact names — they are the ABI contract between the two.
pub const SERIALIZE_STATE_FUNCTION_NAME: &str = "serialize_state_slkdfjaf3lasjfdsa";
pub const DESERIALIZE_STATE_FUNCTION_NAME: &str = "deserialize_state_sldafjal3lkfjasldf";
pub const FREE_SERIALIZED_DATA_FUNCTION_NAME: &str = "free_serialized_data_lsadkjfa3alfjda";
pub const LOAD_FONT_FUNCTION_NAME: &str = "load_font_into_system_lskdjfa3lkfjasdf";
pub const START_WORKER_FUNCTION_NAME: &str = "start_worker_lskdjfa3lkfjasdf";
pub const STOP_WORKER_FUNCTION_NAME: &str = "stop_worker_lskdjfa3lkfjasdf";
