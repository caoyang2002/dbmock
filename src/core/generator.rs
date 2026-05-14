use crate::core::schema::Schema;
use crate::errors::Result;
use crate::fieldconfig::infer::MockConfig;
use async_trait::async_trait;
use std::collections::HashMap;

/// Trait for data generation strategies
#[async_trait]
pub trait DataGenerator: Send + Sync {
    /// Generate mock data for a set of tables
    async fn generate(
        &self,
        schema: &Schema,
        row_counts: &HashMap<String, usize>,
        preview: bool,
        mock_config: Option<&MockConfig>,
    ) -> Result<GenerationReport>;
}

/// Report of generation results
#[derive(Debug, Default)]
pub struct GenerationReport {
    pub tables_processed: usize,
    pub total_rows_inserted: u64,
    pub sql_statements: Vec<String>,
    pub errors: Vec<String>,
}
