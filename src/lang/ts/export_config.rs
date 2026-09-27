use super::{BigIntExportBehavior};

/// Options for controlling the behavior of the Typescript exporter.
pub struct ExportConfiguration {
    /// How BigInts should be exported.
    pub(crate) bigint: BigIntExportBehavior,
    /// Whether to export types by default.
    /// This can be overridden on a type basis by using `#[specta(export)]`.
    #[cfg(feature = "export")]
    pub(crate) export_by_default: Option<bool>,
}

impl ExportConfiguration {
    /// Construct a new `ExportConfiguration`
    pub fn new() -> Self {
        Default::default()
    }

    /// Configure the BigInt handling behaviour
    pub fn bigint(mut self, bigint: BigIntExportBehavior) -> Self {
        self.bigint = bigint;
        self
    }

    /// Configure whether or not to export types by default.
    ///
    /// This can be overridden on a specific type by using `#[specta(export)]`.
    ///
    /// This parameter only takes effect when this configuration if passed into [`export::ts_with_cfg`](crate::export::ts_with_cfg)
    #[cfg(feature = "export")]
    pub fn export_by_default(mut self, x: Option<bool>) -> Self {
        self.export_by_default = x;
        self
    }
}

impl Default for ExportConfiguration {
    fn default() -> Self {
        Self {
            bigint: Default::default(),
            #[cfg(feature = "export")]
            export_by_default: None,
        }
    }
}
