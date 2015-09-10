use alloc::Alloc as AllocTrait;

mod direct_alloc;

mod bump_alloc;

use boxing::Boxing;

#[cfg(not_now)]
#[test]
fn demo_direct_in_place() {
    let std = direct_alloc::Alloc;
    let b = in Boxing(std) { 3 };
    println!("at end of demo_direct b: {:?}", b);}

#[cfg(not_now)]
#[test]
fn demo_bump_calls() {
    use std::ptr::Unique;
    let mut bmp = bump_alloc::Alloc::new(4*1024*1024);
    let p: Unique<u32>;
    unsafe {
        p = bmp.alloc_one().unwrap();
        **p = 3;
        println!("at end of demo_bump_calls *p: {:?}", *p);
        bmp.dealloc_one(p);
    }
}

#[test]
fn demo_bump_in_place() {
    let bmp = bump_alloc::Alloc::new(4*1024*1024);
    let b = in Boxing(bmp) { 3 };
    println!("at end of demo_direct b: {:?}", b);
    // ::std::mem::forget(b);
}
