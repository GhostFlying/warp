use ai::project_context::model::{ProjectContextModel, ProjectRule};
use remote_server::proto::{file_context_proto, ReadFileContextFile, ReadFileContextRequest};
use repo_metadata::{
    local_model::{GetContentsArgs, IndexedRepoState},
    RepoContent, RepoMetadataModel, RepositoryCoverage, RepositoryIdentifier,
};
use std::collections::HashMap;
use warp_util::{local_or_remote_path::LocalOrRemotePath, remote_path::RemotePath};
use warpui::{AppContext, Entity, ModelContext, SingletonEntity};

use crate::remote_server::manager::RemoteServerManager;

pub(crate) struct RemoteProjectRulesModel {
    refresh_generations: HashMap<RepositoryIdentifier, u64>,
    next_refresh_generation: u64,
}

impl RemoteProjectRulesModel {
    pub(crate) fn new(ctx: &mut ModelContext<Self>) -> Self {
        let repo_metadata = RepoMetadataModel::handle(ctx);
        ctx.subscribe_to_model(&repo_metadata, |me, event, ctx| {
            me.handle_repo_metadata_event(event, ctx);
        });

        let repo_metadata = RepoMetadataModel::as_ref(ctx);
        let mut repo_ids = repo_metadata.local_repository_ids(ctx);
        repo_ids.extend(
            repo_metadata
                .remote_repository_ids(ctx)
                .cloned()
                .map(RepositoryIdentifier::Remote),
        );
        let mut model = Self {
            refresh_generations: HashMap::new(),
            next_refresh_generation: 0,
        };
        for repo_id in repo_ids {
            model.refresh_repository(repo_id, ctx);
        }
        model
    }

    fn handle_repo_metadata_event(
        &mut self,
        event: &repo_metadata::wrapper_model::RepoMetadataEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        use repo_metadata::wrapper_model::RepoMetadataEvent;

        match event {
            RepoMetadataEvent::RepositoryUpdated { id: repo_id }
            | RepoMetadataEvent::FileTreeEntryUpdated { id: repo_id } => {
                self.refresh_repository(repo_id.clone(), ctx);
            }
            RepoMetadataEvent::FileTreeUpdated { ids } => {
                for repo_id in ids
                    .iter()
                    .filter(|repo_id| matches!(repo_id, RepositoryIdentifier::Remote(_)))
                {
                    self.refresh_repository(repo_id.clone(), ctx);
                }
            }
            RepoMetadataEvent::RepositoryRemoved { id: repo_id } => {
                self.refresh_generations.remove(repo_id);
                if let Some(root_path) = repo_id.to_local_or_remote_path() {
                    ProjectContextModel::handle(ctx).update(ctx, |model, ctx| {
                        model.replace_rules_for_metadata_root(root_path, Vec::new(), ctx);
                    });
                }
            }
            RepoMetadataEvent::UpdatingRepositoryFailed { id } => {
                self.refresh_repository(id.clone(), ctx);
            }
            RepoMetadataEvent::IncrementalUpdateReady { .. } => {}
        }
    }

    fn refresh_repository(&mut self, repo_id: RepositoryIdentifier, ctx: &mut ModelContext<Self>) {
        if let RepositoryIdentifier::Local(_) = &repo_id {
            let metadata = RepoMetadataModel::as_ref(ctx);
            match metadata.local_repository_coverage(&repo_id, ctx) {
                Some(RepositoryCoverage::Complete) => {}
                Some(RepositoryCoverage::Degraded) => {
                    self.refresh_local_repository_from_filesystem(&repo_id, ctx);
                    return;
                }
                None if matches!(
                    metadata.repository_state(&repo_id, ctx),
                    Some(IndexedRepoState::Failed(_))
                ) =>
                {
                    self.refresh_local_repository_from_filesystem(&repo_id, ctx);
                    return;
                }
                None => return,
            }
        }
        let refresh_generation = self.advance_refresh_generation(&repo_id);
        let Some(root_path) = repo_id.to_local_or_remote_path() else {
            return;
        };
        let rule_paths =
            find_project_rule_files_in_tree(&repo_id, RepoMetadataModel::as_ref(ctx), ctx);

        if rule_paths.is_empty() {
            self.replace_rules_if_current(&repo_id, refresh_generation, root_path, Vec::new(), ctx);
            return;
        }
        if matches!(repo_id, RepositoryIdentifier::Local(_)) {
            let repo_id_for_result = repo_id.clone();
            ctx.spawn(
                async move {
                    let rules = hydrate_local_project_rules(rule_paths).await;
                    (root_path, rules)
                },
                move |me, (root_path, rules), ctx| {
                    me.replace_rules_if_current(
                        &repo_id_for_result,
                        refresh_generation,
                        root_path,
                        rules,
                        ctx,
                    );
                },
            );
            return;
        }
        let RepositoryIdentifier::Remote(remote_root) = &repo_id else {
            unreachable!("local repositories return after local hydration");
        };
        let Some(client) = RemoteServerManager::as_ref(ctx)
            .client_for_host(&remote_root.host_id)
            .cloned()
        else {
            return;
        };
        let repo_id_for_result = repo_id.clone();

        ctx.spawn(
            async move {
                let request = ReadFileContextRequest {
                    files: rule_paths
                        .iter()
                        .filter_map(|path| match path {
                            LocalOrRemotePath::Remote(remote) => Some(ReadFileContextFile {
                                path: remote.path.as_str().to_string(),
                                line_ranges: Vec::new(),
                            }),
                            LocalOrRemotePath::Local(_) => None,
                        })
                        .collect(),
                    max_file_bytes: None,
                    max_batch_bytes: None,
                };
                let response = client.read_file_context(request).await?;
                let content_by_path = response
                    .file_contexts
                    .into_iter()
                    .filter_map(|file_context| {
                        let file_context_proto::Content::TextContent(content) =
                            file_context.content?
                        else {
                            return None;
                        };
                        Some((file_context.file_name, content))
                    })
                    .collect::<HashMap<_, _>>();
                let rules = rule_paths
                    .into_iter()
                    .filter_map(|path| {
                        let LocalOrRemotePath::Remote(remote) = &path else {
                            return None;
                        };
                        let content = content_by_path.get(remote.path.as_str())?.clone();
                        Some(ProjectRule { path, content })
                    })
                    .collect();
                Ok::<(LocalOrRemotePath, Vec<ProjectRule>), anyhow::Error>((root_path, rules))
            },
            move |me, hydrated_rules, ctx| match hydrated_rules {
                Ok((root_path, rules)) => {
                    me.replace_rules_if_current(
                        &repo_id_for_result,
                        refresh_generation,
                        root_path,
                        rules,
                        ctx,
                    );
                }
                Err(err) => log::warn!("Failed to read remote project rules: {err}"),
            },
        );
    }

    fn refresh_local_repository_from_filesystem(
        &mut self,
        repo_id: &RepositoryIdentifier,
        ctx: &mut ModelContext<Self>,
    ) {
        self.advance_refresh_generation(repo_id);
        let Some(root_path) = repo_id.local_path_buf() else {
            return;
        };
        ProjectContextModel::handle(ctx).update(ctx, |model, ctx| {
            model.use_filesystem_fallback_for_root(&root_path);
            if let Err(error) = model.index_and_store_rules(root_path, ctx) {
                log::warn!("Failed to index project rules from local fallback: {error}");
            }
        });
    }

    fn advance_refresh_generation(&mut self, repo_id: &RepositoryIdentifier) -> u64 {
        self.next_refresh_generation += 1;
        self.refresh_generations
            .insert(repo_id.clone(), self.next_refresh_generation);
        self.next_refresh_generation
    }

    fn replace_rules_if_current(
        &mut self,
        repo_id: &RepositoryIdentifier,
        refresh_generation: u64,
        root_path: LocalOrRemotePath,
        rules: Vec<ProjectRule>,
        ctx: &mut ModelContext<Self>,
    ) {
        if self.refresh_generations.get(repo_id) != Some(&refresh_generation) {
            return;
        }

        ProjectContextModel::handle(ctx).update(ctx, |model, ctx| {
            model.replace_rules_for_metadata_root(root_path, rules, ctx);
        });
    }
}

impl Entity for RemoteProjectRulesModel {
    type Event = ();
}

impl SingletonEntity for RemoteProjectRulesModel {}

fn find_project_rule_files_in_tree(
    repo_id: &RepositoryIdentifier,
    repo_metadata: &RepoMetadataModel,
    ctx: &AppContext,
) -> Vec<LocalOrRemotePath> {
    let args = GetContentsArgs::default()
        .include_ignored()
        .with_filter(move |content| {
            let RepoContent::File(file) = content else {
                return false;
            };
            matches_project_rule_file(file.path.file_name())
        });

    repo_metadata
        .get_repo_contents(repo_id, args, ctx)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|content| {
            let RepoContent::File(file) = content else {
                return None;
            };
            match repo_id {
                RepositoryIdentifier::Local(_) => {
                    file.path.to_local_path().map(LocalOrRemotePath::Local)
                }
                RepositoryIdentifier::Remote(remote_root) => Some(LocalOrRemotePath::Remote(
                    RemotePath::new(remote_root.host_id.clone(), file.path.as_ref().clone()),
                )),
            }
        })
        .collect()
}

async fn hydrate_local_project_rules(rule_paths: Vec<LocalOrRemotePath>) -> Vec<ProjectRule> {
    let mut rules = Vec::new();
    for path in rule_paths {
        let Some(local_path) = path.to_local_path() else {
            continue;
        };
        match async_fs::read_to_string(local_path).await {
            Ok(content) => rules.push(ProjectRule { path, content }),
            Err(error) => {
                log::warn!(
                    "Failed to read metadata-backed local project rule {}: {error}",
                    local_path.display()
                );
            }
        }
    }
    rules
}

fn matches_project_rule_file(file_name: Option<&str>) -> bool {
    file_name.is_some_and(|file_name| {
        file_name.eq_ignore_ascii_case("WARP.md") || file_name.eq_ignore_ascii_case("AGENTS.md")
    })
}

#[cfg(test)]
#[path = "remote_project_rules_tests.rs"]
mod tests;
