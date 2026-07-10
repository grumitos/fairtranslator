use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::error::AppError;
use crate::models::{GlossaryTerm, Project};
use crate::services::provider::{
    TranslationBatchContext, TranslationProvider, TranslationRequest, TranslationResponse,
};
use crate::services::translation_apply::{self, ProviderApplyResult};

pub(crate) struct BatchRunControl {
    next_run_id: AtomicU64,
    active_run_id: AtomicU64,
    cancelled_run_id: AtomicU64,
}

impl BatchRunControl {
    pub(crate) const fn new() -> Self {
        Self {
            next_run_id: AtomicU64::new(1),
            active_run_id: AtomicU64::new(0),
            cancelled_run_id: AtomicU64::new(0),
        }
    }

    pub(crate) fn try_acquire(&self) -> Result<BatchRunGuard<'_>, AppError> {
        let run_id = self.next_run_id.fetch_add(1, Ordering::Relaxed);
        self.active_run_id
            .compare_exchange(0, run_id, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| AppError::Project("Ya hay una traducción batch en curso".into()))?;
        Ok(BatchRunGuard {
            control: self,
            run_id,
        })
    }

    pub(crate) fn cancel(&self, run_id: u64) -> bool {
        if run_id == 0 || self.active_run_id.load(Ordering::Acquire) != run_id {
            return false;
        }
        self.cancelled_run_id.fetch_max(run_id, Ordering::AcqRel);
        true
    }

    fn is_cancelled(&self, run_id: u64) -> bool {
        self.cancelled_run_id.load(Ordering::Acquire) == run_id
    }
}

pub(crate) struct BatchRunGuard<'a> {
    control: &'a BatchRunControl,
    run_id: u64,
}

impl BatchRunGuard<'_> {
    pub(crate) fn run_id(&self) -> u64 {
        self.run_id
    }
}

impl Drop for BatchRunGuard<'_> {
    fn drop(&mut self) {
        let _ = self.control.active_run_id.compare_exchange(
            self.run_id,
            0,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }
}

pub(crate) struct BatchRunLifecycle<'a> {
    control: &'a BatchRunControl,
    run_id: u64,
    cancelled: bool,
    files_processed: usize,
    chunks_processed: usize,
    file_chunk_totals: HashMap<String, usize>,
    file_chunks_processed: HashMap<String, usize>,
}

impl<'a> BatchRunLifecycle<'a> {
    pub(crate) fn new(control: &'a BatchRunControl, run_id: u64, batch_paths: &[String]) -> Self {
        let mut file_chunk_totals = HashMap::new();
        for path in batch_paths {
            *file_chunk_totals.entry(path.clone()).or_insert(0) += 1;
        }
        Self {
            control,
            run_id,
            cancelled: false,
            files_processed: 0,
            chunks_processed: 0,
            file_chunk_totals,
            file_chunks_processed: HashMap::new(),
        }
    }

    pub(crate) fn should_stop(&mut self) -> bool {
        if self.control.is_cancelled(self.run_id) {
            self.cancelled = true;
        }
        self.cancelled
    }

    pub(crate) fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub(crate) fn record_processed_chunk(&mut self, file_path: &str) -> (usize, usize) {
        self.chunks_processed += 1;
        let processed = self
            .file_chunks_processed
            .entry(file_path.to_string())
            .or_insert(0);
        *processed += 1;
        let total = self.file_chunk_totals.get(file_path).copied().unwrap_or(1);
        if *processed == total {
            self.files_processed += 1;
        }
        (*processed, total)
    }

    pub(crate) fn cancelled(&self) -> bool {
        self.cancelled
    }

    pub(crate) fn files_processed(&self) -> usize {
        self.files_processed
    }

    pub(crate) fn chunks_processed(&self) -> usize {
        self.chunks_processed
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FileTranslationBatch {
    pub(crate) relative_path: String,
    pub(crate) entries: Vec<TranslationRequest>,
    pub(crate) context: TranslationBatchContext,
}

pub(crate) enum BatchChunkOutcome {
    Applied {
        responses: Vec<TranslationResponse>,
        apply_result: ProviderApplyResult,
    },
    Cancelled,
    ProjectChanged(AppError),
    ProviderFailed(AppError),
    ApplyFailed(AppError),
}

fn ensure_expected_project_id(
    current_project_id: Option<&str>,
    expected_project_id: &str,
) -> Result<(), AppError> {
    match current_project_id {
        Some(current) if current == expected_project_id => Ok(()),
        Some(_) => Err(AppError::Project(
            "El proyecto abierto cambió durante la traducción batch".into(),
        )),
        None => Err(AppError::Project(
            "El proyecto se cerró durante la traducción batch".into(),
        )),
    }
}

pub(crate) fn ensure_current_project_id(
    project_state: &Arc<Mutex<Option<Project>>>,
    expected_project_id: &str,
) -> Result<(), AppError> {
    let project = project_state
        .lock()
        .map_err(|error| AppError::Project(format!("Lock error: {error}")))?;
    ensure_expected_project_id(
        project.as_ref().map(|project| project.id.as_str()),
        expected_project_id,
    )
}

pub(crate) async fn run_translation_chunk(
    lifecycle: &mut BatchRunLifecycle<'_>,
    project_state: &Arc<Mutex<Option<Project>>>,
    expected_project_id: &str,
    provider: &dyn TranslationProvider,
    source_lang: &str,
    target_lang: &str,
    glossary: &[GlossaryTerm],
    batch: &FileTranslationBatch,
) -> BatchChunkOutcome {
    if lifecycle.should_stop() {
        return BatchChunkOutcome::Cancelled;
    }
    if let Err(error) = ensure_current_project_id(project_state, expected_project_id) {
        lifecycle.cancel();
        return BatchChunkOutcome::ProjectChanged(error);
    }

    let response_result = provider
        .translate_batch(
            &batch.entries,
            source_lang,
            target_lang,
            glossary,
            &batch.context,
        )
        .await;

    if lifecycle.should_stop() {
        return BatchChunkOutcome::Cancelled;
    }

    let responses = match response_result {
        Ok(responses) => responses,
        Err(error) => return BatchChunkOutcome::ProviderFailed(error),
    };

    let mut project = match project_state
        .lock()
        .map_err(|error| AppError::Project(format!("Lock error: {error}")))
        .and_then(|guard| {
            ensure_expected_project_id(
                guard.as_ref().map(|project| project.id.as_str()),
                expected_project_id,
            )?;
            Ok(guard)
        }) {
        Ok(project) => project,
        Err(error) => {
            lifecycle.cancel();
            return BatchChunkOutcome::ProjectChanged(error);
        }
    };

    if lifecycle.should_stop() {
        return BatchChunkOutcome::Cancelled;
    }

    let apply_result = match project
        .as_mut()
        .ok_or_else(|| AppError::Project("No hay ningún proyecto abierto".into()))
        .and_then(|project| {
            translation_apply::apply_provider_responses(project, &batch.relative_path, &responses)
        }) {
        Ok(result) => result,
        Err(error) => return BatchChunkOutcome::ApplyFailed(error),
    };

    BatchChunkOutcome::Applied {
        responses,
        apply_result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    use crate::models::{Engine, EntryStatus, TranslationEntry, TranslationFile};

    struct CancellingProvider {
        control: Arc<BatchRunControl>,
        run_id: u64,
        calls: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl TranslationProvider for CancellingProvider {
        async fn translate_batch(
            &self,
            requests: &[TranslationRequest],
            _source_lang: &str,
            _target_lang: &str,
            _glossary: &[GlossaryTerm],
            _context: &TranslationBatchContext,
        ) -> Result<Vec<TranslationResponse>, AppError> {
            let call = self.calls.fetch_add(1, Ordering::AcqRel) + 1;
            if call == 2 {
                self.control.cancel(self.run_id);
            }
            Ok(requests
                .iter()
                .map(|request| TranslationResponse {
                    entry_id: request.entry_id.clone(),
                    translated_text: format!("Translated {}", request.entry_id),
                })
                .collect())
        }

        fn name(&self) -> &'static str {
            "test"
        }

        async fn validate(&self) -> Result<(), AppError> {
            Ok(())
        }
    }

    fn make_project() -> Project {
        Project {
            id: "project-1".into(),
            name: "Cancellation Test".into(),
            source_lang: "en".into(),
            target_lang: "es".into(),
            engine: Engine::Mv,
            game_dir: "D:\\Games\\Test".into(),
            files: vec![TranslationFile {
                relative_path: "data/Map001.json".into(),
                entries: (1..=3)
                    .map(|index| TranslationEntry {
                        id: format!("entry-{index}"),
                        file_context: format!("Map001 > text {index}"),
                        original: format!("Source {index}"),
                        translation: String::new(),
                        status: EntryStatus::Untranslated,
                        notes: String::new(),
                    })
                    .collect(),
            }],
            glossary: Vec::new(),
            style_guide: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn make_batches() -> Vec<FileTranslationBatch> {
        (1..=3)
            .map(|index| FileTranslationBatch {
                relative_path: "data/Map001.json".into(),
                entries: vec![TranslationRequest {
                    entry_id: format!("entry-{index}"),
                    file_context: format!("Map001 > text {index}"),
                    text: format!("Source {index}"),
                    notes: String::new(),
                }],
                context: TranslationBatchContext::default(),
            })
            .collect()
    }

    #[tokio::test]
    async fn cancellation_during_second_request_does_not_apply_it_or_later_chunks() {
        let control = Arc::new(BatchRunControl::new());
        let guard = control.try_acquire().unwrap();
        let run_id = guard.run_id();
        let batches = make_batches();
        let paths = batches
            .iter()
            .map(|batch| batch.relative_path.clone())
            .collect::<Vec<_>>();
        let mut lifecycle = BatchRunLifecycle::new(&control, run_id, &paths);
        let project_state = Arc::new(Mutex::new(Some(make_project())));
        let calls = Arc::new(AtomicUsize::new(0));
        let provider = CancellingProvider {
            control: control.clone(),
            run_id,
            calls: calls.clone(),
        };

        for batch in &batches {
            match run_translation_chunk(
                &mut lifecycle,
                &project_state,
                "project-1",
                &provider,
                "en",
                "es",
                &[],
                batch,
            )
            .await
            {
                BatchChunkOutcome::Applied {
                    responses,
                    apply_result,
                } => {
                    assert_eq!(responses.len(), 1);
                    assert_eq!(apply_result.applied_entries, 1);
                    lifecycle.record_processed_chunk(&batch.relative_path);
                }
                BatchChunkOutcome::Cancelled => break,
                BatchChunkOutcome::ProjectChanged(error)
                | BatchChunkOutcome::ProviderFailed(error)
                | BatchChunkOutcome::ApplyFailed(error) => {
                    panic!("unexpected chunk failure: {error}")
                }
            }
        }

        assert!(lifecycle.cancelled());
        assert_eq!(lifecycle.chunks_processed(), 1);
        assert_eq!(lifecycle.files_processed(), 0);
        assert_eq!(calls.load(Ordering::Acquire), 2);
        let project = project_state.lock().unwrap();
        let entries = &project.as_ref().unwrap().files[0].entries;
        assert!(!entries[0].translation.is_empty());
        assert!(entries[1].translation.is_empty());
        assert!(entries[2].translation.is_empty());
        drop(project);

        drop(guard);
        let next = control.try_acquire().unwrap();
        assert!(!control.is_cancelled(next.run_id()));
    }

    #[test]
    fn stale_cancel_cannot_erase_or_target_a_newer_run() {
        let control = BatchRunControl::new();
        let first = control.try_acquire().unwrap();
        let first_id = first.run_id();
        assert!(control.cancel(first_id));
        drop(first);

        let second = control.try_acquire().unwrap();
        let second_id = second.run_id();
        assert!(control.cancel(second_id));
        assert!(!control.cancel(first_id));
        assert!(control.is_cancelled(second_id));
    }
}
