//! `nom-recursive` is an extension of [nom](https://docs.rs/nom) to handle left recursion.
//!
//! ## Examples
//!
//! The following example show a quick example.
//! If `#[recursive_parser]` is removed, stack overflow will occur because of infinite recursion.
//!
//! ```
//! use nom::branch::*;
//! use nom::character::complete::*;
//! use nom::IResult;
//! use nom_locate::LocatedSpan;
//! use nom_recursive::{recursive_parser, RecursiveInfo};
//!
//! // Input type must implement trait HasRecursiveInfo
//! // nom_locate::LocatedSpan<T, RecursiveInfo> implements it.
//! type Span<'a> = LocatedSpan<&'a str, RecursiveInfo<&'a str>>;
//!
//! pub fn expr(s: Span) -> IResult<Span, String> {
//!     alt((expr_binary, term))(s)
//! }
//!
//! // Apply recursive_parser by custom attribute
//! #[recursive_parser]
//! pub fn expr_binary(s: Span) -> IResult<Span, String> {
//!     let (s, x) = expr(s)?;
//!     let (s, y) = char('+')(s)?;
//!     let (s, z) = expr(s)?;
//!     let ret = format!("{}{}{}", x, y, z);
//!     Ok((s, ret))
//! }
//!
//! pub fn term(s: Span) -> IResult<Span, String> {
//!     let (s, x) = char('1')(s)?;
//!     Ok((s, x.to_string()))
//! }
//!
//! fn main() {
//!     let ret = expr(LocatedSpan::new_extra("1+1", RecursiveInfo::new()));
//!     println!("{:?}", ret.unwrap().1);
//! }
//! ```

pub use nom_recursive_macros::recursive_parser;
use std::collections::HashMap;

#[cfg(all(not(feature = "tracer128"), not(feature = "tracer256"),))]
const RECURSIVE_FLAG_WORDS: usize = 1;
#[cfg(all(feature = "tracer128", not(feature = "tracer256"),))]
const RECURSIVE_FLAG_WORDS: usize = 2;
#[cfg(feature = "tracer256")]
const RECURSIVE_FLAG_WORDS: usize = 4;

pub struct RecursiveIndexes {
    indexes: HashMap<&'static str, usize>,
    next: usize,
}

impl RecursiveIndexes {
    pub fn new() -> Self {
        RecursiveIndexes {
            indexes: HashMap::new(),
            next: 0,
        }
    }

    pub fn get(&mut self, key: &'static str) -> usize {
        if let Some(x) = self.indexes.get(key) {
            *x
        } else {
            let new_index = self.next;
            assert!(new_index < RECURSIVE_FLAG_WORDS * 64, "Recursive tracers exceed the maximum number({}). Consider use feature `tracer128` or `tracer256` to extend it.", RECURSIVE_FLAG_WORDS * 64);
            self.next += 1;
            self.indexes.insert(key, new_index);
            new_index
        }
    }
}

thread_local!(
    pub static RECURSIVE_STORAGE: core::cell::RefCell<crate::RecursiveIndexes> = {
        core::cell::RefCell::new(crate::RecursiveIndexes::new())
    }
);

/// The type of payload used by recursive tracer
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecursiveInfo<T: Clone + Default> {
    flag: [u64; RECURSIVE_FLAG_WORDS],
    copy: T,
}

impl<T: Clone + Default> Default for RecursiveInfo<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Default> RecursiveInfo<T> {
    pub fn new() -> Self {
        RecursiveInfo {
            flag: [0; RECURSIVE_FLAG_WORDS],
            copy: Default::default(),
        }
    }

    pub fn check_flag(&self, id: usize) -> bool {
        let upper = id / 64;
        let lower = id % 64;
        ((self.flag[upper] >> lower) & 1) == 1
    }

    pub fn set_flag(&mut self, id: usize) {
        let upper = id / 64;
        let lower = id % 64;

        let val = 1u64 << lower;
        let mask = !(1u64 << lower);

        self.flag[upper] = (self.flag[upper] & mask) | val;
    }

    pub fn clear_flags(&mut self) {
        for i in 0..self.flag.len() {
            self.flag[i] = 0u64;
        }
    }

    pub fn get_copy(&self) -> T {
        self.copy.clone()
    }

    pub fn set_copy(&mut self, copy: T) {
        self.copy = copy;
    }
}

/// Trait for recursive tracer
///
/// The input type of nom parser must implement this.
pub trait HasRecursiveInfo<T: Clone + Default> {
    fn get_recursive_info(&self) -> RecursiveInfo<T>;
    fn set_recursive_info(self, info: RecursiveInfo<T>) -> Self;
}

impl<T: Clone + Default> HasRecursiveInfo<T> for RecursiveInfo<T> {
    fn get_recursive_info(&self) -> RecursiveInfo<T> {
        self.clone()
    }

    fn set_recursive_info(self, info: RecursiveInfo<T>) -> Self {
        info
    }
}

pub trait HasRecursiveType<T: Clone + Default> {
    fn get_value(&self) -> T;
}

impl<T: Clone + Default, U> HasRecursiveType<T> for nom_locate::LocatedSpan<T, U>
where
    U: HasRecursiveInfo<T>,
{
    fn get_value(&self) -> T {
        self.fragment().clone()
    }
}

impl<T: Clone + Default, U> HasRecursiveInfo<T> for nom_locate::LocatedSpan<T, U>
where
    U: HasRecursiveInfo<T>,
{
    fn get_recursive_info(&self) -> RecursiveInfo<T> {
        self.extra.get_recursive_info()
    }

    fn set_recursive_info(mut self, info: RecursiveInfo<T>) -> Self {
        self.extra = self.extra.set_recursive_info(info);
        self
    }
}
