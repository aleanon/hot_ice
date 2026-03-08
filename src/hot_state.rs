use serde::{Serialize, de::DeserializeOwned};
use std::any::Any;
use std::mem;

use crate::error::HotIceError;

/// Casts `&dyn Any` to `&T` without `TypeId` verification.
///
/// This is the stable equivalent of the nightly `downcast_unchecked_ref`.
/// A debug assertion checks size and alignment as a best-effort guard
/// against type mismatches during development.
///
/// # Safety
///
/// The concrete type behind the trait object must actually be `T`.
unsafe fn unchecked_downcast_ref<T: 'static>(any: &dyn Any) -> &T {
    debug_assert_eq!(
        mem::size_of_val(any),
        mem::size_of::<T>(),
        "hot_state downcast size mismatch: expected {}, got {}",
        mem::size_of::<T>(),
        mem::size_of_val(any),
    );
    debug_assert_eq!(
        mem::align_of_val(any),
        mem::align_of::<T>(),
        "hot_state downcast alignment mismatch: expected {}, got {}",
        mem::align_of::<T>(),
        mem::align_of_val(any),
    );
    unsafe { &*(any as *const dyn Any as *const T) }
}

/// Casts `&mut dyn Any` to `&mut T` without `TypeId` verification.
///
/// # Safety
///
/// The concrete type behind the trait object must actually be `T`.
unsafe fn unchecked_downcast_mut<T: 'static>(any: &mut dyn Any) -> &mut T {
    debug_assert_eq!(mem::size_of_val(&*any), mem::size_of::<T>());
    debug_assert_eq!(mem::align_of_val(&*any), mem::align_of::<T>());
    unsafe { &mut *(any as *mut dyn Any as *mut T) }
}

pub trait DynState: Send + Sync + 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn serialize_state(&self) -> Result<Vec<u8>, String>;
}

impl<T> DynState for T
where
    T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn serialize_state(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self).map_err(|e| e.to_string())
    }
}

pub struct HotState {
    state: Box<dyn DynState>,
}

impl HotState {
    pub fn new<T>(state: T) -> Self
    where
        T: DynState,
    {
        Self {
            state: Box::new(state),
        }
    }

    /// # Safety contract
    ///
    /// Uses an unchecked pointer cast instead of safe `downcast_ref`
    /// because `downcast_ref` relies on `TypeId`, which changes across
    /// cdylib reloads — the same struct compiled into two different cdylib
    /// loads gets different `TypeId` values, making `downcast_ref` always
    /// return `None` after a reload.
    ///
    /// This is sound as long as:
    /// 1. The caller passes the correct type `T` (enforced by the macro-
    ///    generated code which always uses the user's declared state type).
    /// 2. On state type changes, `deserialize_state` replaces the inner
    ///    `Box<dyn DynState>` with the new type before any downcast.
    pub fn ref_mut_state<T: 'static>(&mut self) -> &mut T {
        unsafe { unchecked_downcast_mut::<T>(self.state.as_any_mut()) }
    }

    pub fn ref_state<T: 'static>(&self) -> &T {
        unsafe { unchecked_downcast_ref::<T>(self.state.as_any()) }
    }

    pub fn serialize_state<T>(&self) -> Result<Vec<u8>, HotIceError>
    where
        T: DynState,
    {
        let serialized = self
            .state
            .serialize_state()
            .map_err(HotIceError::FailedToSerializeState)?;

        Ok(serialized)
    }

    pub fn deserialize_state<T>(&mut self, data: &[u8]) -> Result<(), HotIceError>
    where
        T: DynState + DeserializeOwned + Default,
    {
        let mut result = Ok(());
        let new_state: T = if data.is_empty() {
            result = Err(HotIceError::FailedToDeserializeState(
                "Empty data".to_string(),
            ));
            T::default()
        } else {
            match serde_json::from_slice(data) {
                Ok(state) => state,
                Err(e) => {
                    result = Err(HotIceError::FailedToDeserializeState(e.to_string()));
                    T::default()
                }
            }
        };

        let old_state = std::mem::replace(&mut self.state, Box::new(new_state));

        // Ownership transfer: the old state's memory is still referenced by
        // the reloader (via raw pointer from the cdylib FFI boundary). The
        // reloader is responsible for freeing it after the cdylib is unloaded.
        // Dropping here would cause a use-after-free in the cdylib.
        std::mem::forget(old_state);

        result
    }
}
