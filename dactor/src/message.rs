/// Defines a message type and its expected reply.
///
/// Implemented on the MESSAGE, not the actor — decouples message definition
/// from handling, allowing the same message to be handled by different actors.
pub trait Message: Send + 'static {
    /// The reply type. Use `()` for fire-and-forget (tell) messages.
    type Reply: Send + 'static;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Increment(u64);
    impl Message for Increment {
        type Reply = ();
    }

    struct GetCount;
    impl Message for GetCount {
        type Reply = u64;
    }

    struct Reset;
    impl Message for Reset {
        type Reply = u64;
    }

    #[test]
    fn test_message_reply_types() {
        fn assert_reply_unit<M: Message<Reply = ()>>() {}
        fn assert_reply_u64<M: Message<Reply = u64>>() {}

        assert_reply_unit::<Increment>();
        assert_reply_u64::<GetCount>();
        assert_reply_u64::<Reset>();
    }
}
