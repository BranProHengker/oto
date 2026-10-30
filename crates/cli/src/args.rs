use std::ffi::OsString;
use std::io::{self, Write as _};
use std::path::{self, Path};
use std::process::ExitCode;

use clap::error::ErrorKind;
use clap::{
    Arg, ArgAction, Command, CommandFactory as _, FromArgMatches as _, Parser, Subcommand,
    ValueEnum, ValueHint,
};
use clap_complete::Shell;

use crate::commands::{
    CommandType, Format, Item, Kind, Level, Local, PlaybackCommand, ProviderCommand, QueueCommand,
    Repeat, Request, SearchQuery, Switch,
};
use crate::item;

/// The command line as the user types it. It is flat where `Request` is grouped, so the two
/// shapes are kept apart and joined in `into_command`.
#[derive(Parser)]
#[command(
    name = "sonora",
    about = "A native music streaming client",
    override_usage = "sonora [FILE_OR_LINK]...\n       sonora [--json] <COMMAND>",
    disable_help_subcommand = true,
    disable_version_flag = true,
    arg = Arg::new("version")
        .short('v')
        .long("version")
        .action(ArgAction::Version)
        .help("Print the version"),
)]
struct Args {
    #[command(subcommand)]
    command: Option<Sub>,
    /// Files to play or links to open
    #[arg(value_name = "FILE_OR_LINK", value_hint = ValueHint::AnyPath)]
    items: Vec<String>,
    /// Print the answer as JSON, for a script.
    #[arg(long, global = true)]
    json: bool,
}

/// A command line clap refused, or the help or version it was asked for, still to be printed.
pub struct Refusal(clap::Error);

impl Refusal {
    /// Prints it, help to stdout and a mistake to stderr, and gives the code to exit with.
    pub fn exit(self) -> ExitCode {
        self.0.print().ok();
        ExitCode::from(self.0.exit_code() as u8)
    }
}

#[derive(Subcommand)]
enum Sub {
    /// Resume playback, or play files, links or the best hit of a search now.
    Play {
        /// Files, or links of the current provider, to play now; the queue carries on after.
        #[arg(value_hint = ValueHint::AnyPath, value_parser = queued)]
        items: Vec<Item>,
        #[command(flatten)]
        searched: Searched,
    },
    /// Pause playback.
    Pause,
    /// Pause playback, or resume it when paused.
    #[command(alias = "play-pause")]
    Toggle,
    /// Skip to the next track.
    Next,
    /// Go back to the previous track.
    #[command(alias = "previous")]
    Prev,
    /// Jump to a point of the current track, in percent.
    Seek {
        #[arg(value_parser = clap::value_parser!(u8).range(0..=100))]
        percent: u8,
    },
    /// Print the current track as JSON.
    Current,
    /// Print the volume, or set it: `50`, `+5`, `-5`.
    Volume {
        /// A level from 0 to 100, or a step from the current one.
        #[arg(allow_negative_numbers = true, value_parser = level)]
        level: Option<Level>,
    },
    /// Print shuffle, or switch it.
    Shuffle { switch: Option<SwitchArg> },
    /// Print repeat, or set it.
    Repeat { repeat: Option<RepeatArg> },
    /// Queue files or links after the tracks queued by hand.
    Add {
        /// Files, or links of the current provider.
        #[arg(
            required_unless_present = "search",
            value_hint = ValueHint::AnyPath,
            value_parser = queued
        )]
        items: Vec<Item>,
        #[command(flatten)]
        searched: Searched,
    },
    /// Queue files or links right after the current track.
    PlayNext {
        /// Files, or links of the current provider.
        #[arg(
            required_unless_present = "search",
            value_hint = ValueHint::AnyPath,
            value_parser = queued
        )]
        items: Vec<Item>,
        #[command(flatten)]
        searched: Searched,
    },
    /// Search the current provider. Each hit ends in the link `add` and `play-next` take.
    Search {
        /// Words to look for; quote them when they hold characters the shell reads, such as
        /// `'`, `?` or `&`.
        #[arg(required = true)]
        query: Vec<String>,
        /// Only hits of this kind.
        #[arg(long = "type", short = 't', value_name = "TYPE")]
        kind: Option<KindArg>,
        /// How many hits to print.
        #[arg(long, short = 'n', default_value_t = 10)]
        limit: u16,
    },
    /// List the providers, whether each has an account and which one is current.
    Providers,
    /// Print the current provider, or switch to one that has an account.
    Provider {
        /// A provider as `sonora providers` lists it, such as `spotify`.
        slug: Option<String>,
    },
    /// Print the completion script for a shell.
    Completions { shell: Shell },
}

/// A search to take the best hit of, in place of files or links.
#[derive(clap::Args)]
struct Searched {
    /// Take the best hit of a search instead. The words follow; quote them when they hold
    /// characters the shell reads, such as `'`, `?` or `&`.
    #[arg(
        short = 's',
        long = "search",
        value_name = "QUERY",
        num_args = 1..,
        conflicts_with = "items"
    )]
    search: Option<Vec<String>>,
    /// The kind of hit `--search` takes.
    #[arg(
        short = 't',
        long = "type",
        value_name = "TYPE",
        default_value = "song",
        requires = "search"
    )]
    kind: KindArg,
}

impl Searched {
    /// The items to play or queue: the search when one was given, else the files and links.
    fn or(self, items: Vec<Item>) -> Vec<Item> {
        match self.search {
            Some(words) => vec![Item::Search {
                text: words.join(" "),
                kind: self.kind.into(),
            }],
            None => items,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum SwitchArg {
    On,
    Off,
    Toggle,
}

#[derive(Clone, Copy, ValueEnum)]
enum KindArg {
    Song,
    Album,
    Artist,
    Playlist,
}

#[derive(Clone, Copy, ValueEnum)]
enum RepeatArg {
    Off,
    All,
    One,
}

/// Reads the command line, `args` including the program name, with `version` for `--version`.
/// A mistake in a subcommand is refused. Any other, such as a flag a launcher added that Sonora
/// does not know, leaves the flags out and keeps the rest as things to open, so Sonora still
/// starts.
pub fn parse(args: Vec<OsString>, version: &'static str) -> Result<CommandType, Refusal> {
    let mut command = Args::command().version(version);
    let parsed = command
        .try_get_matches_from_mut(&args)
        .and_then(|matches| Args::from_arg_matches(&matches));
    let parsed = match parsed {
        Ok(parsed) => parsed,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) || names_subcommand(&command, &args) =>
        {
            return Err(Refusal(error));
        }
        Err(_) => Args {
            command: None,
            json: false,
            items: args
                .iter()
                .skip(1)
                .filter_map(|arg| arg.to_str())
                .filter(|arg| !arg.starts_with('-'))
                .map(str::to_owned)
                .collect(),
        },
    };
    into_command(parsed, &mut command).map_err(Refusal)
}

/// Writes the completion script for `shell` to stdout. It is built whole first, since clap
/// panics when stdout closes under it, as it does when piped into `head`.
pub fn completions(shell: Shell) {
    // the version only has to be there for `-v` to build, a script never shows it
    let mut command = Args::command().version(env!("CARGO_PKG_VERSION"));
    let mut script = Vec::new();
    clap_complete::generate(shell, &mut command, "sonora", &mut script);
    io::stdout().write_all(&script).ok();
}

/// `line` is the command the arguments were read against, for the usage a refusal prints.
fn into_command(args: Args, line: &mut Command) -> Result<CommandType, clap::Error> {
    let Some(command) = args.command else {
        if let Some(typo) = args.items.iter().find(|item| is_typo(item)) {
            return Err(line.error(
                ErrorKind::InvalidSubcommand,
                format!("unrecognized subcommand '{typo}'"),
            ));
        }
        let items = args.items.into_iter().map(item::anchored).collect();
        return Ok(CommandType::Launch(items));
    };
    let format = match args.json {
        true => Format::Json,
        false => Format::Text,
    };
    let play = Request::Playback;
    let request = match command {
        Sub::Play { items, searched } => match searched.or(items) {
            items if items.is_empty() => play(PlaybackCommand::Play),
            items => Request::Queue(QueueCommand::Play(items)),
        },
        Sub::Pause => play(PlaybackCommand::Pause),
        Sub::Toggle => play(PlaybackCommand::Toggle),
        Sub::Next => play(PlaybackCommand::Next),
        Sub::Prev => play(PlaybackCommand::Previous),
        Sub::Seek { percent } => play(PlaybackCommand::Seek(percent)),
        Sub::Current => play(PlaybackCommand::CurrentTrack),
        Sub::Volume { level } => play(PlaybackCommand::Volume(level)),
        Sub::Shuffle { switch } => play(PlaybackCommand::Shuffle(switch.map(Switch::from))),
        Sub::Repeat { repeat } => play(PlaybackCommand::Repeat(repeat.map(Repeat::from))),
        Sub::Add { items, searched } => Request::Queue(QueueCommand::Add(searched.or(items))),
        Sub::PlayNext { items, searched } => {
            Request::Queue(QueueCommand::PlayNext(searched.or(items)))
        }
        Sub::Search { query, kind, limit } => Request::Search(SearchQuery {
            text: query.join(" "),
            kind: kind.map(Kind::from),
            limit,
        }),
        Sub::Providers => Request::Provider(ProviderCommand::List),
        Sub::Provider { slug } => Request::Provider(ProviderCommand::Current(slug)),
        Sub::Completions { shell } => return Ok(CommandType::Local(Local::Completions(shell))),
    };
    Ok(CommandType::Remote(request, format))
}

impl From<SwitchArg> for Switch {
    fn from(switch: SwitchArg) -> Self {
        match switch {
            SwitchArg::On => Self::On,
            SwitchArg::Off => Self::Off,
            SwitchArg::Toggle => Self::Toggle,
        }
    }
}

impl From<KindArg> for Kind {
    fn from(kind: KindArg) -> Self {
        match kind {
            KindArg::Song => Self::Song,
            KindArg::Album => Self::Album,
            KindArg::Artist => Self::Artist,
            KindArg::Playlist => Self::Playlist,
        }
    }
}

impl From<RepeatArg> for Repeat {
    fn from(repeat: RepeatArg) -> Self {
        match repeat {
            RepeatArg::Off => Self::Off,
            RepeatArg::All => Self::All,
            RepeatArg::One => Self::One,
        }
    }
}

/// Whether the first word after the program names a subcommand, so a mistake past it is the
/// user's to see rather than a launcher's to ignore.
fn names_subcommand(command: &Command, args: &[OsString]) -> bool {
    args.iter()
        .skip(1)
        .filter_map(|arg| arg.to_str())
        .find(|arg| !arg.starts_with('-'))
        .is_some_and(|word| command.find_subcommand(word).is_some())
}

/// A volume as typed: a signed step from the current level, or a level to land on.
fn level(text: &str) -> Result<Level, String> {
    let signed = text.starts_with(['+', '-']);
    let value: i16 = text
        .parse()
        .map_err(|_| format!("'{text}' is not a number"))?;
    match signed {
        true => i8::try_from(value)
            .map(Level::By)
            .map_err(|_| format!("a step of {value} is too large")),
        false => u8::try_from(value)
            .ok()
            .filter(|value| *value <= 100)
            .map(Level::To)
            .ok_or_else(|| format!("{value} is not between 0 and 100")),
    }
}

/// An item to queue as typed: an address goes as it is, a path has to name a file here.
fn queued(text: &str) -> Result<Item, String> {
    if item::is_address(text) {
        return Ok(Item::Link(text.to_owned()));
    }
    let path = path::absolute(item::local_path(text)).map_err(|error| error.to_string())?;
    match path.exists() {
        true => Ok(Item::Path(path)),
        false => Err(format!("no file at '{}'", path.display())),
    }
}

/// Whether an item meant as a file or link reads as a mistyped command instead: a bare word
/// that names no file here and is no address.
fn is_typo(item: &str) -> bool {
    !item.contains(['/', '\\', ':', '.']) && !Path::new(item).exists()
}
