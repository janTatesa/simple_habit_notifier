use clap::{CommandFactory, Parser};
use clap_complete::{ArgValueCompleter, CompleteEnv, CompletionCandidate, Shell};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};

use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs,
    path::PathBuf,
    sync::LazyLock,
    time::Duration,
};

use jiff::{Zoned, civil::Date};

static HABITS_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| dirs::config_dir().unwrap().join("habits"));
fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    CompleteEnv::with_factory(Cli::command).complete();

    match Cli::parse().subcommand {
        Subcommand::Daemon { delay_mins } => loop {
            let state = State::load()?;
            let now = Zoned::now().date();
            let habits = fs::read_to_string(&*HABITS_PATH)?
                .lines()
                .filter(|habit| {
                    !habit.is_empty() && (!state.checked.contains(*habit) || state.last_date != now)
                })
                .collect::<Vec<_>>()
                .join(", ");
            notify_rust::Notification::new()
                .summary("Habit reminder")
                .body(&habits)
                .sound_name("dialog-information")
                .show()?;
            std::thread::sleep(Duration::from_mins(delay_mins.into()));
        },
        Subcommand::Show => {
            let state = State::load()?;
            let now = Zoned::now().date();
            let habits = fs::read_to_string(&*HABITS_PATH)?
                .lines()
                .filter(|habit| {
                    !habit.is_empty() && (!state.checked.contains(*habit) || state.last_date != now)
                })
                .collect::<Vec<_>>()
                .join(&" ∙ ".bright_black().to_string());
            println!("{habits}",)
        }
        Subcommand::Check { habit } => {
            let file = fs::read_to_string(&*HABITS_PATH)?;
            let habits: Vec<_> = file.lines().filter(|habit| !habit.is_empty()).collect();
            if !habits.contains(&&*habit) {
                exit_with_error("Habit doesn't exist")
            }

            let mut state = State::load()?;
            let now = Zoned::now().date();

            if state.checked.contains(&*habit) && state.last_date == now {
                exit_with_error("Habit is already checked")
            }

            if state.last_date != now {
                state.checked = HashSet::new();
                state.last_date = now;
            }

            state.checked.insert(habit);
            fs::write(&*STATE_PATH, serde_json::to_string_pretty(&state)?)?;
        }
        Subcommand::Completion { shell } => {
            let completions = match shell {
                Shell::Bash => "source <(COMPLETE=bash shn)",
                Shell::Elvish => "eval (E:COMPLETE=elvish shn | slurp)",
                Shell::Fish => "COMPLETE=fish shn | source",
                Shell::PowerShell => {
                    "$env:COMPLETE = \"powershell\"; shn | Out-String | Invoke-Expression; Remove-Item Env:\\COMPLETE"
                }
                Shell::Zsh => "source <(COMPLETE=zsh shn)",
                _ => {
                    exit_with_error("Unsupported shell");
                }
            };
            println!("{completions}");
        }
    }

    Ok(())
}

fn exit_with_error(error: &str) -> ! {
    eprintln!("{}", error.red());
    std::process::exit(1)
}

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    subcommand: Subcommand,
}

#[derive(clap::Subcommand)]
enum Subcommand {
    Daemon {
        #[arg(long, short, default_value_t = 30)]
        delay_mins: u16,
    },
    Show,
    Check {
        #[arg(add = ArgValueCompleter::new(complete_habit))]
        habit: String,
    },
    Completion {
        shell: Shell,
    },
}

fn complete_habit(current: &OsStr) -> Vec<CompletionCandidate> {
    let state = State::load().unwrap();
    let now = Zoned::now().date();

    let current = current.to_str().unwrap();
    fs::read_to_string(&*HABITS_PATH)
        .unwrap()
        .lines()
        .filter(|habit| {
            !habit.is_empty()
                && (!state.checked.contains(*habit) || state.last_date != now)
                && habit.starts_with(current)
        })
        .map(|habit| CompletionCandidate::new(habit))
        .collect()
}

#[derive(Serialize, Deserialize)]
struct State {
    checked: HashSet<String>,
    last_date: Date,
}

static STATE_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| dirs::data_dir().unwrap().join("simple-habit-tracker.json"));
impl State {
    fn load() -> color_eyre::Result<Self> {
        if !STATE_PATH.exists() {
            return Ok(Self {
                checked: HashSet::new(),
                last_date: Zoned::now().date(),
            });
        }

        Ok(serde_json::from_str(&fs::read_to_string(&*STATE_PATH)?)?)
    }
}
