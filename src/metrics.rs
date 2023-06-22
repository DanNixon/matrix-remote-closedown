use crate::command::Operation;
use kagiyama::prometheus::{
    self as prometheus_client,
    encoding::EncodeLabelSet,
    metrics::{counter::Counter, family::Family},
};
use lazy_static::lazy_static;

#[derive(Debug, Clone, Eq, Hash, PartialEq, EncodeLabelSet)]
pub(crate) struct CommandLables {
    operation: Operation,
}

impl CommandLables {
    pub(crate) fn new(operation: Operation) -> Self {
        Self { operation }
    }
}

lazy_static! {
    pub(crate) static ref COMMANDS: Family::<CommandLables, Counter> =
        Family::<CommandLables, Counter>::default();
}
