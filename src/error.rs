//! Strongly-typed domain error definitions for DineInTakeOut POS.

/// Domain error variants covering persistence, thermal printing, CSV loading, and validation.
#[derive(thiserror::Error, Debug)]
pub enum PosError {
    #[error("Database error: {0}")]
    Database(#[from] turso::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("CSV parsing error: {0}")]
    Csv(#[from] csv::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Thermal printer error: {0}")]
    Printer(String),

    #[error("Application error: {0}")]
    Other(String),
}

impl PosError {
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    pub fn printer(msg: impl Into<String>) -> Self {
        Self::Printer(msg.into())
    }

    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

impl From<String> for PosError {
    fn from(msg: String) -> Self {
        Self::Other(msg)
    }
}

impl From<&str> for PosError {
    fn from(msg: &str) -> Self {
        Self::Other(msg.to_string())
    }
}

pub type PosResult<T> = Result<T, PosError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_and_constructors() {
        let val_err = PosError::validation("Invalid mobile number");
        assert_eq!(
            val_err.to_string(),
            "Validation error: Invalid mobile number"
        );

        let nf_err = PosError::not_found("Table Garden #4");
        assert_eq!(nf_err.to_string(), "Resource not found: Table Garden #4");

        let prn_err = PosError::printer("Paper out");
        assert_eq!(prn_err.to_string(), "Thermal printer error: Paper out");

        let oth_err: PosError = "Something failed".into();
        assert_eq!(oth_err.to_string(), "Application error: Something failed");

        let io_err = PosError::from(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file missing",
        ));
        assert!(io_err.to_string().contains("file missing"));
    }
}
