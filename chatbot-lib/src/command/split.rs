use core::iter::FusedIterator;
#[derive(Debug, Clone)]
pub struct CommandArguments<'a> {
    str: &'a str,
    range: std::ops::Range<usize>,
}

impl<'a> From<&'a str> for CommandArguments<'a> {
    fn from(value: &'a str) -> Self {
        CommandArguments {
            str: value,
            range: 0..value.len(),
        }
    }
}

fn none_if_empty(value: &str) -> Option<&str> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}
/*
fn split_whitespace_once(value: &str) -> (&str, &str) {
    match value.find(char::is_whitespace) {
        Some(index) => value.split_at(index),
        None => (value, ""),
    }
}
*/
fn find_index<F>(value: &str, f: F) -> usize
where
    F: Fn(char) -> bool,
{
    value.find(f).unwrap_or(value.len())
}

fn rfind_index_plus_one<F>(value: &str, f: F) -> usize
where
    F: Fn(char) -> bool,
{
    value
        .char_indices()
        .rev()
        .take_while(|(_, c)| !(f)(*c))
        .last()
        .map_or_else(|| value.len(), |(index, _)| index)
}
/*
fn rsplit_whitespace_once(value: &str) -> (&str, &str) {
    value.split_at(rfind_index_plus_one(value, char::is_whitespace))
}
*/
impl<'a> CommandArguments<'a> {
    pub fn as_str(&self) -> &'a str {
        self.str[self.range.clone()].trim()
    }

    pub fn next_rest(&mut self) -> Option<<Self as Iterator>::Item> {
        let result = self.as_str();
        self.range = 0..0;
        none_if_empty(result)
    }

    pub fn consumed_begin(&self) -> Self {
        Self::from(&self.str[..self.range.start])
    }

    pub fn consumed_end(&self) -> Self {
        Self::from(&self.str[self.range.end..])
    }
}

impl<'a> Iterator for CommandArguments<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        let std::ops::Range { start, end } = self.range;
        self.str[start..end]
            .find(|c: char| !c.is_whitespace())
            .map(|i| start + i)
            .and_then(|start| {
                let index = start + find_index(&self.str[start..end], char::is_whitespace);
                self.range = index..end;
                none_if_empty(&self.str[start..index])
            })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some((self.range.len() + 1) / 2))
    }

    fn last(mut self) -> Option<&'a str> {
        self.next_back()
    }
}

impl FusedIterator for CommandArguments<'_> {}

impl DoubleEndedIterator for CommandArguments<'_> {
    fn next_back(&mut self) -> Option<<Self as Iterator>::Item> {
        let std::ops::Range { start, end } = self.range;
        self.str[start..end]
            .rfind(|c: char| !c.is_whitespace())
            .map(|i| start + i + 1)
            .and_then(|end| {
                let index = rfind_index_plus_one(&self.str[start..end], char::is_whitespace);
                self.range = start..index;
                none_if_empty(&self.str[index..end])
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rfind_index_plus_one() {
        assert_eq!(rfind_index_plus_one("_____abc", |c| c == 'c'), 8);
        assert_eq!(rfind_index_plus_one("_____abc", |c| c == 'b'), 7);
        assert_eq!(rfind_index_plus_one("_____abc", |c| c == 'a'), 6);
        assert_eq!(rfind_index_plus_one("_____abc", |c| c == '_'), 5);
        assert_eq!(rfind_index_plus_one("_____abc", |_| false), 0);
        assert_eq!(rfind_index_plus_one("", |_| false), 0);
        assert_eq!(rfind_index_plus_one("", |_| true), 0);
        assert_eq!(rfind_index_plus_one(" ", |_| false), 0);
        assert_eq!(rfind_index_plus_one(" ", |_| true), 1);

        let value = "abc def";
        assert_eq!(
            value.split_at(rfind_index_plus_one(value, char::is_whitespace)),
            ("abc ", "def")
        );
        let value = "abcdef";
        assert_eq!(
            value.split_at(rfind_index_plus_one(value, char::is_whitespace)),
            ("", "abcdef")
        );
        let value = "abcdef ";
        assert_eq!(
            value.split_at(rfind_index_plus_one(value, char::is_whitespace)),
            ("abcdef ", "")
        );
        let value = " abcdef";
        assert_eq!(
            value.split_at(rfind_index_plus_one(value, char::is_whitespace)),
            (" ", "abcdef")
        );
    }

    #[test]
    fn test() {
        let test = "Hello World!";
        let mut iter = CommandArguments::from(test);
        assert_eq!(iter.next(), Some("Hello"));
        assert_eq!(iter.consumed_begin().as_str(), "Hello");
        assert_eq!(iter.next(), Some("World!"));
        assert_eq!(iter.consumed_begin().as_str(), "Hello World!");
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_more_spaces() {
        let test = "   Hello    World!   ";
        let mut iter = CommandArguments::from(test);
        assert_eq!(iter.next(), Some("Hello"));
        assert_eq!(iter.consumed_begin().as_str(), "Hello");
        assert_eq!(iter.next(), Some("World!"));
        assert_eq!(iter.consumed_begin().as_str(), "Hello    World!");
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_back() {
        let test = "Hello World!";
        let mut iter = CommandArguments::from(test);
        assert_eq!(iter.next_back(), Some("World!"));
        assert_eq!(iter.consumed_end().as_str(), "World!");
        assert_eq!(iter.next_back(), Some("Hello"));
        assert_eq!(iter.consumed_end().as_str(), "Hello World!");
        assert_eq!(iter.next_back(), None);
    }

    #[test]
    fn test_back_more_spaces() {
        let test = "   Hello    World!   ";
        let mut iter = CommandArguments::from(test);
        assert_eq!(iter.next_back(), Some("World!"));
        assert_eq!(iter.consumed_end().as_str(), "World!");
        assert_eq!(iter.next_back(), Some("Hello"));
        assert_eq!(iter.consumed_end().as_str(), "Hello    World!");
        assert_eq!(iter.next_back(), None);
    }

    #[test]
    fn test_as_str() {
        let test = "   This \t\n is \n   a text to test this.  ";
        let mut iter = CommandArguments::from(test);
        assert_eq!(iter.consumed_begin().as_str(), "");
        assert_eq!(iter.consumed_end().as_str(), "");
        assert_eq!(iter.next(), Some("This"));
        assert_eq!(iter.consumed_begin().as_str(), "This");
        assert_eq!(iter.next(), Some("is"));
        assert_eq!(iter.consumed_begin().as_str(), "This \t\n is");
        assert_eq!(iter.as_str(), "a text to test this.");
        assert_eq!(iter.next(), Some("a"));
        assert_eq!(iter.consumed_begin().as_str(), "This \t\n is \n   a");
        assert_eq!(iter.as_str(), "text to test this.");
    }

    #[test]
    fn test_as_str_empty() {
        let test = "";
        let mut iter = CommandArguments::from(test);
        assert_eq!(iter.as_str(), "");
        assert_eq!(iter.consumed_begin().as_str(), "");
        assert_eq!(iter.consumed_end().as_str(), "");
        assert_eq!(iter.next(), None);
        assert_eq!(iter.as_str(), "");
        assert_eq!(iter.consumed_begin().as_str(), "");
        assert_eq!(iter.consumed_end().as_str(), "");
    }

    #[test]
    fn test_as_str_whitespaces() {
        let test = " \t\n ";
        let mut iter = CommandArguments::from(test);
        assert_eq!(iter.as_str(), "");
        assert_eq!(iter.consumed_begin().as_str(), "");
        assert_eq!(iter.consumed_end().as_str(), "");
        assert_eq!(iter.next(), None);
        assert_eq!(iter.as_str(), "");
        assert_eq!(iter.consumed_begin().as_str(), "");
        assert_eq!(iter.consumed_end().as_str(), "");
    }

    #[test]
    fn test_next_rest() {
        let test = "   This \t\n is \n   a text to test this.  ";
        let mut iter = CommandArguments::from(test);
        assert_eq!(iter.next(), Some("This"));
        assert_eq!(iter.next(), Some("is"));
        assert_eq!(iter.next_rest(), Some("a text to test this."));
        assert_eq!(iter.as_str(), "");
        assert_eq!(iter.next_rest(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.as_str(), "");
    }

    #[test]
    fn test_next_rest_empty() {
        let test = "";
        let mut iter = CommandArguments::from(test);
        assert_eq!(iter.as_str(), "");
        assert_eq!(iter.next_rest(), None);
        assert_eq!(iter.as_str(), "");
        assert_eq!(iter.next(), None);
        assert_eq!(iter.as_str(), "");
    }

    #[test]
    fn test_next_rest_whitespaces() {
        let test = " \t\n ";
        let mut iter = CommandArguments::from(test);
        assert_eq!(iter.as_str(), "");
        assert_eq!(iter.next_rest(), None);
        assert_eq!(iter.as_str(), "");
        assert_eq!(iter.next(), None);
        assert_eq!(iter.as_str(), "");
    }

    #[test]
    fn test_size_hint() {
        let tests = [
            "",
            " ",
            "   ",
            "hello",
            " a",
            "a ",
            "a b",
            "a b c",
            " a b c",
            "a b c ",
            "  hello     world  ",
            "ß",
            "öäüß",
            " ö ä ü ß ",
        ];
        for test in &tests {
            let iter = CommandArguments::from(*test);
            let (min, max) = iter.size_hint();
            let count = iter.count();
            println!("{:?}: {:?} <= {:?} <= {:?}", test, min, count, max);
            assert!(min <= count, "{} <= {}", min, count);
            if let Some(max) = max {
                assert!(count <= max, "{} <= {}", count, max);
            }
        }
    }
}
