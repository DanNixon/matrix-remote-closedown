use crate::command::Operation;
use kagiyama::{AlwaysReady, Watcher};
use lazy_static::lazy_static;
use prometheus_client::encoding::text::Encode;
use prometheus_client::metrics::{counter::Counter, family::Family};

#[derive(Clone, Eq, Hash, PartialEq, Encode)]
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

pub(crate) fn register(watcher: &Watcher<AlwaysReady>) {
    let mut registry = watcher.metrics_registry();

    {
        let registry = registry.sub_registry_with_prefix("matrixremoteclosedown");
        registry.register("commands", "Command requests", Box::new(COMMANDS.clone()));
    }
}
