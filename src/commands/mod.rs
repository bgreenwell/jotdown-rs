//! Implementation logic for every `jd` subcommand, grouped by feature area.
//! Re-exported flat here so callers keep using `commands::command_x(...)`.

mod capture;
mod clean;
mod export_import;
mod git;
mod info;
mod note_ops;
mod notebook;
mod pin;
mod property;
mod query;
mod shell;
mod tag;

pub use capture::{command_down, command_edit, command_new, command_task};
pub use clean::command_clean;
pub use export_import::{command_export, command_import};
pub use git::{command_decrypt, command_init, command_sync};
pub use info::command_info;
pub use note_ops::{command_append, command_daily, command_move, command_prepend, command_rename};
pub use notebook::command_notebook;
pub use pin::{command_pin, command_unpin};
pub use property::command_property;
#[cfg(not(windows))]
pub use query::command_select;
pub use query::{
    command_by_week, command_delete, command_find, command_list, command_on, command_show,
    command_tags_filter, command_today, command_yesterday,
};
pub use shell::command_shell;
pub use tag::command_tag;
