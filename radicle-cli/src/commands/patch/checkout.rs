use anyhow::anyhow;

use radicle::cob::patch;
use radicle::cob::patch::{Patch, PatchId};
use radicle::git::RefString;
use radicle::storage::git::Repository;
use radicle::storage::ReadRepository;
use radicle::{git, rad};

use crate::terminal as term;

pub fn run(
    patch_id: &PatchId,
    stored: &Repository,
    working: &git::raw::Repository,
) -> anyhow::Result<()> {
    let patches = patch::Patches::open(stored)?;
    let patch = patches
        .get(patch_id)?
        .ok_or_else(|| anyhow!("Patch `{patch_id}` not found"))?;

    let mut spinner = term::spinner("Performing checkout...");
    let patch_branch =
        // SAFETY: Patch IDs are valid refstrings.
        git::refname!("patch").join(RefString::try_from(term::format::cob(patch_id)).unwrap());
    let commit = find_patch_commit(&patch, stored, working)?;

    // Create patch branch and switch to it.
    working.branch(patch_branch.as_str(), &commit, true)?;
    working.checkout_tree(commit.as_object(), None)?;
    working.set_head(&git::refs::workdir::branch(&patch_branch))?;

    spinner.message(format!(
        "Switched to branch {}",
        term::format::highlight(&patch_branch)
    ));
    spinner.finish();

    if let Some(branch) = rad::setup_patch_upstream(patch_id, *patch.head(), working)? {
        let tracking = branch
            .name()?
            .ok_or_else(|| anyhow!("failed to create tracking branch: invalid name"))?;
        term::success!(
            "Branch {} setup to track {}",
            term::format::highlight(patch_branch),
            term::format::tertiary(tracking)
        );
    }
    Ok(())
}

/// Try to find the patch head in our working copy, and if we don't find it,
/// fetch it from storage first.
fn find_patch_commit<'a>(
    patch: &Patch,
    stored: &Repository,
    working: &'a git::raw::Repository,
) -> anyhow::Result<git::raw::Commit<'a>> {
    let patch_head = *patch.head();

    match working.find_commit(patch_head.into()) {
        Ok(commit) => Ok(commit),
        Err(e) if git::ext::is_not_found_err(&e) => {
            let url = git::url::File::new(stored.path());

            working.remote_anonymous(url.to_string().as_str())?.fetch(
                &[patch_head.to_string()],
                None,
                None,
            )?;
            working.find_commit(patch_head.into()).map_err(|e| e.into())
        }
        Err(e) => Err(e.into()),
    }
}
