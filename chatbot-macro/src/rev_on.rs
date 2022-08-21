use std::iter::FusedIterator;

pub enum RevOn<I, F>
where
    I: Iterator,
{
    Forwards {
        inner: I,
        f: F,
    },
    Backwards {
        inner: I,
        last: <I as Iterator>::Item,
    },
    Empty,
}

impl<I, F> Default for RevOn<I, F>
where
    I: Iterator,
{
    fn default() -> Self {
        Self::Empty
    }
}

mod private {
    pub trait Sealed {}

    // Implement for those same types, but no others.
    impl<I> Sealed for I where I: DoubleEndedIterator {}
}

pub trait RevOnIterator: DoubleEndedIterator + Sized + private::Sealed {
    fn rev_on<F>(self, f: F) -> RevOn<Self, F>
    where
        F: Fn(&<Self as Iterator>::Item) -> bool;
}

impl<I> RevOnIterator for I
where
    I: DoubleEndedIterator,
{
    fn rev_on<F>(self, f: F) -> RevOn<Self, F>
    where
        F: Fn(&<Self as Iterator>::Item) -> bool,
    {
        RevOn::Forwards { inner: self, f }
    }
}

impl<I, F> Iterator for RevOn<I, F>
where
    I: DoubleEndedIterator,
    F: Fn(&<I as Iterator>::Item) -> bool,
{
    type Item = (<I as Iterator>::Item, bool);

    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        loop {
            return match std::mem::take(self) {
                RevOn::Forwards { mut inner, f } => match inner.next() {
                    Some(next) if (f)(&next) => {
                        *self = RevOn::Backwards { inner, last: next };
                        continue;
                    }
                    Some(next) => {
                        *self = RevOn::Forwards { inner, f };
                        Some((next, false))
                    }
                    None => None,
                },
                RevOn::Backwards { mut inner, last } => match inner.next_back() {
                    Some(next) => {
                        *self = RevOn::Backwards { inner, last };
                        Some((next, true))
                    }
                    None => Some((last, true)),
                },
                RevOn::Empty => None,
            };
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            RevOn::Forwards { ref inner, .. } => inner.size_hint(),
            RevOn::Backwards { ref inner, .. } => {
                let (min, max) = inner.size_hint();
                (
                    min.saturating_add(1),
                    max.and_then(|max| max.checked_add(1)),
                )
            }
            RevOn::Empty => (0, Some(0)),
        }
    }

    fn last(self) -> Option<Self::Item> {
        match self {
            RevOn::Forwards { mut inner, f } => match inner.try_fold(None, |_, value| {
                if (f)(&value) {
                    Err(value)
                } else {
                    Ok(Some(value))
                }
            }) {
                Ok(None) => None,
                Ok(Some(value)) => Some((value, false)),
                Err(value) => Some((value, true)),
            },
            RevOn::Backwards { last, .. } => Some((last, true)),
            RevOn::Empty => None,
        }
    }
}

impl<I, F> ExactSizeIterator for RevOn<I, F>
where
    I: DoubleEndedIterator + ExactSizeIterator,
    F: Fn(&<I as Iterator>::Item) -> bool,
{
    fn len(&self) -> usize {
        match self {
            RevOn::Forwards { ref inner, .. } => inner.len(),
            RevOn::Backwards { ref inner, .. } => inner.len().checked_add(1).unwrap(),
            RevOn::Empty => 0,
        }
    }
    /*
        fn is_empty(&self) -> bool {
            match self {
                RevOn::Forwards { ref inner, .. } => inner.is_empty(),
                RevOn::Backwards { .. } => false,
                RevOn::Empty => true,
            }
        }
    */
}

impl<I, F> FusedIterator for RevOn<I, F>
where
    I: DoubleEndedIterator + FusedIterator,
    F: Fn(&<I as Iterator>::Item) -> bool,
{
}
