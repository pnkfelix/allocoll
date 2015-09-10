use alloc;
use alloc_crate::raw_vec::RawVec;

use std::mem;

#[derive(Copy, Clone)]
pub struct Alloc;

impl alloc::Alloc for Alloc {
    #[inline]
    unsafe fn alloc(&mut self, kind: alloc::Kind) -> alloc::Address {
        // TODO: ensure alignment too
        let data: RawVec<u8> = RawVec::with_capacity(kind.size());
        let p = data.ptr();
        // println!("  alloc kind: {:?} => {:p}", kind, p);
        mem::forget(data);
        p
    }
    #[inline]
    unsafe fn dealloc(&mut self, ptr: alloc::Address, kind: alloc::Kind) {
        // println!("dealloc ptr {:p} kind: {:?}", ptr, kind);
        drop(RawVec::from_raw_parts(ptr, kind.size()))
    }
}
