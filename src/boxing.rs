use alloc::{Alloc, Kind};
use boxed::Box;

use std::mem;
use std::ops::{Place, Placer, InPlace};

pub struct Boxing<A:Alloc>(pub A);

impl<T, A:Alloc> Placer<T> for Boxing<A> {
    type Place = InterimBox<T, A>;
    fn make_place(mut self) -> InterimBox<T, A> {
        // println!("start of <Boxing as Placer>::make_place");
        let ret = unsafe {
            InterimBox {
                p: self.0.alloc(Kind::new::<T>()) as *mut T,
                a: self.0
            }
        };
        // println!("at end of <Boxing as Placer>::make_place");
        ret
    }
}

pub struct InterimBox<T, A> {
    p: *mut T,
    a: A,
}

impl<T, A> Place<T> for InterimBox<T, A> {
    fn pointer(&mut self) -> *mut T { self.p }
}

impl<T, A: Alloc> InPlace<T> for InterimBox<T, A> {
    type Owner = Box<T, A>;
    unsafe fn finalize(mut self) -> Box<T, A> {
        println!("start of InterimBox::finalize");
        let p = mem::replace(&mut self.p, mem::uninitialized());
        let a = mem::replace(&mut self.a, mem::uninitialized());
        mem::forget(self);
        let ret = Box::from_raw_alloc(p, a);
        println!("at end of InterimBox::finalize");
        ret
    }
}
