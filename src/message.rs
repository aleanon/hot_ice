use std::any::{Any, TypeId};

pub trait DynMessage: Send + 'static + std::fmt::Debug {
    fn clone_box(&self) -> Box<dyn DynMessage>;
    fn into_hot_message(self) -> HotMessage;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
    fn type_id(&self) -> TypeId;
}

impl<T> DynMessage for T
where
    T: Send + 'static + std::fmt::Debug + Clone,
{
    fn clone_box(&self) -> Box<dyn DynMessage> {
        Box::new(self.clone())
    }

    fn into_hot_message(self) -> HotMessage {
        HotMessage::from_message(self)
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
}

#[derive(Debug)]
pub struct HotMessage(pub Box<dyn DynMessage>);

impl HotMessage {
    pub fn from_message<M: DynMessage>(message: M) -> Self {
        Self(Box::new(message) as Box<dyn DynMessage>)
    }

    pub fn into_message<M: DynMessage + 'static>(self) -> M {
        let any_box = self.0.into_any();
        *any_box.downcast::<M>().unwrap()
    }
}

unsafe impl Send for HotMessage {}

impl Clone for HotMessage {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}
