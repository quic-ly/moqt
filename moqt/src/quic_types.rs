#[allow(non_camel_case_types)]
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum Perspective {
    #[default]
    IS_SERVER,
    IS_CLIENT,
}
