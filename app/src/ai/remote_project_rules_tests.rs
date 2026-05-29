use std::collections::HashMap;
use std::fs;
use std::time::Duration;

use ai::project_context::model::{ProjectContextModel, ProjectContextModelEvent, ProjectRule};
use repo_metadata::entry::{DirectoryEntry, Entry, FileMetadata};
use repo_metadata::file_tree_store::FileTreeState;
use repo_metadata::repositories::DetectedRepositories;
use repo_metadata::{
    DirectoryWatcher, RepoMetadataModel, RepositoryCoverage, RepositoryIdentifier,
};
use tempfile::TempDir;
use warp_util::local_or_remote_path::LocalOrRemotePath;
use warp_util::standardized_path::StandardizedPath;
use warpui::{App, Entity, ModelContext, SingletonEntity};

use super::RemoteProjectRulesModel;

struct RulesIndexedListener;

impl RulesIndexedListener {
    fn new(indexed_tx: async_channel::Sender<()>, ctx: &mut ModelContext<Self>) -> Self {
        ctx.subscribe_to_model(&ProjectContextModel::handle(ctx), move |_, event, _| {
            if matches!(event, ProjectContextModelEvent::PathIndexed) {
                let _ = indexed_tx.try_send(());
            }
        });
        Self
    }
}

impl Entity for RulesIndexedListener {
    type Event = ();
}

struct RulesDeltaListener;

impl RulesDeltaListener {
    fn new(
        deleted_tx: async_channel::Sender<Vec<std::path::PathBuf>>,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        ctx.subscribe_to_model(&ProjectContextModel::handle(ctx), move |_, event, _| {
            if let ProjectContextModelEvent::KnownRulesChanged(delta) = event {
                let _ = deleted_tx.try_send(delta.deleted_rules.clone());
            }
        });
        Self
    }
}

impl Entity for RulesDeltaListener {
    type Event = ();
}

fn local_rules_model() -> RemoteProjectRulesModel {
    RemoteProjectRulesModel {
        refresh_generations: HashMap::new(),
        next_refresh_generation: 0,
    }
}

fn local_rule_state(repo: &std::path::Path, rule_path: Option<&std::path::Path>) -> FileTreeState {
    let children = rule_path
        .map(|rule_path| {
            vec![Entry::File(FileMetadata::new(
                rule_path.to_path_buf(),
                false,
            ))]
        })
        .unwrap_or_default();
    let root = Entry::Directory(DirectoryEntry {
        path: StandardizedPath::try_from_local(repo).unwrap(),
        children,
        ignored: false,
        loaded: true,
    });
    FileTreeState::new(root, Vec::new(), None)
}

#[test]
fn complete_local_repository_hydrates_rules_from_metadata_tree() {
    let (indexed_tx, indexed_rx) = async_channel::unbounded();

    App::test((), |mut app| async move {
        app.add_singleton_model(|_| DetectedRepositories::default());
        let project_context = app.add_singleton_model(|_| ProjectContextModel::default());
        let repo_metadata = app.add_singleton_model(RepoMetadataModel::new);
        let _listener = app.add_model(|ctx| RulesIndexedListener::new(indexed_tx, ctx));
        let rules_model = app.add_model(|_| local_rules_model());

        let temp_dir = TempDir::new().unwrap();
        let repo = temp_dir.path().to_path_buf();
        let rule_path = repo.join("WARP.md");
        fs::write(&rule_path, "metadata rule").unwrap();
        let repo_id = RepositoryIdentifier::try_local(&repo).unwrap();
        repo_metadata.update(&mut app, |model, ctx| {
            model.insert_test_state_with_coverage(
                StandardizedPath::try_from_local(&repo).unwrap(),
                local_rule_state(&repo, Some(&rule_path)),
                RepositoryCoverage::Complete,
                ctx,
            );
        });

        rules_model.update(&mut app, |model, ctx| {
            model.refresh_repository(repo_id, ctx);
        });
        indexed_rx.recv().await.unwrap();

        project_context.read(&app, |model, _| {
            let result = model
                .find_applicable_project_rules(&repo.join("src/main.rs"))
                .expect("metadata-hydrated local project rule should apply");
            assert_eq!(result.active_rules.len(), 1);
            assert_eq!(result.active_rules[0].content, "metadata rule");
        });
    });
}

#[test]
fn failed_local_repository_uses_filesystem_rule_fallback() {
    let (indexed_tx, indexed_rx) = async_channel::unbounded();

    App::test((), |mut app| async move {
        app.add_singleton_model(DirectoryWatcher::new_for_testing);
        app.add_singleton_model(|_| DetectedRepositories::default());
        let project_context = app.add_singleton_model(|_| ProjectContextModel::default());
        let repo_metadata = app.add_singleton_model(RepoMetadataModel::new);
        let _listener = app.add_model(|ctx| RulesIndexedListener::new(indexed_tx, ctx));
        let rules_model = app.add_model(|_| local_rules_model());

        let temp_dir = TempDir::new().unwrap();
        let repo = dunce::canonicalize(temp_dir.path()).unwrap();
        let rule_path = repo.join("WARP.md");
        fs::write(&rule_path, "failed fallback rule").unwrap();
        let repo_id = RepositoryIdentifier::try_local(&repo).unwrap();
        repo_metadata.update(&mut app, |model, ctx| {
            model.insert_test_failed_state(StandardizedPath::try_from_local(&repo).unwrap(), ctx);
        });

        rules_model.update(&mut app, |model, ctx| {
            model.refresh_repository(repo_id, ctx);
        });
        indexed_rx.recv().await.unwrap();

        project_context.read(&app, |model, _| {
            let result = model
                .find_applicable_project_rules(&repo.join("main.rs"))
                .expect("failed-metadata local project rule fallback should apply");
            assert_eq!(result.active_rules.len(), 1);
            assert_eq!(result.active_rules[0].content, "failed fallback rule");
        });
    });
}

#[test]
fn degraded_local_fallback_rescan_persists_deleted_rule_paths() {
    let (indexed_tx, indexed_rx) = async_channel::unbounded();
    let (deleted_tx, deleted_rx) = async_channel::unbounded();

    App::test((), |mut app| async move {
        app.add_singleton_model(DirectoryWatcher::new_for_testing);
        app.add_singleton_model(|_| DetectedRepositories::default());
        let project_context = app.add_singleton_model(|_| ProjectContextModel::default());
        let repo_metadata = app.add_singleton_model(RepoMetadataModel::new);
        let _indexed_listener = app.add_model(|ctx| RulesIndexedListener::new(indexed_tx, ctx));
        let _deleted_listener = app.add_model(|ctx| RulesDeltaListener::new(deleted_tx, ctx));
        let rules_model = app.add_model(|_| local_rules_model());

        let temp_dir = TempDir::new().unwrap();
        let repo = dunce::canonicalize(temp_dir.path()).unwrap();
        let rule_path = repo.join("WARP.md");
        fs::write(&rule_path, "fallback rule").unwrap();
        let repo_id = RepositoryIdentifier::try_local(&repo).unwrap();
        repo_metadata.update(&mut app, |model, ctx| {
            model.insert_test_state_with_coverage(
                StandardizedPath::try_from_local(&repo).unwrap(),
                local_rule_state(&repo, None),
                RepositoryCoverage::Degraded,
                ctx,
            );
        });

        rules_model.update(&mut app, |model, ctx| {
            model.refresh_repository(repo_id.clone(), ctx);
        });
        indexed_rx.recv().await.unwrap();
        assert!(deleted_rx.recv().await.unwrap().is_empty());

        fs::remove_file(&rule_path).unwrap();
        rules_model.update(&mut app, |model, ctx| {
            model.refresh_repository(repo_id, ctx);
        });
        indexed_rx.recv().await.unwrap();
        assert_eq!(deleted_rx.recv().await.unwrap(), vec![rule_path]);
        project_context.read(&app, |model, _| {
            assert!(model
                .find_applicable_project_rules(&repo.join("main.rs"))
                .is_none());
        });
    });
}

#[test]
fn metadata_replacement_wins_over_in_flight_local_fallback_scan() {
    let (indexed_tx, indexed_rx) = async_channel::unbounded();

    App::test((), |mut app| async move {
        let project_context = app.add_singleton_model(|_| ProjectContextModel::default());
        let _listener = app.add_model(|ctx| RulesIndexedListener::new(indexed_tx, ctx));
        let temp_dir = TempDir::new().unwrap();
        let repo = dunce::canonicalize(temp_dir.path()).unwrap();
        let rule_path = repo.join("WARP.md");
        fs::write(&rule_path, "fallback rule").unwrap();

        project_context.update(&mut app, |model, ctx| {
            model.index_and_store_rules(repo.clone(), ctx).unwrap();
            model.replace_rules_for_metadata_root(
                LocalOrRemotePath::Local(repo.clone()),
                vec![ProjectRule {
                    path: LocalOrRemotePath::Local(rule_path.clone()),
                    content: "metadata rule".to_string(),
                }],
                ctx,
            );
        });
        indexed_rx.recv().await.unwrap();
        async_io::Timer::after(Duration::from_millis(20)).await;

        project_context.read(&app, |model, _| {
            let result = model
                .find_applicable_project_rules(&repo.join("main.rs"))
                .expect("metadata rule should remain applied");
            assert_eq!(result.active_rules[0].content, "metadata rule");
        });
        assert!(indexed_rx.try_recv().is_err());
    });
}

#[test]
fn degraded_local_repository_uses_filesystem_rule_fallback() {
    let (indexed_tx, indexed_rx) = async_channel::unbounded();

    App::test((), |mut app| async move {
        app.add_singleton_model(DirectoryWatcher::new_for_testing);
        app.add_singleton_model(|_| DetectedRepositories::default());
        let project_context = app.add_singleton_model(|_| ProjectContextModel::default());
        let repo_metadata = app.add_singleton_model(RepoMetadataModel::new);
        let _listener = app.add_model(|ctx| RulesIndexedListener::new(indexed_tx, ctx));
        let rules_model = app.add_model(|_| local_rules_model());

        let temp_dir = TempDir::new().unwrap();
        let repo = dunce::canonicalize(temp_dir.path()).unwrap();
        let nested_dir = repo.join("src");
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(nested_dir.join("WARP.md"), "fallback rule").unwrap();
        let repo_id = RepositoryIdentifier::try_local(&repo).unwrap();
        repo_metadata.update(&mut app, |model, ctx| {
            model.insert_test_state_with_coverage(
                StandardizedPath::try_from_local(&repo).unwrap(),
                local_rule_state(&repo, None),
                RepositoryCoverage::Degraded,
                ctx,
            );
        });

        rules_model.update(&mut app, |model, ctx| {
            model.refresh_repository(repo_id, ctx);
        });
        indexed_rx.recv().await.unwrap();

        project_context.read(&app, |model, _| {
            let result = model
                .find_applicable_project_rules(&nested_dir.join("main.rs"))
                .expect("filesystem-fallback local project rule should apply");
            assert_eq!(result.active_rules.len(), 1);
            assert_eq!(result.active_rules[0].content, "fallback rule");
        });
    });
}
