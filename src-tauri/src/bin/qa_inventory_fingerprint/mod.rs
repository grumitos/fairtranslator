use std::collections::{HashMap, HashSet};

use app_lib::models::{AiBatchEntry, TranslationFile};

pub const INVENTORY_FINGERPRINT_PREFIX: &str = "fairtl-inventory-v1-fnv1a64:";

const FNV1A64_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV1A64_PRIME: u64 = 0x00000100000001b3;
const INVENTORY_DOMAIN: &[u8] = b"fairtranslator-project-inventory-v1\0";

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV1A64_PRIME);
    }
}

fn hash_field(hash: &mut u64, value: &str) {
    hash_bytes(hash, &(value.len() as u64).to_be_bytes());
    hash_bytes(hash, value.as_bytes());
}

pub fn inventory_fingerprint(files: &[TranslationFile]) -> String {
    let mut rows = files
        .iter()
        .flat_map(|file| {
            file.entries.iter().map(move |entry| {
                (
                    file.relative_path.as_str(),
                    entry.id.as_str(),
                    entry.original.as_str(),
                )
            })
        })
        .collect::<Vec<_>>();
    rows.sort_unstable();

    let mut hash = FNV1A64_OFFSET_BASIS;
    hash_bytes(&mut hash, INVENTORY_DOMAIN);
    hash_bytes(&mut hash, &(rows.len() as u64).to_be_bytes());
    for (relative_path, entry_id, original) in rows {
        hash_field(&mut hash, relative_path);
        hash_field(&mut hash, entry_id);
        hash_field(&mut hash, original);
    }

    format!("{INVENTORY_FINGERPRINT_PREFIX}{hash:016x}")
}

pub fn verify_project_inventory_fingerprint<'a>(
    expected: Option<&'a str>,
    project_files: &[TranslationFile],
) -> Result<&'a str, String> {
    let Some(expected) = expected else {
        return Err(
            "El manifest no incluye inventoryFingerprint. Regenera la muestra con qa_prepare antes de ejecutar qa_apply; no se aplicó ninguna respuesta."
                .to_string(),
        );
    };
    let digest = expected
        .strip_prefix(INVENTORY_FINGERPRINT_PREFIX)
        .filter(|digest| {
            digest.len() == 16
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .ok_or_else(|| {
            "Versión o formato de inventoryFingerprint no compatible. Regenera la muestra con qa_prepare antes de ejecutar qa_apply; no se aplicó ninguna respuesta."
                .to_string()
        })?;
    debug_assert_eq!(digest.len(), 16);

    if inventory_fingerprint(project_files) != expected {
        return Err(
            "El proyecto guardado ya no coincide con el inventario del manifest (ruta, ID u original). Regenera la muestra con qa_prepare; no se aplicó ninguna respuesta."
                .to_string(),
        );
    }

    Ok(expected)
}

pub fn verify_source_inventory_fingerprint(
    expected: &str,
    source_files: &[TranslationFile],
) -> Result<(), String> {
    if inventory_fingerprint(source_files) != expected {
        return Err(
            "La fuente o el parser actual ya no coincide con el inventario del manifest (ruta, ID u original). Regenera la muestra con qa_prepare; no se aplicó ninguna respuesta."
                .to_string(),
        );
    }

    Ok(())
}

pub fn verify_batch_inventory_coverage(
    project_files: &[TranslationFile],
    batches: &[Vec<AiBatchEntry>],
) -> Result<(), String> {
    let mut expected = HashMap::new();
    for file in project_files {
        for entry in &file.entries {
            if entry.original.trim().is_empty() {
                continue;
            }

            let key = (file.relative_path.as_str(), entry.id.as_str());
            if expected.insert(key, entry.original.as_str()).is_some() {
                return Err(format!(
                    "El proyecto contiene una identidad de entrada duplicada: {} :: {}. Regenera la muestra.",
                    file.relative_path, entry.id
                ));
            }
        }
    }

    let mut seen = HashSet::new();
    for entry in batches.iter().flatten() {
        let key = (entry.file_path.as_str(), entry.entry_id.as_str());
        if !seen.insert(key) {
            return Err(format!(
                "Los lotes contienen una entrada duplicada: {} :: {}. Regenera la muestra con qa_prepare.",
                entry.file_path, entry.entry_id
            ));
        }
        match expected.get(&key) {
            None => {
                return Err(format!(
                    "Los lotes contienen una entrada desconocida: {} :: {}. Regenera la muestra con qa_prepare.",
                    entry.file_path, entry.entry_id
                ))
            }
            Some(original) if *original != entry.original.as_str() => {
                return Err(format!(
                    "El original del lote no coincide para {} :: {}. Regenera la muestra con qa_prepare.",
                    entry.file_path, entry.entry_id
                ))
            }
            Some(_) => {}
        }
    }

    if seen.len() != expected.len() {
        let missing = expected
            .keys()
            .find(|key| !seen.contains(*key))
            .map(|(path, id)| format!("{path} :: {id}"))
            .unwrap_or_else(|| "entrada desconocida".to_string());
        return Err(format!(
            "Los lotes no cubren todo el inventario preparado; falta {missing}. Regenera la muestra con qa_prepare."
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_lib::models::{EntryStatus, TranslationEntry};

    fn entry(id: &str, original: &str, translation: &str) -> TranslationEntry {
        TranslationEntry {
            id: id.to_string(),
            file_context: "ignored context".to_string(),
            original: original.to_string(),
            translation: translation.to_string(),
            status: EntryStatus::Reviewed,
            notes: "ignored notes".to_string(),
        }
    }

    fn file(path: &str, entries: Vec<TranslationEntry>) -> TranslationFile {
        TranslationFile {
            relative_path: path.to_string(),
            entries,
        }
    }

    fn inventory() -> Vec<TranslationFile> {
        vec![
            file("data/Map002.json", vec![entry("event:2", "World", "Mundo")]),
            file(
                "data/Map001.json",
                vec![
                    entry("event:2", "Bye", "Adiós"),
                    entry("event:1", "Hello", "Hola"),
                ],
            ),
        ]
    }

    #[test]
    fn fingerprint_is_canonical_across_file_and_entry_order() {
        let expected = inventory_fingerprint(&inventory());
        let mut reordered = inventory();
        reordered.reverse();
        reordered[1].entries.reverse();

        assert_eq!(inventory_fingerprint(&reordered), expected);
    }

    #[test]
    fn fingerprint_v1_contract_is_stable() {
        assert_eq!(
            inventory_fingerprint(&inventory()),
            "fairtl-inventory-v1-fnv1a64:2d7921a256d4ca25"
        );
    }

    #[test]
    fn fingerprint_tracks_path_id_and_original_only() {
        let base = inventory_fingerprint(&inventory());

        let mut changed_path = inventory();
        changed_path[0].relative_path = "data/Map003.json".to_string();
        assert_ne!(inventory_fingerprint(&changed_path), base);

        let mut changed_id = inventory();
        changed_id[0].entries[0].id = "event:3".to_string();
        assert_ne!(inventory_fingerprint(&changed_id), base);

        let mut changed_original = inventory();
        changed_original[0].entries[0].original = "Changed".to_string();
        assert_ne!(inventory_fingerprint(&changed_original), base);

        let mut changed_translation = inventory();
        changed_translation[0].entries[0].translation = "Otra".to_string();
        changed_translation[0].entries[0].notes = "other notes".to_string();
        changed_translation[0].entries[0].status = EntryStatus::Final;
        assert_eq!(inventory_fingerprint(&changed_translation), base);
    }

    #[test]
    fn verification_rejects_legacy_unknown_and_drifted_inventory() {
        let files = inventory();
        let fingerprint = inventory_fingerprint(&files);

        let legacy = verify_project_inventory_fingerprint(None, &files).unwrap_err();
        assert!(legacy.contains("qa_prepare"));

        let unknown =
            verify_project_inventory_fingerprint(Some("future-v2:value"), &files).unwrap_err();
        assert!(unknown.contains("no compatible"));

        let mut changed_project = inventory();
        changed_project[0].entries[0].original = "Project drift".to_string();
        let project_error =
            verify_project_inventory_fingerprint(Some(&fingerprint), &changed_project).unwrap_err();
        assert!(project_error.contains("proyecto guardado"));

        let mut changed_source = inventory();
        changed_source[0].entries[0].original = "Source drift".to_string();
        let source_error =
            verify_source_inventory_fingerprint(&fingerprint, &changed_source).unwrap_err();
        assert!(source_error.contains("fuente o el parser"));
    }

    #[test]
    fn batch_coverage_requires_every_entry_exactly_once() {
        let files = inventory();
        let rows = vec![vec![
            AiBatchEntry {
                file_path: "data/Map001.json".into(),
                entry_id: "event:1".into(),
                file_context: String::new(),
                original: "Hello".into(),
                current_translation: String::new(),
                instruction_candidate: false,
                instruction_reason: None,
            },
            AiBatchEntry {
                file_path: "data/Map001.json".into(),
                entry_id: "event:2".into(),
                file_context: String::new(),
                original: "Bye".into(),
                current_translation: String::new(),
                instruction_candidate: false,
                instruction_reason: None,
            },
            AiBatchEntry {
                file_path: "data/Map002.json".into(),
                entry_id: "event:2".into(),
                file_context: String::new(),
                original: "World".into(),
                current_translation: String::new(),
                instruction_candidate: false,
                instruction_reason: None,
            },
        ]];

        assert!(verify_batch_inventory_coverage(&files, &rows).is_ok());

        let mut missing = rows.clone();
        missing[0].pop();
        assert!(verify_batch_inventory_coverage(&files, &missing)
            .unwrap_err()
            .contains("falta"));

        let mut duplicate = rows.clone();
        let repeated = duplicate[0][0].clone();
        duplicate[0].push(repeated);
        assert!(verify_batch_inventory_coverage(&files, &duplicate)
            .unwrap_err()
            .contains("duplicada"));

        let mut changed = rows;
        changed[0][0].original = "Changed".into();
        assert!(verify_batch_inventory_coverage(&files, &changed)
            .unwrap_err()
            .contains("original"));
    }
}
