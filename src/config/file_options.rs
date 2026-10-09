use crate::config::FormatOptions;

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub(crate) enum BackupSuffix {
    #[default]
    Unspecified,
    None,
    Value(String),
}

impl BackupSuffix {
    pub(crate) fn set(&mut self, value: &str) {
        if value == "none" {
            *self = Self::None;
        } else if !value.is_empty() && !matches!(self, Self::None) {
            *self = Self::Value(value.to_string());
        }
    }

    pub(crate) fn inherit(&mut self, fallback: Self) {
        if matches!(self, Self::Unspecified) {
            *self = fallback;
        }
    }

    pub(crate) fn as_deref(&self) -> Option<&str> {
        match self {
            Self::Unspecified => Some(".orig"),
            Self::None => None,
            Self::Value(value) => Some(value),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub(crate) struct ConfigFileOptions {
    pub(crate) format: FormatOptions,
    pub(crate) backup_suffix: BackupSuffix,
}
