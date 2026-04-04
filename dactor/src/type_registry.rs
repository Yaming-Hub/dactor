//! Type registry for remote message deserialization and actor factory lookup.
//!
//! When a node receives a [`WireEnvelope`](crate::remote::WireEnvelope) from the
//! network, it needs to deserialize the message body bytes back into a concrete
//! Rust type. The [`TypeRegistry`] maps message type names (strings) to
//! deserializer functions that know how to reconstruct the original type.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dactor::type_registry::TypeRegistry;
//!
//! let mut registry = TypeRegistry::new();
//!
//! // Register a deserializer for a message type
//! registry.register("myapp::Increment", |bytes| {
//!     let msg: Increment = serde_json::from_slice(bytes)?;
//!     Ok(Box::new(msg))
//! });
//!
//! // Later, when receiving a WireEnvelope:
//! let any_msg = registry.deserialize("myapp::Increment", &body_bytes)?;
//! let msg = any_msg.downcast::<Increment>().unwrap();
//! ```

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use crate::remote::SerializationError;

/// A function that deserializes bytes into a type-erased value.
pub type DeserializerFn =
    Arc<dyn Fn(&[u8]) -> Result<Box<dyn Any + Send>, SerializationError> + Send + Sync>;

/// A function that creates an actor from serialized Args bytes.
/// Used for remote actor spawning.
pub type FactoryFn =
    Arc<dyn Fn(&[u8]) -> Result<Box<dyn Any + Send>, SerializationError> + Send + Sync>;

/// Registry mapping message type names to their deserializer functions.
///
/// Populated at startup with all message types that the node can handle
/// remotely. When a [`WireEnvelope`](crate::remote::WireEnvelope) arrives, the
/// registry is consulted to find the correct deserializer for the
/// `message_type` field.
///
/// Also stores [`ErasedActorFactory`] instances for remote actor spawning.
pub struct TypeRegistry {
    /// Message type name → deserializer function.
    deserializers: HashMap<String, DeserializerFn>,
    /// Actor type name → closure-based factory (legacy API).
    factories: HashMap<String, FactoryFn>,
    /// Actor type name → trait-based factory (for deps-aware remote spawn).
    actor_factories: HashMap<String, Arc<dyn ErasedActorFactory>>,
}

impl TypeRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            deserializers: HashMap::new(),
            factories: HashMap::new(),
            actor_factories: HashMap::new(),
        }
    }

    /// Register a deserializer function for a message type.
    ///
    /// The `type_name` should match the `message_type` field in
    /// [`WireEnvelope`](crate::remote::WireEnvelope), typically
    /// `std::any::type_name::<M>()`.
    pub fn register(
        &mut self,
        type_name: impl Into<String>,
        deserializer: impl Fn(&[u8]) -> Result<Box<dyn Any + Send>, SerializationError>
            + Send
            + Sync
            + 'static,
    ) {
        self.deserializers
            .insert(type_name.into(), Arc::new(deserializer));
    }

    /// Register a message type using serde_json deserialization.
    ///
    /// Convenience method that auto-generates a deserializer for types that
    /// implement `serde::Deserialize`. Uses `std::any::type_name::<M>()` as
    /// the registry key.
    #[cfg(feature = "serde")]
    pub fn register_type<M>(&mut self)
    where
        M: serde::de::DeserializeOwned + Send + 'static,
    {
        let type_name = std::any::type_name::<M>().to_string();
        self.deserializers.insert(
            type_name,
            Arc::new(|bytes: &[u8]| {
                let value: M = serde_json::from_slice(bytes).map_err(|e| {
                    SerializationError::new(format!(
                        "failed to deserialize {}: {e}",
                        std::any::type_name::<M>()
                    ))
                })?;
                Ok(Box::new(value) as Box<dyn Any + Send>)
            }),
        );
    }

    /// Deserialize a message body using the registered deserializer.
    ///
    /// Returns `Err` if no deserializer is registered for `type_name`.
    pub fn deserialize(
        &self,
        type_name: &str,
        bytes: &[u8],
    ) -> Result<Box<dyn Any + Send>, SerializationError> {
        let deserializer = self.deserializers.get(type_name).ok_or_else(|| {
            SerializationError::new(format!("no deserializer registered for '{type_name}'"))
        })?;
        deserializer(bytes)
    }

    /// Check if a deserializer is registered for a type name.
    pub fn has_type(&self, type_name: &str) -> bool {
        self.deserializers.contains_key(type_name)
    }

    /// Number of registered message types.
    pub fn type_count(&self) -> usize {
        self.deserializers.len()
    }

    /// Register an actor factory function for remote spawning.
    ///
    /// The `type_name` should match `std::any::type_name::<A>()` for
    /// the actor type. The factory deserializes the actor's `Args` from
    /// bytes and returns the constructed actor as `Box<dyn Any + Send>`.
    pub fn register_factory(
        &mut self,
        type_name: impl Into<String>,
        factory: impl Fn(&[u8]) -> Result<Box<dyn Any + Send>, SerializationError>
            + Send
            + Sync
            + 'static,
    ) {
        self.factories.insert(type_name.into(), Arc::new(factory));
    }

    /// Register an actor factory using serde_json for `Args` deserialization.
    ///
    /// Convenience method for actors whose `Args` type implements
    /// `serde::Deserialize`. The factory deserializes `Args`, then calls
    /// `Actor::create(args, deps)` with the provided `deps`.
    #[cfg(feature = "serde")]
    pub fn register_actor<A>(&mut self, deps: A::Deps)
    where
        A: crate::actor::Actor + Send + 'static,
        A::Args: serde::de::DeserializeOwned,
        A::Deps: Clone + Send + Sync + 'static,
    {
        let type_name = std::any::type_name::<A>().to_string();
        self.factories.insert(
            type_name,
            Arc::new(move |bytes: &[u8]| {
                let args: A::Args = serde_json::from_slice(bytes).map_err(|e| {
                    SerializationError::new(format!(
                        "failed to deserialize args for {}: {e}",
                        std::any::type_name::<A>()
                    ))
                })?;
                let actor = A::create(args, deps.clone());
                Ok(Box::new(actor) as Box<dyn Any + Send>)
            }),
        );
    }

    /// Look up an actor factory by type name and create an actor from bytes.
    pub fn create_actor(
        &self,
        type_name: &str,
        args_bytes: &[u8],
    ) -> Result<Box<dyn Any + Send>, SerializationError> {
        let factory = self.factories.get(type_name).ok_or_else(|| {
            SerializationError::new(format!("no actor factory registered for '{type_name}'"))
        })?;
        factory(args_bytes)
    }

    /// Check if a factory is registered for an actor type name.
    pub fn has_factory(&self, type_name: &str) -> bool {
        self.factories.contains_key(type_name) || self.actor_factories.contains_key(type_name)
    }

    /// Number of registered actor factories.
    pub fn factory_count(&self) -> usize {
        self.factories.len() + self.actor_factories.len()
    }

    /// Register a trait-based actor factory for remote spawning.
    ///
    /// Unlike [`register_factory`](Self::register_factory) which captures
    /// deps at registration time, this accepts an [`ErasedActorFactory`]
    /// that receives deps at creation time — allowing the remote node to
    /// resolve deps locally per spawn request.
    pub fn register_actor_factory(
        &mut self,
        type_name: impl Into<String>,
        factory: Arc<dyn ErasedActorFactory>,
    ) {
        self.actor_factories.insert(type_name.into(), factory);
    }

    /// Create an actor using a registered [`ErasedActorFactory`], passing
    /// deps at creation time.
    ///
    /// This is the preferred API for remote spawn — deps are resolved
    /// locally on the target node, not serialized from the source.
    pub fn create_actor_with_deps(
        &self,
        type_name: &str,
        args_bytes: &[u8],
        deps: Box<dyn Any + Send>,
    ) -> Result<Box<dyn Any + Send>, SerializationError> {
        let factory = self.actor_factories.get(type_name).ok_or_else(|| {
            SerializationError::new(format!(
                "no actor factory (trait-based) registered for '{type_name}'"
            ))
        })?;
        factory.create_erased(args_bytes, deps)
    }
}

impl Default for TypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ActorFactory trait (P2)
// ---------------------------------------------------------------------------

/// Factory for reconstructing an actor from serialized `Args` bytes on a
/// remote node.
///
/// Each actor type that can be remotely spawned registers an `ActorFactory`
/// in the [`TypeRegistry`]. When a [`SpawnRequest`](crate::system_actors::SpawnRequest)
/// arrives, the factory deserializes the `Args`, resolves local `Deps`,
/// and calls [`Actor::create(args, deps)`](crate::actor::Actor::create).
///
/// # Type erasure
///
/// `ActorFactory` is type-erased via [`ErasedActorFactory`] so that
/// different actor types can be stored in the same registry.
pub trait ActorFactory<A: crate::actor::Actor>: Send + Sync + 'static {
    /// Deserialize `Args` from bytes and create the actor.
    ///
    /// `deps` are resolved locally on the target node — they are never
    /// serialized across the network.
    fn create_from_bytes(&self, args_bytes: &[u8], deps: A::Deps) -> Result<A, SerializationError>;
}

/// Type-erased actor factory stored in [`TypeRegistry`].
///
/// Returns the constructed actor as `Box<dyn Any + Send>`.
pub trait ErasedActorFactory: Send + Sync + 'static {
    /// The Rust type name of the actor (for diagnostics).
    fn actor_type_name(&self) -> &'static str;

    /// Create an actor from serialized args and type-erased deps.
    ///
    /// The `deps` parameter is `Box<dyn Any + Send>` — the caller must
    /// provide the correct `Deps` type (downcast will fail otherwise).
    fn create_erased(
        &self,
        args_bytes: &[u8],
        deps: Box<dyn Any + Send>,
    ) -> Result<Box<dyn Any + Send>, SerializationError>;
}

/// Default [`ActorFactory`] implementation using serde_json for `Args`
/// deserialization. Created by [`TypeRegistry::register_actor`].
#[cfg(feature = "serde")]
pub struct JsonActorFactory<A: crate::actor::Actor> {
    _phantom: std::marker::PhantomData<fn() -> A>,
}

#[cfg(feature = "serde")]
impl<A> ActorFactory<A> for JsonActorFactory<A>
where
    A: crate::actor::Actor + Send + 'static,
    A::Args: serde::de::DeserializeOwned,
{
    fn create_from_bytes(&self, args_bytes: &[u8], deps: A::Deps) -> Result<A, SerializationError> {
        let args: A::Args = serde_json::from_slice(args_bytes).map_err(|e| {
            SerializationError::new(format!(
                "failed to deserialize args for {}: {e}",
                std::any::type_name::<A>()
            ))
        })?;
        Ok(A::create(args, deps))
    }
}

#[cfg(feature = "serde")]
impl<A> ErasedActorFactory for JsonActorFactory<A>
where
    A: crate::actor::Actor + Send + 'static,
    A::Args: serde::de::DeserializeOwned,
    A::Deps: Send + 'static,
{
    fn actor_type_name(&self) -> &'static str {
        std::any::type_name::<A>()
    }

    fn create_erased(
        &self,
        args_bytes: &[u8],
        deps: Box<dyn Any + Send>,
    ) -> Result<Box<dyn Any + Send>, SerializationError> {
        let deps = deps.downcast::<A::Deps>().map_err(|_| {
            SerializationError::new(format!(
                "deps type mismatch for {}: expected {}",
                std::any::type_name::<A>(),
                std::any::type_name::<A::Deps>()
            ))
        })?;
        let actor = self.create_from_bytes(args_bytes, *deps)?;
        Ok(Box::new(actor))
    }
}

#[cfg(feature = "serde")]
impl<A: crate::actor::Actor> JsonActorFactory<A> {
    /// Create a new `JsonActorFactory`.
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

#[cfg(feature = "serde")]
impl<A: crate::actor::Actor> Default for JsonActorFactory<A> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_deserialize_custom() {
        let mut registry = TypeRegistry::new();

        // Register a simple u64 deserializer (big-endian 8 bytes)
        registry.register("test::Counter", |bytes: &[u8]| {
            if bytes.len() != 8 {
                return Err(SerializationError::new("expected 8 bytes"));
            }
            let val = u64::from_be_bytes(bytes.try_into().unwrap());
            Ok(Box::new(val))
        });

        assert!(registry.has_type("test::Counter"));
        assert!(!registry.has_type("test::Unknown"));
        assert_eq!(registry.type_count(), 1);

        // Deserialize
        let bytes = 42u64.to_be_bytes();
        let any = registry.deserialize("test::Counter", &bytes).unwrap();
        let val = any.downcast::<u64>().unwrap();
        assert_eq!(*val, 42);
    }

    #[test]
    fn deserialize_unknown_type_returns_error() {
        let registry = TypeRegistry::new();
        let result = registry.deserialize("unknown::Type", &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("no deserializer"));
    }

    #[test]
    fn register_and_create_actor_custom() {
        let mut registry = TypeRegistry::new();

        // Register a factory that parses a u64 "args" and returns it
        registry.register_factory("test::Worker", |bytes: &[u8]| {
            if bytes.len() != 8 {
                return Err(SerializationError::new("expected 8 bytes"));
            }
            let val = u64::from_be_bytes(bytes.try_into().unwrap());
            Ok(Box::new(val))
        });

        assert!(registry.has_factory("test::Worker"));
        assert_eq!(registry.factory_count(), 1);

        let bytes = 99u64.to_be_bytes();
        let any = registry.create_actor("test::Worker", &bytes).unwrap();
        let val = any.downcast::<u64>().unwrap();
        assert_eq!(*val, 99);
    }

    #[test]
    fn create_actor_unknown_type_returns_error() {
        let registry = TypeRegistry::new();
        let result = registry.create_actor("unknown::Actor", &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("no actor factory"));
    }

    #[cfg(feature = "serde")]
    mod serde_tests {
        use super::*;

        #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        struct Increment {
            amount: u64,
        }

        #[test]
        fn register_type_and_roundtrip() {
            let mut registry = TypeRegistry::new();
            registry.register_type::<Increment>();

            let msg = Increment { amount: 42 };
            let bytes = serde_json::to_vec(&msg).unwrap();

            let type_name = std::any::type_name::<Increment>();
            assert!(registry.has_type(type_name));

            let any = registry.deserialize(type_name, &bytes).unwrap();
            let deserialized = any.downcast::<Increment>().unwrap();
            assert_eq!(*deserialized, msg);
        }

        #[test]
        fn register_type_invalid_bytes() {
            let mut registry = TypeRegistry::new();
            registry.register_type::<Increment>();

            let type_name = std::any::type_name::<Increment>();
            let result = registry.deserialize(type_name, b"not json");
            assert!(result.is_err());
        }

        // -- ActorFactory tests --

        struct TestActor {
            value: u64,
        }

        impl crate::actor::Actor for TestActor {
            type Args = u64;
            type Deps = ();
            fn create(args: u64, _deps: ()) -> Self {
                Self { value: args }
            }
        }

        #[test]
        fn json_actor_factory_create_from_bytes() {
            let factory = JsonActorFactory::<TestActor>::new();
            let args_bytes = serde_json::to_vec(&42u64).unwrap();
            let actor = factory.create_from_bytes(&args_bytes, ()).unwrap();
            assert_eq!(actor.value, 42);
        }

        #[test]
        fn json_actor_factory_invalid_bytes() {
            let factory = JsonActorFactory::<TestActor>::new();
            let result = factory.create_from_bytes(b"not json", ());
            assert!(result.is_err());
        }

        #[test]
        fn erased_actor_factory_create() {
            let factory = JsonActorFactory::<TestActor>::new();
            assert!(factory.actor_type_name().contains("TestActor"));

            let args_bytes = serde_json::to_vec(&99u64).unwrap();
            let deps: Box<dyn std::any::Any + Send> = Box::new(());
            let any_actor = factory.create_erased(&args_bytes, deps).unwrap();
            let actor = any_actor.downcast::<TestActor>().unwrap();
            assert_eq!(actor.value, 99);
        }

        #[test]
        fn erased_actor_factory_wrong_deps_type() {
            let factory = JsonActorFactory::<TestActor>::new();
            let args_bytes = serde_json::to_vec(&1u64).unwrap();
            let wrong_deps: Box<dyn std::any::Any + Send> = Box::new(42u64);
            let result = factory.create_erased(&args_bytes, wrong_deps);
            assert!(result.is_err());
            assert!(result.unwrap_err().message.contains("deps type mismatch"));
        }
    }
}
