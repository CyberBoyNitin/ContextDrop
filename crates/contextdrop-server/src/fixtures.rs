use std::sync::Arc;

use contextdrop_core::{FixtureManifest, FixtureSpec, InputFormat};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Fixture {
    pub spec: FixtureSpec,
    pub content: &'static str,
}

impl Fixture {
    pub fn summary(&self) -> FixtureSummary {
        FixtureSummary {
            id: self.spec.id.clone(),
            name: self.spec.name.clone(),
            description: self.spec.description.clone(),
            category: self.spec.category.clone(),
            format: self.spec.format,
            original_bytes: self.content.len(),
            assertion_count: self.spec.must_preserve.len(),
            notes: self.spec.notes.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FixtureDetail {
    pub fixture: FixtureSummary,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixtureSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub format: InputFormat,
    pub original_bytes: usize,
    pub assertion_count: usize,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FixtureSet {
    fixtures: Arc<Vec<Fixture>>,
}

impl FixtureSet {
    pub fn load() -> Result<Self, String> {
        let manifest: FixtureManifest =
            serde_json::from_str(include_str!("../../../fixtures/manifest.json"))
                .map_err(|error| format!("embedded fixture manifest is invalid: {error}"))?;
        let mut fixtures = Vec::with_capacity(manifest.fixtures.len());
        for spec in manifest.fixtures {
            let content = fixture_content(&spec.id).ok_or_else(|| {
                format!(
                    "embedded fixture content is missing for manifest id {:?}",
                    spec.id
                )
            })?;
            fixtures.push(Fixture { spec, content });
        }
        Ok(Self {
            fixtures: Arc::new(fixtures),
        })
    }

    pub fn all(&self) -> &[Fixture] {
        &self.fixtures
    }

    pub fn get(&self, id: &str) -> Option<&Fixture> {
        self.fixtures.iter().find(|fixture| fixture.spec.id == id)
    }
}

fn fixture_content(id: &str) -> Option<&'static str> {
    Some(match id {
        "rust-build-failure" => {
            include_str!("../../../fixtures/rust-build-failure.log")
        }
        "typescript-build-failure" => {
            include_str!("../../../fixtures/typescript-build-failure.log")
        }
        "jest-test-failure" => include_str!("../../../fixtures/jest-test-failure.log"),
        "pytest-traceback" => include_str!("../../../fixtures/pytest-traceback.log"),
        "npm-install-progress" => {
            include_str!("../../../fixtures/npm-install-progress.log")
        }
        "structured-http-log" => include_str!("../../../fixtures/structured-http.log"),
        "json-duplicate-records" => {
            include_str!("../../../fixtures/large-json-array.json")
        }
        "security-scan" => include_str!("../../../fixtures/security-scan.log"),
        "rust-panic" => include_str!("../../../fixtures/rust-panic.log"),
        "dense-source-control" => include_str!("../../../fixtures/dense-source.txt"),
        "go-test-failure" => include_str!("../../../fixtures/go-test-failure.log"),
        "multilingual-errors" => include_str!("../../../fixtures/multilingual-errors.log"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_manifest_fixtures_are_embedded() {
        let set = FixtureSet::load().expect("fixtures must load");
        assert!(set.all().len() >= 10);
        for fixture in set.all() {
            assert!(!fixture.content.is_empty());
            for pattern in &fixture.spec.must_preserve {
                assert!(
                    fixture.content.contains(pattern),
                    "fixture {} is missing expected evidence {:?}",
                    fixture.spec.id,
                    pattern
                );
            }
        }
    }
}
