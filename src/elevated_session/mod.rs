mod filesystem_helper;
mod invoking_user;
mod user_commands;

#[cfg(unix)]
pub(crate) use self::{
    filesystem_helper::{Request, Response, run_as_invoking_user},
    invoking_user::{
        InvocationContext, InvokingUser, SESSION_ENVIRONMENT_KEYS, context, env_var, home_dir,
        trash_home_dir, user_environment_value,
    },
    user_commands::{prepare, prepare_external},
};

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) use self::invoking_user::trash_data_dir;

#[cfg(not(unix))]
pub(crate) use self::invoking_user::{env_var, home_dir, trash_home_dir};

pub(crate) use self::filesystem_helper::run;
