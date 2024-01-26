use core::ops::Deref;

pub const OK: Result<(), ()> = Ok(());
pub const FAIL: Result<(), ()> = Err(());
