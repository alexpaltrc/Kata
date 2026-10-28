//! Starting Kata when the user signs in to Windows, through the package's startup
//! task. Only installed Kata has one: run from the build, it never starts with Windows.

use windows::{
    ApplicationModel::{Activation::ActivationKind, AppInstance, StartupTask, StartupTaskState},
    core::{Result, h},
};

use crate::report;

/// Matches the `TaskId` in `packaging/AppxManifest.xml`.
fn task() -> Result<StartupTask> {
    StartupTask::GetAsync(h!("Startup"))?.join()
}

fn enabled(state: StartupTaskState) -> bool {
    state == StartupTaskState::Enabled || state == StartupTaskState::EnabledByPolicy
}

pub fn is_on() -> bool {
    task().and_then(|task| task.State()).is_ok_and(enabled)
}

/// Turns it on or off, and returns whether it ended up on: turned off in Windows'
/// own settings, it stays off until turned on there.
pub fn set(on: bool) -> bool {
    let state = task().and_then(|task| {
        if on {
            task.RequestEnableAsync()?.join()
        } else {
            task.Disable()?;
            task.State()
        }
    });
    match state {
        Ok(state) => enabled(state),
        Err(error) => {
            report("startup", &error);
            false
        }
    }
}

/// Whether Windows started Kata as the user signed in.
pub fn at_sign_in() -> bool {
    AppInstance::GetActivatedEventArgs()
        .and_then(|args| args.Kind())
        .is_ok_and(|kind| kind == ActivationKind::StartupTask)
}
