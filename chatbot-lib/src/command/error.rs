use core::fmt::Debug;

#[derive(Debug)]
pub enum CommandError<Error> {
    CommandMismatch,
    SubcommandMismatch,
    ArgumentMissing,
    ArgumentParsing(Error),
    ArgumentsLeftOver,
    NamedArgumentParsing(&'static str, Error),
    RequestError(Error),
}

impl<Error> CommandError<Error> {
    pub fn map_err<F, O>(self, op: O) -> CommandError<F>
    where
        O: FnOnce(Error) -> F,
    {
        match self {
            CommandError::CommandMismatch => CommandError::CommandMismatch,
            CommandError::SubcommandMismatch => CommandError::SubcommandMismatch,
            CommandError::ArgumentMissing => CommandError::ArgumentMissing,
            CommandError::ArgumentParsing(error) => CommandError::ArgumentParsing(op(error)),
            CommandError::ArgumentsLeftOver => CommandError::ArgumentsLeftOver,
            CommandError::NamedArgumentParsing(name, error) => {
                CommandError::NamedArgumentParsing(name, op(error))
            }
            CommandError::RequestError(error) => CommandError::RequestError(op(error)),
        }
    }

    pub fn is_argument_error(&self) -> bool {
        matches!(
            self,
            CommandError::ArgumentMissing
                | CommandError::ArgumentParsing(_)
                | CommandError::ArgumentsLeftOver
                | CommandError::NamedArgumentParsing(_, _)
        )
    }

    pub fn is_subcommand_mismatch(&self) -> bool {
        matches!(self, CommandError::SubcommandMismatch)
    }
}

impl<'a, Error: Debug + 'a> CommandError<Error> {
    pub fn dyn_err(self) -> CommandError<Box<dyn Debug + 'a>> {
        self.map_err(|err| -> Box<dyn std::fmt::Debug + 'a> { Box::new(err) })
    }
}

impl<Error> CommandError<Error> {
    pub fn unit_err(self) -> CommandError<()> {
        self.map_err(|_| ())
    }
}
