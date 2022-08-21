use std::{borrow::Cow, time::Duration, time::SystemTime};

pub trait FromArgument<'a>: Sized {
    type Error: std::error::Error + Send + Sync + 'static;
    fn from_argument(argument: &'a str) -> Result<Self, Self::Error>;
}

impl<'a> FromArgument<'a> for &'a str {
    type Error = core::convert::Infallible;
    fn from_argument(argument: &'a str) -> Result<Self, Self::Error> {
        Ok(argument)
    }
}

impl<'a, T: FromArgument<'a>> FromArgument<'a> for Option<T> {
    type Error = core::convert::Infallible;
    fn from_argument(argument: &'a str) -> Result<Self, Self::Error> {
        Ok(<T as FromArgument>::from_argument(argument).ok())
    }
}

impl<'a, T: FromArgument<'a>, E> FromArgument<'a> for Result<T, E>
where
    T::Error: Into<E>,
{
    type Error = core::convert::Infallible;
    fn from_argument(argument: &'a str) -> Result<Self, Self::Error> {
        Ok(<T as FromArgument>::from_argument(argument).map_err(|e| e.into()))
    }
}

macro_rules! impl_from_argument {
    ($($ty:ty) +) => {
        $(
            impl FromArgument<'_> for $ty {
                type Error = <Self as core::str::FromStr>::Err;
                fn from_argument(argument: &str) -> Result<Self, Self::Error> {
                    argument.parse()
                }
            }
        )+
    };
}

impl_from_argument! {
    bool char f32 f64 i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize String
    std::net::IpAddr
    std::net::SocketAddr
    std::ffi::OsString
    std::net::Ipv4Addr
    std::net::Ipv6Addr
    std::net::SocketAddrV4
    std::net::SocketAddrV6
    std::num::NonZeroI8
    std::num::NonZeroI16
    std::num::NonZeroI32
    std::num::NonZeroI64
    std::num::NonZeroI128
    std::num::NonZeroIsize
    std::num::NonZeroU8
    std::num::NonZeroU16
    std::num::NonZeroU32
    std::num::NonZeroU64
    std::num::NonZeroU128
    std::num::NonZeroUsize
    std::path::PathBuf

    humantime::Duration
    humantime::Timestamp

    chrono::NaiveDate
    chrono::NaiveDateTime
    chrono::NaiveTime

    http::uri::Uri
    url::Url
}

impl FromArgument<'_> for () {
    type Error = core::convert::Infallible;
    fn from_argument(_argument: &str) -> Result<Self, Self::Error> {
        Ok(())
    }
}

impl<'a> FromArgument<'a> for Cow<'a, str> {
    type Error = core::convert::Infallible;
    fn from_argument(argument: &'a str) -> Result<Self, Self::Error> {
        Ok(argument.into())
    }
}

impl<'a> FromArgument<'a> for Duration {
    type Error = humantime::DurationError;
    fn from_argument(argument: &'a str) -> Result<Self, Self::Error> {
        humantime::parse_duration(argument)
    }
}

impl<'a> FromArgument<'a> for SystemTime {
    type Error = humantime::TimestampError;
    fn from_argument(argument: &'a str) -> Result<Self, Self::Error> {
        humantime::parse_rfc3339(argument)
    }
}
