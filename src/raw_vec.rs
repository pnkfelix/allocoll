use alloc::{self, Alloc, DefaultAlloc};
use boxed::Box;

use alloc_crate::heap::EMPTY;
use alloc_crate::oom;

use std::mem;
use std::ptr::Unique;
use std::slice::{self};
use std::{isize, usize};

#[unsafe_no_drop_flag]
pub struct RawVec<T, A:Alloc = DefaultAlloc> {
    ptr: Unique<T>,
    cap: usize,
    alloc: A,
}

fn empty<T>() -> (Unique<T>, usize) {
    // !0 is usize::MAX. This branch should be stripped at compile time.
    let cap = if mem::size_of::<T>() == 0 { !0 } else { 0 };

    // heap::EMPTY doubles as "unallocated" and "zero-sized allocation"
    unsafe { (Unique::new(EMPTY as *mut T), cap) }
}

impl<T, A:Alloc> RawVec<T, A> {
    pub fn new() -> Self where A: Default {
        Self::with_alloc(Default::default())
    }

    pub fn with_alloc(a: A) -> Self {
        let (ptr, cap) = empty();
        RawVec { ptr: ptr, cap: cap, alloc: a }
    }

    pub fn with_capacity(cap: usize) -> Self where A: Default {
        Self::with_capacity_alloc(cap, Default::default())
    }

    pub fn with_capacity_alloc(cap: usize, mut a: A) -> Self {
        unsafe {
            let elem_size = mem::size_of::<T>();

            let alloc_size = cap.checked_mul(elem_size).expect("capacity overflow");
            alloc_guard(alloc_size);

            // handles ZSTs and `cap = 0` alike
            let ptr = if alloc_size == 0 {
                EMPTY as *mut u8
            } else {
                let ptr = a.alloc(alloc::Kind::new::<T>().array(cap));
                if ptr.is_null() { oom() }
                ptr
            };

            RawVec { ptr: Unique::new(ptr as *mut _), cap: cap, alloc: a }
        }
    }

    pub unsafe fn from_raw_parts(ptr: *mut T, cap: usize) -> Self where A: Default {
        RawVec { ptr: Unique::new(ptr), cap: cap, alloc: Default::default() }
    }

    pub unsafe fn from_raw_parts_alloc(ptr: *mut T, cap: usize, a: A) -> Self {
        RawVec { ptr: Unique::new(ptr), cap: cap, alloc: a }
    }

    pub fn from_box(slice: Box<[T], A>) -> Self {
        unsafe {
            let len = slice.len();
            let (mut v, a) = slice.value_alloc();
            RawVec::from_raw_parts_alloc(v.get_mut().as_mut_ptr(), len, a)
        }
    }
}

impl<T, A:Alloc> RawVec<T, A> {
    pub fn ptr(&self) -> *mut T {
        *self.ptr
    }

    pub fn cap(&self) -> usize {
        if mem::size_of::<T>() == 0 { !0 } else { self.cap }
    }

    #[inline(never)]
    #[cold]
    pub fn double(&mut self) {
        unsafe {
            let elem_size = mem::size_of::<T>();

            // since we set the capacity to usize::MAX when elem_size is
            // 0, getting to here necessarily means the RawVec is overfull.
            assert!(elem_size != 0, "capacity overflow");

            let (new_cap, ptr) = if self.cap == 0 {
                // skip to 4 because tiny Vec's are dumb; but not if that would cause overflow
                let new_cap = if elem_size > (!0) / 8 { 1 } else { 4 };
                let ptr = self.alloc.alloc(alloc::Kind::new::<T>().array(new_cap));
                (new_cap, ptr)
            } else {
                // Since we guarantee that we never allocate more than isize::MAX bytes,
                // `elem_size * self.cap <= isize::MAX` as a precondition, so this can't overflow
                let new_cap = 2 * self.cap;
                let new_alloc_size = new_cap * elem_size;
                alloc_guard(new_alloc_size);
                let ptr = self.alloc.realloc(*self.ptr as *mut _,
                                             alloc::Kind::new::<T>().array(self.cap),
                                             new_alloc_size);
                (new_cap, ptr)
            };

            // If allocate or reallocate fail, we'll get `null` back
            if ptr.is_null() { oom() }

            self.ptr = Unique::new(ptr as *mut _);
            self.cap = new_cap;
        }
    }

    pub fn reserve_exact(&mut self, used_cap: usize, needed_extra_cap: usize) {
        unsafe {
            let elem_size = mem::size_of::<T>();

            // NOTE: we don't early branch on ZSTs here because we want this
            // to actually catch "asking for more than usize::MAX" in that case.
            // If we make it past the first branch then we are guaranteed to
            // panic.

            // Don't actually need any more capacity.
            // Wrapping in case they gave a bad `used_cap`.
            if self.cap().wrapping_sub(used_cap) >= needed_extra_cap { return; }

            // Nothing we can really do about these checks :(
            let new_cap = used_cap.checked_add(needed_extra_cap).expect("capacity overflow");
            let new_alloc_size = new_cap.checked_mul(elem_size).expect("capacity overflow");
            alloc_guard(new_alloc_size);

            let ptr = if self.cap == 0 {
                self.alloc.alloc(alloc::Kind::new::<T>().array(new_cap))
            } else {
                self.alloc.realloc(*self.ptr as *mut _,
                                   alloc::Kind::new::<T>().array(self.cap),
                                   new_alloc_size)
            };

            // If allocate or reallocate fail, we'll get `null` back
            if ptr.is_null() { oom() }

            self.ptr = Unique::new(ptr as *mut _);
            self.cap = new_cap;
        }
    }

    pub fn reserve(&mut self, used_cap: usize, needed_extra_cap: usize) {
        unsafe {
            let elem_size = mem::size_of::<T>();

            // NOTE: we don't early branch on ZSTs here because we want this
            // to actually catch "asking for more than usize::MAX" in that case.
            // If we make it past the first branch then we are guaranteed to
            // panic.

            // Don't actually need any more capacity.
            // Wrapping in case they give a bas `used_cap`
            if self.cap().wrapping_sub(used_cap) >= needed_extra_cap { return; }

            // Nothing we can really do about these checks :(
            let new_cap = used_cap.checked_add(needed_extra_cap)
                                  .and_then(|cap| cap.checked_mul(2))
                                  .expect("capacity overflow");
            let new_alloc_size = new_cap.checked_mul(elem_size).expect("capacity overflow");
            // FIXME: may crash and burn on over-reserve
            alloc_guard(new_alloc_size);

            let ptr = if self.cap == 0 {
                self.alloc.alloc(alloc::Kind::new::<T>().array(new_cap))
            } else {
                self.alloc.realloc(*self.ptr as *mut _,
                                   alloc::Kind::new::<T>().array(self.cap),
                                   new_alloc_size)
            };

            // If allocate or reallocate fail, we'll get `null` back
            if ptr.is_null() { oom() }

            self.ptr = Unique::new(ptr as *mut _);
            self.cap = new_cap;
        }
    }

    pub fn shrink_to_fit(&mut self, amount: usize) {
        let elem_size = mem::size_of::<T>();

        // Set the `cap` because they might be about to promote to a `Box<[T]>`
        if elem_size == 0 {
            self.cap = amount;
            return;
        }

        // This check is my waterloo; it's the only thing Vec wouldn't have to do.
        assert!(self.cap >= amount, "Tried to shrink to a larger capacity");

        if amount == 0 {
            let (ptr, cap) = empty();
            self.ptr = ptr;
            self.cap = cap;
        } else if self.cap != amount {
            unsafe {
                // Overflow check is unnecessary as the vector is already at
                // least this large.
                let ptr = self.alloc.realloc(*self.ptr as *mut _,
                                             alloc::Kind::new::<T>().array(self.cap),
                                             amount * elem_size);
                if ptr.is_null() { oom() }
                self.ptr = Unique::new(ptr as *mut _);
            }
            self.cap = amount;
        }
    }

    pub unsafe fn into_box(mut self) -> Box<[T], A> {
        let alloc = mem::replace(&mut self.alloc, mem::uninitialized());
        // NOTE: not calling `cap()` here, actually using the real `cap` field!
        let slice = slice::from_raw_parts_mut(self.ptr(), self.cap);
        let output: Box<[T], A> = Box::from_raw_alloc(slice, alloc);
        mem::forget(self);
        output
    }

    pub fn unsafe_no_drop_flag_needs_drop(&self) -> bool {
        self.cap != mem::POST_DROP_USIZE
    }
}

impl<T, A:Alloc> Drop for RawVec<T, A> {
    /// Frees the memory owned by the RawVec *without* trying to Drop its contents.
    fn drop(&mut self) {
        let elem_size = mem::size_of::<T>();
        if elem_size != 0 && self.cap != 0 && self.unsafe_no_drop_flag_needs_drop() {
            unsafe {
                self.alloc.dealloc(*self.ptr as *mut _,
                                   alloc::Kind::new::<T>().array(self.cap));
            }
        }
    }
}



// We need to guarantee the following:
// * We don't ever allocate `> isize::MAX` byte-size objects
// * We don't overflow `usize::MAX` and actually allocate too little
//
// On 64-bit we just need to check for overflow since trying to allocate
// `> isize::MAX` bytes will surely fail. On 32-bit we need to add an extra
// guard for this in case we're running on a platform which can use all 4GB in
// user-space. e.g. PAE or x32

#[inline]
fn alloc_guard(alloc_size: usize) {
    if usize::BITS < 64 {
        assert!(alloc_size <= isize::MAX as usize, "capacity overflow");
    }
}
