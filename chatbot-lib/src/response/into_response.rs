use super::Response;
use crate::request::CommandRequest;

pub trait IntoResponse<'a> {
    fn into_response(self, request: &CommandRequest<'_>) -> Response<'a>;
}

impl<'a> IntoResponse<'a> for () {
    fn into_response(self, _request: &CommandRequest<'_>) -> Response<'a> {
        Response::none()
    }
}

impl<'a, T: IntoResponse<'a>> IntoResponse<'a> for Option<T> {
    fn into_response(self, request: &CommandRequest<'_>) -> Response<'a> {
        match self {
            None => Response::none(),
            Some(value) => value.into_response(request),
        }
    }
}

impl<'a> IntoResponse<'a> for Box<str> {
    fn into_response(self, _request: &CommandRequest<'_>) -> Response<'a> {
        Response::new(self.into_string())
    }
}

impl<'a> IntoResponse<'a> for &'a Box<str> {
    fn into_response(self, _request: &CommandRequest<'_>) -> Response<'a> {
        Response::new(self.as_ref())
    }
}

macro_rules! impl_cow_into_response {
    ($($ty:ty) +) => {
        $(
            impl<'a> IntoResponse<'a> for $ty {
                fn into_response(self, _request: &CommandRequest<'_>) -> Response<'a> {
                    Response::new(self)
                }
            }
        )+
    };
}

impl_cow_into_response! {
    &'a str
    String
    &'a String
    std::borrow::Cow<'a, str>
}

macro_rules! impl_into_response {
    ($($ty:ty) +) => {
        $(
            impl<'a> IntoResponse<'a> for $ty {
                fn into_response(self, _request: &CommandRequest<'_>) -> Response<'a> {
                    Response::new(self.to_string())
                }
            }

            impl<'a> IntoResponse<'a> for &$ty {
                fn into_response(self, _request: &CommandRequest<'_>) -> Response<'a> {
                    Response::new(self.to_string())
                }
            }
        )+
    };
}

impl_into_response! {
    bool char f32 f64 i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize
    std::net::IpAddr
    std::net::SocketAddr
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
}
