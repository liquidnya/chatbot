use super::CommandArguments;
use itertools::Itertools;

pub struct FindSharedSyntax<'a> {
    prefix: CommandArguments<'a>,
    choice: Vec<&'a str>,
}

/*
// I don't know what does function ever did
fn find_prefix_index(a: &str, b: &str) -> usize {
    let mut iter = a.char_indices().zip(b.char_indices());
    let value: Option<usize> = iter
        .find(|((_, a), (_, b))| a != b)
        .map(|((a, _), (b, _))| {
            assert_eq!(a, b);
            a
        });
    value.unwrap_or_else(|| std::cmp::min(a.len(), b.len()))
}
*/

impl<'a> ToString for FindSharedSyntax<'a> {
    fn to_string(&self) -> String {
        if self.choice.is_empty() {
            self.prefix.as_str().to_string()
        } else {
            format!("{} {}", self.prefix.as_str(), self.choice.iter().join("|"))
        }
    }
}

impl<'a> FindSharedSyntax<'a> {
    pub fn new(command: &'a str) -> Self {
        Self {
            prefix: command.into(),
            choice: Vec::with_capacity(0),
        }
    }

    pub fn append(&mut self, command: &'a str) {
        let mut command = CommandArguments::from(command);
        let mut prefix = self.prefix.clone();
        loop {
            let next_prefix = prefix.next();
            let next_command = command.next();
            if next_prefix == None {
                if let Some(next_command) = next_command {
                    self.choice.push(next_command);
                }
                break;
            }
            if next_prefix != next_command {
                self.prefix = prefix.consumed_begin();
                self.prefix.next_back();
                self.choice.clear();
                if let Some(next_prefix) = next_prefix {
                    self.choice.push(next_prefix);
                }
                if let Some(next_command) = next_command {
                    self.choice.push(next_command);
                }
                break;
            }
        }
    }
}

#[cfg(test)]
mod test {
    /*use super::find_prefix_index;
    #[test]
    fn test_find_prefix_index() {
        assert_eq!(find_prefix_index("abc", "abc"), 3);
        assert_eq!(find_prefix_index("abc", "def"), 0);
        assert_eq!(find_prefix_index("aba", "abä"), 2);
    }*/

    use super::FindSharedSyntax;

    #[test]
    fn test_find_prefix_index() {
        let mut find = FindSharedSyntax::new("!song add <command> <url> <cooldown..>");
        find.append("!song rm <command>");
        println!("{}", find.to_string());
    }
}
