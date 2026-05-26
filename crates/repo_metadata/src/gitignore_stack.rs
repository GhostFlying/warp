use std::path::Path;

use ignore::gitignore::Gitignore;

pub struct GitignoreStack {
    gitignores: Vec<Gitignore>,
    active_indices: Vec<usize>,
}

impl GitignoreStack {
    pub fn new(gitignores: Vec<Gitignore>) -> Self {
        let active_indices = (0..gitignores.len()).collect();
        Self {
            gitignores,
            active_indices,
        }
    }

    pub fn into_gitignores(self) -> Vec<Gitignore> {
        self.gitignores
    }

    pub fn active_len(&self) -> usize {
        self.active_indices.len()
    }

    pub fn truncate_active(&mut self, active_len: usize) {
        self.active_indices.truncate(active_len);
    }

    pub fn push_active(&mut self, gitignore: Gitignore) {
        let index = self.gitignores.len();
        self.gitignores.push(gitignore);
        self.active_indices.push(index);
    }

    pub fn matches(&self, path: &Path, is_dir: bool, check_ancestors: bool) -> bool {
        self.active_indices.iter().any(|index| {
            gitignore_matches_path(&self.gitignores[*index], path, is_dir, check_ancestors)
        })
    }
}

pub(crate) fn gitignore_matches_path(
    gitignore: &Gitignore,
    path: &Path,
    is_dir: bool,
    check_ancestors: bool,
) -> bool {
    if let Ok(relative_path) = path.strip_prefix(gitignore.path()) {
        // `matched_path_or_any_parents` panics if the path has a root.
        // If not on windows, we allow paths with a root if the gitignore path is empty (since this denotes a global gitignore).
        if relative_path.has_root() && (cfg!(windows) || gitignore.path() != Path::new("")) {
            return false;
        }

        if check_ancestors {
            gitignore
                .matched_path_or_any_parents(relative_path, is_dir)
                .is_ignore()
        } else {
            gitignore.matched(relative_path, is_dir).is_ignore()
        }
    } else {
        false
    }
}
