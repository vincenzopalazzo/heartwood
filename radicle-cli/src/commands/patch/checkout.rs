use crate::terminal as term;
use radicle::cob::patch::{self, PatchId};
use radicle::git;
use radicle::profile::Profile;
use radicle::storage::git::Repository;

pub fn run(
    storage: &Repository,
    profile: &Profile,
    git_workdir: &git::raw::Repository,
    patch_id: &PatchId,
) -> anyhow::Result<()> {
    let patchs = patch::Patches::open(profile.public_key, storage)?;
    let Some(patch) = patchs.get(patch_id)? else {
        anyhow::bail!("Patch `{patch_id}` not found");
    };

    let spinner = term::spinner("Performing patch checkout...");

    // Getting the patch obj!
    let patch_haed = patch.head();
    let obj = git_workdir.revparse_single(&patch_haed.to_string())?;

    let branch_name = format!("patch/{}", term::format::cob(patch_id));
    if !find_branch_by_name(git_workdir, &branch_name) {
        // checkout the patch in a new branch!
        git_workdir.branch(&branch_name, &obj.as_commit().unwrap(), false)?;
        // and then point the current `HEAD` inside the new branch.
        git_workdir.set_head(format!("refs/heads/{branch_name}").as_str())?;
    } else {
        term::info!(
            "branch `{}` already exist, checking out",
            term::format::highlight(branch_name.to_owned())
        );
    }
    spinner.finish();

    // 3. Write to the UI Terminal
    term::info!(
        "ok: branch {} created",
        term::format::highlight(branch_name)
    );

    Ok(())
}

// find all the branch and filter it by name can be expensive if you are working
// in a repository bit enought.
//
// So, this will try to switch branch by pointing to all the branch stored inside the
// `refs/heads` and if fails, there is no branch, so we need to checkout it.
fn find_branch_by_name(git_workdir: &git::raw::Repository, branch_name: &str) -> bool {
    git_workdir
        .set_head(format!("refs/heads/{branch_name}").as_str())
        .is_ok()
}
