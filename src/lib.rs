#![allow(unused_features)]
#![feature(unique, unsafe_no_drop_flag, alloc)]
#![feature(heap_api, oom, box_raw, filling_drop, num_bits_bytes)]
#![feature(core_intrinsics)]

#![feature(optin_builtin_traits)] // for `unsafe impl Raw for ..`

#![feature(placement_new_protocol, placement_in_syntax)]


extern crate alloc as alloc_crate;

// extern crate allocprint;

pub mod alloc;
pub mod raw_vec;
pub mod boxed;
pub mod boxing;
// pub mod btree { mod node; }

#[cfg(test)]
mod tests;
