use std::fmt::Display;

#[derive(Debug, PartialEq, Eq)]
pub enum CommandPattern<'a> {
    Command(&'a str),
    Subcommand(&'a str),
    Argument {
        name: &'a str,
        take_all: bool,
        optional: bool,
    },
    TakeAll,
}

impl Display for CommandPattern<'_> {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::result::Result<(), std::fmt::Error> {
        match self {
            CommandPattern::Command(str) | CommandPattern::Subcommand(str) => str.fmt(formatter),
            CommandPattern::TakeAll => "..".fmt(formatter),
            CommandPattern::Argument {
                name,
                take_all: false,
                optional: false,
            } => write!(formatter, "<{}>", name),
            CommandPattern::Argument {
                name,
                take_all: false,
                optional: true,
            } => write!(formatter, "[{}]", name),
            CommandPattern::Argument {
                name,
                take_all: true,
                optional: false,
            } => write!(formatter, "<{}..>", name),
            CommandPattern::Argument {
                name,
                take_all: true,
                optional: true,
            } => write!(formatter, "[{}..]", name),
        }
    }
}

impl<'a> CommandPattern<'a> {
    pub fn key(&self) -> &'a str {
        match self {
            CommandPattern::Command(value)
            | CommandPattern::Subcommand(value)
            | CommandPattern::Argument { name: value, .. } => value,
            CommandPattern::TakeAll => "",
        }
    }

    pub fn is_taking_all(&self) -> bool {
        matches!(
            self,
            CommandPattern::Argument { take_all: true, .. } | CommandPattern::TakeAll
        )
    }

    pub fn is_optional(&self) -> bool {
        matches!(
            self,
            CommandPattern::Argument { optional: true, .. } | CommandPattern::TakeAll
        )
    }
}

impl<'a> From<&'a str> for CommandPattern<'a> {
    fn from(value: &'a str) -> Self {
        if value.starts_with('!') {
            Self::Command(value)
        } else if value == ".." {
            Self::TakeAll
        } else if let Some(value) = value
            .strip_prefix('<')
            .and_then(|value| value.strip_suffix('>'))
        {
            match value.strip_suffix("..") {
                Some(value) => Self::Argument {
                    name: value,
                    take_all: true,
                    optional: false,
                },
                None => Self::Argument {
                    name: value,
                    take_all: false,
                    optional: false,
                },
            }
        } else if let Some(value) = value
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
        {
            match value.strip_suffix("..") {
                Some(value) => Self::Argument {
                    name: value,
                    take_all: true,
                    optional: true,
                },
                None => Self::Argument {
                    name: value,
                    take_all: false,
                    optional: true,
                },
            }
        } else {
            Self::Subcommand(value)
        }
    }
}

impl std::borrow::Borrow<str> for CommandPattern<'_> {
    fn borrow(&self) -> &str {
        self.key()
    }
}

impl std::hash::Hash for CommandPattern<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key().hash(state);
    }
}
