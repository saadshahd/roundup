//! I1–I7: every tool `rup mcp` offers is also a command, `rup <noun> <verb>`, built at run time from
//! the same list (`mcp::tools`): arguments and flags from its input schema, help from its description.

use std::io::Read;
use std::process::ExitCode;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use clap::error::{ContextKind, ErrorKind};
use clap::{Arg, ArgAction, ArgMatches, Command};
use contracts::{Actor, ActorKind, IdentifyParams};
use rmcp::model::JsonObject;
use rpc::RpcError;
use serde_json::{Map, Value};

use crate::mcp::{self, CALL_TIMEOUT, Offered};

/// What a field of a tool's input schema holds.
#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Text(Vec<String>),
    Int,
    Number,
    Bool,
    Array(Box<Kind>),
    /// An object, or anything the schema leaves open: the value is JSON.
    Json,
}

#[derive(Clone, Debug)]
struct Field {
    /// The schema's name, which the Daemon's params use.
    name: String,
    kind: Kind,
    required: bool,
    /// A positional argument; any other field is `--<name>` in kebab-case.
    positional: bool,
    doc: String,
}

impl Field {
    fn flag(&self) -> String {
        self.name.replace('_', "-")
    }
}

/// One command of the tree.
struct Spec {
    noun: String,
    verb: String,
    method: &'static str,
    description: String,
    fields: Vec<Field>,
    door_only: bool,
    waits_for_user: bool,
    /// I9: a top-level command with no verb (`rup events`) that prints what the Daemon pushes.
    stream: bool,
}

fn kebab(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            out.push('-');
            out.push(ch.to_ascii_lowercase());
        } else if ch == '_' {
            out.push('-');
        } else {
            out.push(ch);
        }
    }
    out
}

fn resolve<'a>(node: &'a Value, root: &'a Value) -> &'a Value {
    let target = node
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| reference.strip_prefix('#'))
        .and_then(|pointer| root.pointer(pointer))
        .or_else(|| {
            node.get("allOf")
                .and_then(Value::as_array)
                .filter(|all| all.len() == 1)
                .map(|all| &all[0])
        });
    target.map_or(node, |target| resolve(target, root))
}

/// The kind of a schema node, and whether `null` is one of its values.
fn kind_of(node: &Value, root: &Value) -> (Kind, bool) {
    let node = resolve(node, root);
    let mut nullable = false;
    for key in ["anyOf", "oneOf"] {
        let Some(branches) = node.get(key).and_then(Value::as_array) else {
            continue;
        };
        let (nulls, rest): (Vec<_>, Vec<_>) = branches
            .iter()
            .partition(|branch| branch.get("type").and_then(Value::as_str) == Some("null"));
        nullable |= !nulls.is_empty();
        if let [only] = rest[..] {
            return (kind_of(only, root).0, nullable);
        }
        let choices: Option<Vec<String>> = rest
            .iter()
            .map(|branch| {
                branch
                    .get("const")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .collect();
        return match choices {
            Some(choices) if !choices.is_empty() => (Kind::Text(choices), nullable),
            _ => (Kind::Json, nullable),
        };
    }
    if let Some(choices) = node.get("enum").and_then(Value::as_array) {
        let names: Vec<String> = choices
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        nullable |= choices.iter().any(Value::is_null);
        if !names.is_empty() {
            return (Kind::Text(names), nullable);
        }
    }
    let types: Vec<&str> = match node.get("type") {
        Some(Value::String(one)) => vec![one],
        Some(Value::Array(many)) => many.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    nullable |= types.contains(&"null");
    let types: Vec<&str> = types.into_iter().filter(|t| *t != "null").collect();
    let kind = match types[..] {
        ["string"] => Kind::Text(Vec::new()),
        ["integer"] => Kind::Int,
        ["number"] => Kind::Number,
        ["boolean"] => Kind::Bool,
        ["array"] => Kind::Array(Box::new(
            node.get("items")
                .map_or(Kind::Json, |items| kind_of(items, root).0),
        )),
        _ => Kind::Json,
    };
    (kind, nullable)
}

/// I1: a required string or integer is a positional argument in schema order; every other field is a flag.
fn fields_of(schema: &JsonObject) -> Vec<Field> {
    let root = Value::Object(schema.clone());
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let Some(Value::Object(properties)) = schema.get("properties") else {
        return Vec::new();
    };
    let mut fields: Vec<Field> = properties
        .iter()
        .map(|(name, node)| {
            let (kind, nullable) = kind_of(node, &root);
            let required = required.contains(&name.as_str()) && !nullable;
            let positional = required && matches!(kind, Kind::Text(_) | Kind::Int | Kind::Number);
            let doc = resolve(node, &root)
                .get("description")
                .or_else(|| node.get("description"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            Field {
                name: name.clone(),
                kind,
                required,
                positional,
                doc,
            }
        })
        .collect();
    // Positionals first, in the order `required` lists them.
    fields.sort_by_key(|field| {
        if field.positional {
            required.iter().position(|name| *name == field.name)
        } else {
            None
        }
        .map_or((1, 0), |at| (0, at))
    });
    fields
}

/// I6: `terminal write <id> "text"` types text and Enter; `--base64` sends raw bytes.
fn terminal_write_fields(mut fields: Vec<Field>) -> Vec<Field> {
    fields.retain(|field| field.name != "data");
    let at = fields.iter().take_while(|field| field.positional).count();
    let extra = |name: &str, kind, positional, doc: &str| Field {
        name: name.into(),
        kind,
        required: false,
        positional,
        doc: doc.into(),
    };
    fields.splice(
        at..at,
        [
            extra(
                "text",
                Kind::Text(Vec::new()),
                true,
                "The text to type; `-` reads it from stdin. Enter is pressed after it.",
            ),
            extra(
                "no-enter",
                Kind::Bool,
                false,
                "Leave the Enter out after the text.",
            ),
            extra(
                "base64",
                Kind::Text(Vec::new()),
                false,
                "Send these base64 bytes as they are, instead of text.",
            ),
        ],
    );
    fields
}

fn first_sentence(description: &str) -> &str {
    let end = description.find(". ").unwrap_or(description.len());
    description[..end].trim_end_matches('.')
}

fn specs(door: bool) -> Vec<Spec> {
    mcp::commands(door)
        .into_iter()
        .map(|offer| {
            let (noun, verb) = offer.method.split_once('.').unwrap_or((offer.method, ""));
            let verb = if offer.stream { "" } else { verb };
            let mut fields = fields_of(&offer.tool.input_schema);
            if offer.stream {
                fields.push(Field {
                    name: "follow".into(),
                    kind: Kind::Bool,
                    required: false,
                    positional: false,
                    doc: "Keep streaming Events as they are saved, after the saved ones.".into(),
                });
            }
            if offer.method == "terminal.write" {
                fields = terminal_write_fields(fields);
            }
            if offer.method == "agent.wake" {
                // B45: `rup agent wake <id>... [--any]` names the Agents without a flag.
                for field in fields.iter_mut().filter(|field| field.name == "ids") {
                    field.positional = true;
                    field.required = true;
                }
                fields.sort_by_key(|field| !field.positional);
            }
            if offer.method == "terminal.read" {
                // `rup terminal read 3 --after 120`: a read without it returns what is new since your last read (X16).
                for field in fields.iter_mut().filter(|field| field.name == "after") {
                    field.positional = false;
                    field.required = false;
                }
                fields.sort_by_key(|field| !field.positional);
            }
            Spec {
                noun: noun.to_owned(),
                verb: kebab(verb),
                method: offer.method,
                description: if offer.method == "terminal.write" {
                    "Type text into a Terminal you or your Reach own, then press Enter. `--no-enter` leaves Enter out; `--base64 <data>` sends raw bytes."
                        .to_owned()
                } else {
                    offer.tool.description.as_deref().unwrap_or_default().to_owned()
                },
                fields,
                door_only: offer.door_only,
                waits_for_user: offer.waits_for_user,
                stream: offer.stream,
            }
        })
        .collect()
}

fn placeholder(field: &Field) -> String {
    match &field.kind {
        Kind::Text(choices) => choices
            .first()
            .cloned()
            .unwrap_or_else(|| field.name.clone()),
        _ => field.name.clone(),
    }
}

/// What the user types before the arguments: `rup pad read`, or `rup events` for a stream.
fn invocation(spec: &Spec) -> String {
    if spec.stream {
        format!("rup {}", spec.noun)
    } else {
        format!("rup {} {}", spec.noun, spec.verb)
    }
}

/// The usage line: `rup pad read <name>`.
fn usage(spec: &Spec) -> String {
    let mut line = invocation(spec);
    for field in &spec.fields {
        let flag = field.flag();
        let part = match (field.positional, field.required, &field.kind) {
            (true, true, Kind::Array(_)) => format!(" <{}>...", flag.trim_end_matches('s')),
            (true, true, _) => format!(" <{flag}>"),
            (true, false, _) => format!(" [<{flag}>]"),
            (false, true, Kind::Bool) => format!(" --{flag}"),
            (false, false, Kind::Bool) => format!(" [--{flag}]"),
            (false, true, Kind::Array(_)) => format!(" --{flag} <{flag}>..."),
            (false, false, Kind::Array(_)) => format!(" [--{flag} <{flag}>]..."),
            (false, true, _) => format!(" --{flag} <{}>", placeholder(field)),
            (false, false, _) => format!(" [--{flag} <{}>]", placeholder(field)),
        };
        line.push_str(&part);
    }
    line
}

fn example(spec: &Spec) -> String {
    if spec.stream {
        return format!("{} --after 12 --follow --json", invocation(spec));
    }
    let mut line = invocation(spec);
    for field in spec.fields.iter().filter(|f| f.positional) {
        let sample = match (&field.kind, field.name.as_str()) {
            (Kind::Array(_), "ids") => "3 4 --any".to_owned(),
            (Kind::Int | Kind::Number, _) => "1".to_owned(),
            (_, "id") => "3".to_owned(),
            (_, "name") => "notes".to_owned(),
            (Kind::Text(choices), _) if !choices.is_empty() => choices[0].clone(),
            _ => "\"...\"".to_owned(),
        };
        line.push(' ');
        line.push_str(&sample);
    }
    line
}

fn type_name(kind: &Kind) -> &'static str {
    match kind {
        Kind::Text(_) => "text",
        Kind::Int => "integer",
        Kind::Number => "number",
        Kind::Bool => "flag",
        Kind::Array(_) => "list, repeat the flag",
        Kind::Json => "JSON",
    }
}

fn arg_for(field: &Field) -> Arg {
    let flag = field.flag();
    let help = match (&field.kind, field.positional) {
        (Kind::Text(_) | Kind::Array(_), true) => field.doc.clone(),
        _ => format!("{} ({})", field.doc, type_name(&field.kind))
            .trim()
            .to_owned(),
    };
    let arg = Arg::new(field.name.clone())
        .value_name(flag.clone())
        .help(help)
        .required(field.required);
    let item = match &field.kind {
        Kind::Array(item) => item,
        kind => kind,
    };
    let arg = match item {
        Kind::Int => arg
            .value_parser(clap::value_parser!(i64))
            .allow_negative_numbers(true),
        Kind::Number => arg
            .value_parser(clap::value_parser!(f64))
            .allow_negative_numbers(true),
        Kind::Text(choices) if !choices.is_empty() => {
            arg.value_parser(clap::builder::PossibleValuesParser::new(choices.clone()))
        }
        _ => arg,
    };
    match (&field.kind, field.positional) {
        (Kind::Array(_), true) => arg
            .num_args(1..)
            .value_name(flag.trim_end_matches('s').to_owned()),
        (_, true) => arg,
        (Kind::Bool, false) => arg.long(flag).action(ArgAction::SetTrue).required(false),
        // A repeated text flag is often an argv (`--command sh --command -c`), so its values may start with `-`.
        (Kind::Array(item), false) => arg
            .long(flag)
            .action(ArgAction::Append)
            .allow_hyphen_values(matches!(**item, Kind::Text(_))),
        (_, false) => arg.long(flag),
    }
}

fn verb_command(spec: &Spec) -> Command {
    let mut description = spec.description.clone();
    if spec.door_only {
        description.push_str(" Only a Door has this command.");
    }
    let mut command = Command::new(spec.verb.clone())
        .about(first_sentence(&spec.description).to_owned())
        .long_about(description)
        .after_help(format!("Example:\n  {}", example(spec)));
    for field in &spec.fields {
        command = command.arg(arg_for(field));
    }
    command
}

fn tree(specs: &[Spec]) -> Command {
    let mut root = Command::new("rup")
        .bin_name("rup")
        .disable_help_subcommand(true)
        .disable_version_flag(true)
        .subcommand_required(true)
        .arg(
            Arg::new("agent")
                .long("agent")
                .value_name("id")
                .global(true)
                .help("Act as this Agent; the default is the id in ROUNDUP_AGENT"),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Print the method's result as JSON on stdout"),
        );
    let mut nouns: Vec<&str> = Vec::new();
    for spec in specs.iter().filter(|spec| !spec.stream) {
        if !nouns.contains(&spec.noun.as_str()) {
            nouns.push(&spec.noun);
        }
    }
    for spec in specs.iter().filter(|spec| spec.stream) {
        root = root.subcommand(verb_command(spec).name(spec.noun.clone()));
    }
    for noun in nouns {
        let mut group = Command::new(noun.to_owned())
            .about(format!("Commands on {noun}s"))
            .subcommand_required(true);
        for spec in specs.iter().filter(|spec| spec.noun == noun) {
            group = group.subcommand(verb_command(spec));
        }
        root = root.subcommand(group);
    }
    root
}

/// Exit codes of I4.
const DONE: u8 = 0;
const FAILED: u8 = 1;
const BAD_USAGE: u8 = 2;

const EXAMPLES: &str = "  rup pad read notes
  rup terminal read 3 --after 120
  rup todo create \"Fix login\"
  rup pad write notes - < notes.md";

const OTHER: &[(&str, &str)] = &[
    ("rup help [<command>]", "Print this help, or a command's"),
    ("rup ping", "Check the Daemon answers"),
    (
        "rup mcp <agent-id>",
        "Serve these commands as MCP tools over stdio",
    ),
    (
        "rup signal <agent-id>",
        "Hook: hand the Daemon a Signal from stdin",
    ),
    ("rup context <agent-id>", "Hook: print the Agent's Context"),
    (
        "rup permission <agent-id>",
        "Hook: ask the Daemon about a permission request",
    ),
];

/// CL4: what `rup help` prints.
const WHO_MAY_RUN: &str = "\
rup drives rupd, the roundup Daemon, from a shell or an MCP client.

usage: rup [--as ext:<name>] [--json] <command>

commands, and who may run each (User: a shell outside every Agent; Ext: --as ext:<name>):
  ping        show which Daemon answered
              User: yes   Ext: yes
  mcp         serve the roundup tools over MCP on stdio (rup mcp <agent-id> for an Agent)
              User: yes   Ext: yes
  signal      an Agent's hook (rup signal <agent-id>)
              User: no    Ext: no
  context     an Agent's hook (rup context <agent-id>)
              User: no    Ext: no
  permission  an Agent's hook (rup permission <agent-id>)
              User: no    Ext: no
  help        print this text

options, before the command:
  --as ext:<name>  act as the Ext Actor ext:<name> (1 to 64 of a-z 0-9 - _), so calls are Touches under that name
  --json           with ping, print {actor, socket, result}, who rup acted as included

RUPD_SOCKET names the Daemon's socket; unset, rup uses ~/.roundup/rupd.sock.
`rup ping` shows which Daemon answered.
";

/// I2: what roundup and `rup` are, examples, the commands by noun, the exit codes.
pub fn help_text() -> String {
    let specs = specs(true);
    let mut text = String::from(
        "rup: the command line of roundup\n\n\
         roundup runs coding Agents in a Project and shows the user their Status, Todos, Pads, Messages and Terminals. \
         rup is how an Agent reads and changes all of that from its shell: every command acts as you, the Agent whose id is in ROUNDUP_AGENT.\n\n\
         Examples:\n",
    );
    text.push_str(EXAMPLES);
    text.push_str("\n\nCommands:\n");
    let mut nouns: Vec<&str> = Vec::new();
    for spec in specs.iter().filter(|spec| !spec.stream) {
        if !nouns.contains(&spec.noun.as_str()) {
            nouns.push(&spec.noun);
        }
    }
    for noun in nouns {
        text.push_str(&format!("\n  {noun}\n"));
        for spec in specs.iter().filter(|spec| spec.noun == noun) {
            let door = if spec.door_only { " [Door only]" } else { "" };
            text.push_str(&format!(
                "    {}\n        {}.{door}\n",
                usage(spec),
                first_sentence(&spec.description)
            ));
        }
    }
    text.push_str("\n  streams\n");
    for spec in specs.iter().filter(|spec| spec.stream) {
        text.push_str(&format!(
            "    {}\n        {}.\n        Example: {}\n",
            usage(spec),
            first_sentence(&spec.description),
            example(spec)
        ));
    }
    text.push_str("\n  other\n");
    for (usage, what) in OTHER {
        text.push_str(&format!("    {usage}\n        {what}.\n"));
    }
    text.push_str(
        "\nOptions of every command:\n    \
         --agent <id>   Act as this Agent instead of ROUNDUP_AGENT\n    \
         --json         Print the result as JSON on stdout\n    \
         -h, --help     Print a command's arguments, flags and an example\n\n\
         A text argument takes `-` to read the whole of stdin, so a long body is never quoted.\n\
         `rup help <noun>` lists a noun's commands; `rup help <noun> <verb>` describes one.\n\n\
         Exit codes:\n    \
         0  done\n    \
         1  failed: the Daemon refused, did not answer within 10 s, or was not there\n    \
         2  bad usage: an unknown command, a missing or invalid argument, no Agent id, a Door-only command run by an Agent\n",
    );
    text.push('\n');
    text.push_str(WHO_MAY_RUN);
    text
}

pub async fn run(args: Vec<String>) -> ExitCode {
    let tree_specs = specs(true);
    let mut root = tree(&tree_specs);
    if matches!(args.first().map(String::as_str), Some("help")) {
        return help(&mut root, &args[1..]);
    }
    let matches = match root
        .try_get_matches_from_mut(std::iter::once("rup".to_owned()).chain(args.clone()))
    {
        Ok(matches) => matches,
        Err(err) => return usage_error(&root, &args, &err),
    };
    let Some((noun, group)) = matches.subcommand() else {
        return bad_usage("no command");
    };
    if let Some(spec) = tree_specs
        .iter()
        .find(|spec| spec.stream && spec.noun == noun)
    {
        return events(spec, group).await;
    }
    let Some((verb, leaf)) = group.subcommand() else {
        return bad_usage("no command");
    };
    let spec = tree_specs
        .iter()
        .find(|spec| spec.noun == noun && spec.verb == verb)
        .expect("clap matched a command of the tree");
    call(spec, leaf, &matches).await
}

fn bad_usage(why: &str) -> ExitCode {
    eprintln!("rup: {why}\nrun \"rup help\" to see the commands");
    ExitCode::from(BAD_USAGE)
}

/// `rup help`, `rup help <noun>`, `rup help <noun> <verb>`, and the same for `-h` and `--help`.
fn help(root: &mut Command, path: &[String]) -> ExitCode {
    root.build();
    let mut at: &mut Command = root;
    for step in path {
        let name = step.clone();
        match at.find_subcommand_mut(&name) {
            Some(next) => at = next,
            None => {
                eprintln!("rup: unknown command {name:?}\nrun \"rup help\" to see the commands");
                return ExitCode::from(BAD_USAGE);
            }
        }
    }
    if path.is_empty() {
        print!("{}", help_text());
    } else {
        print!("{}", at.render_long_help());
    }
    ExitCode::from(DONE)
}

fn usage_error(root: &Command, args: &[String], err: &clap::Error) -> ExitCode {
    match err.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            if args.iter().all(|arg| arg.starts_with('-')) {
                print!("{}", help_text());
            } else {
                print!("{}", err.render());
            }
            ExitCode::from(DONE)
        }
        ErrorKind::MissingSubcommand if args.is_empty() => {
            print!("{}", help_text());
            ExitCode::from(DONE)
        }
        ErrorKind::InvalidSubcommand => {
            let unknown = err
                .get(ContextKind::InvalidSubcommand)
                .map(ToString::to_string)
                .unwrap_or_default();
            let prefix = args
                .first()
                .filter(|noun| **noun != unknown && root.find_subcommand(noun).is_some())
                .map_or(String::new(), |noun| format!("{noun} "));
            eprintln!("rup: unknown command {:?}", format!("{prefix}{unknown}"));
            if let Some(close) = err.get(ContextKind::SuggestedSubcommand) {
                let close = close.to_string();
                let closest = close
                    .split(", ")
                    .min_by_key(|candidate| distance(&unknown, candidate))
                    .unwrap_or(&close);
                eprintln!("did you mean {:?}?", format!("{prefix}{closest}"));
            }
            eprintln!("run \"rup help\" to see the commands");
            if prefix.is_empty() {
                // CL4: an unknown command is followed by who may run what.
                eprint!("\n{WHO_MAY_RUN}");
            }
            ExitCode::from(BAD_USAGE)
        }
        _ => {
            let message = err.render().to_string();
            let message = message.strip_prefix("error: ").unwrap_or(&message);
            eprintln!("rup: {}", message.trim_end());
            ExitCode::from(BAD_USAGE)
        }
    }
}

/// Edit distance between two short names, to pick the closest of clap's suggestions.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.chars().enumerate() {
        let mut prev = std::mem::replace(&mut row[0], i + 1);
        for (j, y) in b.iter().enumerate() {
            let next = (prev + usize::from(x != *y))
                .min(row[j] + 1)
                .min(row[j + 1] + 1);
            prev = std::mem::replace(&mut row[j + 1], next);
        }
    }
    row[b.len()]
}

/// A usage failure while turning the parsed command into a call.
struct Usage(String);

struct Input {
    read: bool,
}

impl Input {
    /// I6: `-` is the whole of stdin, as UTF-8.
    fn stdin(&mut self) -> Result<String, Usage> {
        if std::mem::replace(&mut self.read, true) {
            return Err(Usage(
                "stdin is read once; only one argument may be `-`".into(),
            ));
        }
        let mut bytes = Vec::new();
        std::io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|err| Usage(format!("cannot read stdin: {err}")))?;
        String::from_utf8(bytes).map_err(|_| Usage("stdin is not UTF-8 text".into()))
    }

    fn text(&mut self, raw: &str) -> Result<String, Usage> {
        if raw == "-" {
            self.stdin()
        } else {
            Ok(raw.to_owned())
        }
    }
}

fn value_of(
    kind: &Kind,
    name: &str,
    matches: &ArgMatches,
    input: &mut Input,
) -> Result<Option<Value>, Usage> {
    let json = |raw: &str| {
        serde_json::from_str::<Value>(raw)
            .map_err(|err| Usage(format!("--{name} is not JSON: {err}")))
    };
    Ok(match kind {
        Kind::Text(_) => match matches.get_one::<String>(name) {
            Some(raw) => Some(Value::String(input.text(raw)?)),
            None => None,
        },
        Kind::Int => matches.get_one::<i64>(name).map(|n| Value::from(*n)),
        Kind::Number => matches.get_one::<f64>(name).map(|n| Value::from(*n)),
        Kind::Bool => matches.get_flag(name).then_some(Value::Bool(true)),
        Kind::Json => match matches.get_one::<String>(name) {
            Some(raw) => Some(json(&input.text(raw)?)?),
            None => None,
        },
        Kind::Array(item) => {
            let values: Vec<Value> = match &**item {
                Kind::Int => matches
                    .get_many::<i64>(name)
                    .map(|many| many.map(|n| Value::from(*n)).collect())
                    .unwrap_or_default(),
                Kind::Number => matches
                    .get_many::<f64>(name)
                    .map(|many| many.map(|n| Value::from(*n)).collect())
                    .unwrap_or_default(),
                Kind::Text(_) => matches
                    .get_many::<String>(name)
                    .map(|many| many.map(|s| Value::String(s.clone())).collect())
                    .unwrap_or_default(),
                _ => matches
                    .get_many::<String>(name)
                    .map(|many| many.map(|raw| json(raw)).collect::<Result<_, _>>())
                    .transpose()?
                    .unwrap_or_default(),
            };
            (!values.is_empty()).then_some(Value::Array(values))
        }
    })
}

fn arguments(spec: &Spec, leaf: &ArgMatches) -> Result<JsonObject, Usage> {
    let mut input = Input { read: false };
    let mut out = Map::new();
    if spec.method == "terminal.write" {
        let enter = !leaf.get_flag("no-enter");
        let data = match (
            leaf.get_one::<String>("base64"),
            leaf.get_one::<String>("text"),
        ) {
            (Some(_), Some(_)) => {
                return Err(Usage("pass the text or --base64, not both".into()));
            }
            (Some(_), None) if !enter => {
                return Err(Usage(
                    "--base64 sends raw bytes, so it conflicts with --no-enter".into(),
                ));
            }
            (Some(raw), None) => raw.clone(),
            (None, Some(raw)) => {
                let mut bytes = input.text(raw)?.into_bytes();
                if enter {
                    bytes.push(b'\r');
                }
                STANDARD.encode(bytes)
            }
            (None, None) => return Err(Usage("give the text to type, or --base64 <data>".into())),
        };
        out.insert("data".into(), Value::String(data));
    }
    for field in &spec.fields {
        if spec.method == "terminal.write"
            && matches!(field.name.as_str(), "text" | "no-enter" | "base64")
        {
            continue;
        }
        if let Some(value) = value_of(&field.kind, &field.name, leaf, &mut input)? {
            out.insert(field.name.clone(), value);
        }
    }
    Ok(out)
}

/// Why a call did not return a result.
enum Failure {
    /// A Door-only command run by an Agent: bad usage, not a failure of the Daemon.
    Usage(String),
    Rpc(RpcError),
    Gone(String),
}

fn code_name(code: i64) -> &'static str {
    match code {
        rpc::code::PARSE_ERROR => "PARSE_ERROR",
        rpc::code::METHOD_NOT_FOUND => "METHOD_NOT_FOUND",
        rpc::code::INVALID_PARAMS => "INVALID_PARAMS",
        rpc::code::NOT_FOUND => "NOT_FOUND",
        rpc::code::FORBIDDEN => "FORBIDDEN",
        rpc::code::CONFLICT => "CONFLICT",
        rpc::code::UNKNOWN_OUTCOME => "UNKNOWN_OUTCOME",
        rpc::code::BUSY => "BUSY",
        rpc::code::NOT_ACCEPTED => "NOT_ACCEPTED",
        rpc::code::NOT_RUNNING => "NOT_RUNNING",
        rpc::code::NOT_ACKED => "NOT_ACKED",
        rpc::code::EVENTS_EXPIRED => "EVENTS_EXPIRED",
        rpc::code::INBOX_UNAVAILABLE => "INBOX_UNAVAILABLE",
        _ => "INTERNAL",
    }
}

/// I3: one line on what went wrong, one on what to do next.
fn fail(spec: &Spec, failure: &Failure) -> ExitCode {
    match failure {
        Failure::Usage(why) => return bad_usage(why),
        Failure::Gone(why) => eprintln!("rup: {why}"),
        Failure::Rpc(err) => {
            eprintln!("rup: {} ({})", err.message, code_name(err.code));
            eprintln!(
                "{}",
                match err.code {
                    rpc::code::FORBIDDEN => "this Agent may not act on that node".to_owned(),
                    rpc::code::EVENTS_EXPIRED => return ExitCode::from(FAILED),
                    rpc::code::NOT_FOUND => format!("list them with `rup {} list`", spec.noun),
                    rpc::code::UNKNOWN_OUTCOME => format!(
                        "the call may or may not have been applied; read before you retry: `rup help {} list`",
                        spec.noun
                    ),
                    _ => format!(
                        "see its arguments with `rup help {} {}`",
                        spec.noun, spec.verb
                    ),
                }
            );
        }
    }
    ExitCode::from(FAILED)
}

fn gone(socket: &str, why: impl std::fmt::Display) -> Failure {
    Failure::Gone(format!(
        "no Daemon answered at {socket}: {why}\nstart roundup, or point RUPD_SOCKET at its socket"
    ))
}

async fn connect(agent: &str) -> Result<rpc::Client, Failure> {
    let socket = rpc::socket_path().map_err(|err| Failure::Gone(err.to_string()))?;
    let shown = socket.display().to_string();
    let handshake = async {
        let client = rpc::Client::connect(&socket)
            .await
            .map_err(|err| gone(&shown, err))?;
        let actor = Actor {
            kind: ActorKind::Agent,
            id: agent.to_owned(),
            parent: None,
        };
        client
            .request("daemon.identify", IdentifyParams { actor })
            .await
            .map_err(Failure::Rpc)?;
        Ok(client)
    };
    tokio::time::timeout(CALL_TIMEOUT, handshake)
        .await
        .map_err(|_| {
            gone(
                &shown,
                format!("no answer within {} s", CALL_TIMEOUT.as_secs()),
            )
        })?
}

async fn invoke(
    spec: &Spec,
    offer: &Offered,
    agent: &str,
    arguments: JsonObject,
) -> Result<(Value, Value), Failure> {
    let client = connect(agent).await?;
    if spec.door_only {
        match mcp::is_door(&client, agent).await {
            Ok(true) => {}
            Ok(false) => {
                return Err(Failure::Usage(format!(
                    "only a Door has \"rup {} {}\"",
                    spec.noun, spec.verb
                )));
            }
            Err(why) => return Err(Failure::Gone(why)),
        }
    }
    let params = offer.params(agent, Some(arguments));
    let call = client.request(spec.method, &params);
    let result = if spec.waits_for_user {
        call.await
    } else {
        tokio::time::timeout(CALL_TIMEOUT, call)
            .await
            .map_err(|_| {
                Failure::Gone(format!(
                    "no answer within {} s; the call may have been applied",
                    CALL_TIMEOUT.as_secs()
                ))
            })?
    };
    result.map(|value| (params, value)).map_err(Failure::Rpc)
}

async fn call(spec: &Spec, leaf: &ArgMatches, matches: &ArgMatches) -> ExitCode {
    let arguments = match arguments(spec, leaf) {
        Ok(arguments) => arguments,
        Err(Usage(why)) => return bad_usage(&why),
    };
    let agent = leaf
        .get_one::<String>("agent")
        .cloned()
        .or_else(|| matches.get_one::<String>("agent").cloned())
        .or_else(|| {
            std::env::var("ROUNDUP_AGENT")
                .ok()
                .filter(|id| !id.is_empty())
        });
    let Some(agent) = agent else {
        return bad_usage("no Agent id: run it in an Agent's shell, or pass --agent <id>");
    };
    let json = leaf.get_flag("json");
    let offer = mcp::tools(true)
        .into_iter()
        .find(|offer| offer.method == spec.method)
        .expect("the tree is built from the tool list");
    match invoke(spec, &offer, &agent, arguments).await {
        Ok((params, result)) => {
            print_result(spec.method, &params, &result, json);
            ExitCode::from(DONE)
        }
        Err(failure) => fail(spec, &failure),
    }
}

fn line(item: &Value) -> String {
    match item {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// I3, I7: the text the MCP tool returns, or the plain forms of reading a Terminal.
fn print_result(method: &str, params: &Value, result: &Value, json: bool) {
    if json {
        println!("{result}");
        return;
    }
    match method {
        "terminal.read" => {
            print!("{}", result["text"].as_str().unwrap_or_default());
            eprintln!("next: {}", result["next"]);
            if result["truncated"] == true {
                eprintln!("truncated");
            }
        }
        "terminal.screen" => {
            for row in result["lines"].as_array().into_iter().flatten() {
                println!("{}", line(row));
            }
            eprintln!("next: {}", result["offset"]);
            if result["truncated"] == true {
                eprintln!("truncated");
            }
        }
        "terminal.search" => {
            for found in result["matches"].as_array().into_iter().flatten() {
                println!("{}: {}", found["offset"], line(&found["text"]));
            }
            eprintln!("next: {}", result["next"]);
            if result["truncated"] == true {
                eprintln!("truncated");
            }
        }
        "agent.wake" => {
            println!("woken: {}", result["id"]);
            if let Some(why) = result["why"].as_str() {
                println!("why: {why}");
            }
        }
        _ => {
            let text = mcp::success_text(method, params, result.clone());
            let plain = result.to_string();
            match result {
                Value::Array(items) if text == plain => {
                    for item in items {
                        println!("{}", line(item));
                    }
                }
                _ => println!("{text}"),
            }
        }
    }
}

/// I9: print the saved Events past `--after` one JSON object per line, and with `--follow` the live ones after them.
/// It adds no timer, poll or retry: a lost connection is exit 1 and the caller decides to reconnect.
async fn events(spec: &Spec, leaf: &ArgMatches) -> ExitCode {
    use std::io::Write;

    let after = match leaf
        .get_one::<i64>("after")
        .copied()
        .map(u64::try_from)
        .transpose()
    {
        Ok(after) => after,
        Err(_) => return bad_usage("--after takes an Event number, 0 or more"),
    };
    let follow = leaf.get_flag("follow");
    let subscribe = async {
        let socket = rpc::socket_path().map_err(|err| Failure::Gone(err.to_string()))?;
        let shown = socket.display().to_string();
        let client = rpc::Client::connect(&socket)
            .await
            .map_err(|err| gone(&shown, err))?;
        let answer = client
            .request(spec.method, contracts::SubscribeParams { after })
            .await
            .map_err(Failure::Rpc)?;
        Ok::<_, Failure>((client, answer))
    };
    let subscribed = tokio::time::timeout(CALL_TIMEOUT, subscribe)
        .await
        .unwrap_or_else(|_| {
            Err(Failure::Gone(format!(
                "no answer within {} s",
                CALL_TIMEOUT.as_secs()
            )))
        });
    let (mut client, answer) = match subscribed {
        Ok(subscribed) => subscribed,
        Err(failure) => return fail(spec, &failure),
    };
    let head = answer["head"].as_u64().unwrap_or_default();
    let mut last = after;
    if !follow && last.is_none_or(|last| last >= head) {
        return ExitCode::from(DONE);
    }
    let mut out = std::io::stdout().lock();
    while let Some(event) = client.next_event().await {
        let Some(seq) = event.seq else { continue };
        let written = serde_json::to_string(&event)
            .map_err(std::io::Error::other)
            .and_then(|json| writeln!(out, "{json}"))
            .and_then(|()| out.flush());
        match written {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => {
                return ExitCode::from(DONE);
            }
            Err(err) => {
                eprintln!("rup: cannot write to stdout: {err}");
                return ExitCode::from(FAILED);
            }
        }
        last = Some(seq);
        if !follow && seq >= head {
            return ExitCode::from(DONE);
        }
    }
    let at = last.unwrap_or(head);
    eprintln!("rup: connection lost after event {at}; resume with --after {at}");
    ExitCode::from(FAILED)
}
