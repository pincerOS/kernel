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

/// Parse a boolean flag; true is "-c" or "-c=true", false is "-c=false"
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
