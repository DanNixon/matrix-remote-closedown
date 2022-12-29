use crate::command::Operation;
use lazy_static::lazy_static;
use prometheus_client::{
    encoding::EncodeLabelSet,
    metrics::{counter::Counter, family::Family},
};

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
