use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub(crate) struct EnvMarkerExpr {
    pub(crate) left: String,
    pub(crate) operator: String,
    pub(crate) right: String,
}
