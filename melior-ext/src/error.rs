/// An error returned by the wrappers in this crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("operation `{0}` is not registered in the context")]
    UnregisteredOperation(String),

    #[error("operation `{0}` does not belong to an extensible (dynamic) dialect")]
    NotDynamicOperation(String),

    #[error("operation `{0}` already has a trait of this kind attached")]
    TraitAlreadyAttached(String),

    #[error("dialect conversion failed")]
    Conversion,

    #[error("failed to translate the module to LLVM IR")]
    LlvmTranslation,

    #[error("the module has no `llvm.func @main()` that takes no arguments and returns nothing")]
    InvalidMain,

    #[error("failed to JIT-compile the module")]
    JitCompilation,

    #[error("failed to invoke the JIT-compiled function")]
    JitInvocation,
}

/// A failed verification of an operation, with the message to report.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct VerificationError {
    message: String,
}

impl VerificationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl From<String> for VerificationError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

impl From<&str> for VerificationError {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}
