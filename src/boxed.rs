use std::fmt;
use std::intrinsics;
use std::ops::{Deref, DerefMut};
use std::mem;
use std::ptr::{Unique};

use alloc::{Alloc, DefaultAlloc, Kind};

// FIXME: Generalize to support `T: ?Sized`
// (This is hard because I do not yet know how to call the
// appropriate destructor for the underlying data.)

pub struct Box<T: ?Sized, A:Alloc = DefaultAlloc> {
    value: Unique<T>,
    alloc: A,
}

impl<T: ?Sized, A:Alloc> Deref for Box<T, A> {
    type Target = T;

    fn deref(&self) -> &T { unsafe { &**self.value } }
}

impl<T: ?Sized, A:Alloc> DerefMut for Box<T, A> {
    fn deref_mut(&mut self) -> &mut T { unsafe { &mut **self.value } }
}

impl<T: ?Sized, A:Alloc> Box<T, A> {
    pub fn value_alloc(mut self) -> (Unique<T>, A) {
        unsafe {
            let v = mem::replace(&mut self.value, mem::uninitialized());
            let a = mem::replace(&mut self.alloc, mem::uninitialized());
            mem::forget(self);
            (v, a)
        }
    }
    pub unsafe fn from_raw_alloc(raw: *mut T, alloc: A) -> Self {
        Box { value: mem::transmute(raw), alloc: alloc }
    }
}

impl<T: ?Sized, A:Alloc> Drop for Box<T, A> {
    fn drop(&mut self) {
        unsafe {
            println!("starting boxed::Box::drop for 0x{:x}", self as *mut _ as usize);
            intrinsics::drop_in_place(&**self.value as *const T as *mut T);
            let k = Kind::for_value(self.value.get());
            let mut a = mem::replace(&mut self.alloc, mem::dropped());
            a.dealloc(*self.value as *mut u8, k);
            drop(a);
            println!("finished boxed::Box::drop");
        }
    }
}

impl<T: fmt::Display + ?Sized, A:Alloc> fmt::Display for Box<T, A> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: fmt::Debug + ?Sized, A:Alloc> fmt::Debug for Box<T, A> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T, A:Alloc> fmt::Pointer for Box<T, A> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // It's not possible to extract the inner Uniq directly from the Box,
        // instead we cast it to a *const which aliases the Unique
        let ptr: *const T = &**self;
        fmt::Pointer::fmt(&ptr, f)
    }
}
