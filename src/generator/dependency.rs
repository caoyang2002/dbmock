use crate::core::schema::Schema;
use crate::errors::{MockerError, Result};
use std::collections::{HashMap, HashSet, VecDeque};

/// Sort tables in dependency order using topological sort (Kahn's algorithm).
/// Tables with no foreign keys come first; referenced tables come before referencing tables.
pub fn topological_sort(schema: &Schema, requested: &[String]) -> Result<Vec<String>> {
    // Build adjacency: table -> tables it depends on (via FK)
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new(); // dep -> tables that depend on dep

    // Only consider tables that are requested or are dependencies of requested tables
    let all_tables: HashSet<String> = schema.tables.iter().map(|t| t.name.clone()).collect();

    for table in &schema.tables {
        in_degree.entry(table.name.clone()).or_insert(0);
        for fk in &table.foreign_keys {
            if fk.referenced_table == table.name {
                continue; // self-reference, skip
            }
            *in_degree.entry(table.name.clone()).or_insert(0) += 1;
            dependents
                .entry(fk.referenced_table.clone())
                .or_default()
                .push(table.name.clone());
        }
    }

    // Kahn's algorithm
    let mut queue: VecDeque<String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(name, _)| name.clone())
        .collect();

    let mut sorted = Vec::new();

    while let Some(table) = queue.pop_front() {
        sorted.push(table.clone());
        if let Some(deps) = dependents.get(&table) {
            for dep in deps {
                let deg = in_degree.get_mut(dep).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push_back(dep.clone());
                }
            }
        }
    }

    if sorted.len() != in_degree.len() {
        let unresolved: Vec<String> = in_degree
            .keys()
            .filter(|k| !sorted.contains(k))
            .cloned()
            .collect();
        return Err(MockerError::CircularDependency { tables: unresolved });
    }

    // Filter to only requested tables, but keep sorted order
    let requested_set: HashSet<&String> = requested.iter().collect();
    let result: Vec<String> = sorted
        .into_iter()
        .filter(|t| requested_set.contains(t))
        .collect();

    Ok(result)
}
