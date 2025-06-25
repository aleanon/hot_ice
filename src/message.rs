use std::any::{Any, TypeId};

#[derive(Debug, Clone)]
pub enum MessageSource<M> {
    Static(M),
    Dynamic(M),
}

pub trait DynMessage: Send + 'static + std::fmt::Debug {
    fn clone_boxed(&self) -> Box<dyn DynMessage>;
    fn into_hot_message(self) -> HotMessage;
    fn as_any(&self) -> &dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
    fn type_id(&self) -> TypeId;
}

impl<T> DynMessage for T
where
    T: Send + 'static + std::fmt::Debug + Clone,
{
    fn clone_boxed(&self) -> Box<dyn DynMessage> {
        Box::new(self.clone())
    }

    fn into_hot_message(self) -> HotMessage {
        HotMessage::from_message(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }
}

// impl<T> From<T> for HotMessage
// where
//     T: DynMessage,
// {
//     fn from(message: T) -> Self {
//         HotMessage::from_message(message)
//     }
// }

// impl<T> TryInto<T> for HotMessage
// where
//     T: DynMessage,
// {
//     type Error = Self;
//     fn try_into(self) -> Result<T, Self::Error> {
//         self.into_message()
//     }
// }

#[derive(Debug)]
pub struct HotMessage(pub Box<dyn DynMessage>);

impl HotMessage {
    pub fn from_message<M: DynMessage>(message: M) -> Self {
        if TypeId::of::<M>() == TypeId::of::<Self>() {
            let any_box = message.clone_boxed().into_any();
            return *any_box.downcast::<Self>().unwrap();
        }
        Self(Box::new(message) as Box<dyn DynMessage>)
    }

    pub fn into_message<M: DynMessage>(self) -> Result<M, Self> {
        if let Some(_) = self.0.as_any().downcast_ref::<M>() {
            Ok(*self.0.into_any().downcast::<M>().unwrap())
        } else {
            Err(self)
        }
    }

    pub fn clone(&self) -> Self {
        Self(self.0.clone_boxed())
    }

    pub fn type_id(&self) -> TypeId {
        self.0.type_id()
    }
}

impl Clone for HotMessage {
    fn clone(&self) -> Self {
        Self(self.0.clone_boxed())
    }
}
