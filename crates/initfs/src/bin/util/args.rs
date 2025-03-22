use alloc::string::String;

#[derive(Debug, Clone)]
pub enum ArgError {
    InvalidBool(String),
    MissingArg(String),
}

impl core::fmt::Display for ArgError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ArgError::InvalidBool(arg) => write!(f, "Invalid boolean flag: {:?}", arg),
            ArgError::MissingArg(flag) => write!(f, "Flag -{} is missing an argument", flag),
        }
    }
}

impl core::error::Error for ArgError {}

/// Parse an inline boolean flag; true is `-c`` or `-c=true`, false is `-c=false`
///
/// Also supports `yes` and `no` as alternatives to `true` and `false`.
pub fn parse_flag_optional_bool(value: Option<&str>) -> Result<bool, ArgError> {
    match value {
        None => Ok(true),
        Some("false" | "no") => Ok(false),
        Some("true" | "yes") => Ok(true),
        Some(s) => Err(ArgError::InvalidBool(s.into())),
    }
}

/// Parse a required parameter for an option, either inline or as the next arg
pub fn parse_param(
    flag: &str,
    args: &mut impl Iterator<Item = String>,
    inline: Option<&str>,
) -> Result<String, ArgError> {
    match inline {
        Some(v) => Ok(v.into()),
        None => args.next().ok_or_else(|| ArgError::MissingArg(flag.into())),
    }
}

/// A simple command-line argument parser; for each argument, this will
/// determine whether it should be a flag or a positional argument.
///
/// For flags (arguments starting with `-`) it will call `handle_flag`
/// with the flag (with the leading `-` removed), an inline assigned
/// value, if present (`--arg=value`), a mutable reference to the arg
/// iterator for consuming more values, and the name of the program
/// (as given by the first argument).
///
/// Arguments not starting with `-`, or any argument after an argument
/// "--", will be treated as a positional argument; `handle_pos` will
/// be called with the argument and its index among positional arguments.
///
/// Either function may return errors, which immediately stop argument
/// processing and are returned, or may return `Ok(None)` to stop argument
/// parsing without an error.  (This is useful for informational flags like
/// `--help` or `--version`.)
///
/// Flag format: `-{flag}` or `-{flag}={inline}`; double-dashed flags
/// like `--help` are handled as flags where the flag itself starts with
/// `-`: "-help".
pub fn parse_args<I, F, P, E>(
    mut args: I,
    mut handle_flag: F,
    mut handle_pos: P,
) -> Result<Option<()>, E>
where
    I: Iterator<Item = String>,
    F: FnMut(&str, Option<&str>, &mut I, &str) -> Result<Option<()>, E>,
    P: FnMut(usize, String) -> Result<Option<()>, E>,
{
    let mut in_flags = true;
    let mut pos_index = 0;
    let arg0 = args.next().unwrap_or_else(|| "unknown".into());

    while let Some(arg) = args.next() {
        if in_flags && arg.starts_with("-") {
            let (flag, inline) = arg[1..].split_once('=').unzip();
            let flag = flag.unwrap_or(&arg[1..]);

            if flag == "-" && inline.is_none() {
                in_flags = false;
            } else {
                let res = handle_flag(flag, inline, &mut args, &arg0)?;
                if res.is_none() {
                    return Ok(None);
                }
            }
        } else {
            let res = handle_pos(pos_index, arg)?;
            if res.is_none() {
                return Ok(None);
            }
            pos_index += 1;
        }
    }

    Ok(Some(()))
}
