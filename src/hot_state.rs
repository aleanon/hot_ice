use serde::{Serialize, de::DeserializeOwned};
use std::any::Any;

use crate::HotFunctionError;

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
        rmp_serde::to_vec(self).map_err(|e| e.to_string())
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

    pub fn ref_mut_state<T: 'static>(&mut self) -> &mut T {
        unsafe { self.state.as_any_mut().downcast_unchecked_mut::<T>() }
    }

    pub fn ref_state<T: 'static>(&self) -> &T {
        unsafe { self.state.as_any().downcast_unchecked_ref::<T>() }
    }

    pub fn serialize_state<T>(&self) -> Result<Vec<u8>, HotFunctionError>
    where
        T: DynState + Serialize + 'static,
    {
        let serialized = self
            .state
            .serialize_state()
            .map_err(|_| HotFunctionError::FailedToSerializeState)?;

        Ok(serialized)
    }

    pub fn deserialize_state<T>(&mut self, data: &[u8]) -> Result<(), HotFunctionError>
    where
        T: DynState + DeserializeOwned + 'static + Default,
    {
        let new_state: T = if data.is_empty() {
            T::default()
        } else {
            match rmp_serde::from_slice(data) {
                Ok(state) => state,
                Err(_) => T::default(),
            }
        };

        let old_state = std::mem::replace(&mut self.state, Box::new(new_state));

        std::mem::forget(old_state);

        Ok(())
    }
}
