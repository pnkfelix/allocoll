// A truly simple-minded bump-allocator:

// The allocation area is just a single block of LEN bytes.
//
// It is formatted like so:
// [ <entry-1> <entry-2> ... <entry-k> ... ]
//
// where <entry-k> = [ <value-k> <padding-k> <size-of-k> ]
//
// Invariant: abs(<size-of-k>) = size in bytes of <entry-k>
//
// Allocate of <value-i>: rounds up to min-alignment; if resulting
// size fits, it bumps a cursor, and overwrites <size-of-i>.
// Otherwise returns null.
//
// Deallocation 1. negates <size-of-i>, and 2. if this is the last
// element in the heap, it attempts to roll the cursor backwards.
//
// In other words, this allocator is optimized for roughly stack-like
// allocation patterns, and will outright fail if 
//
//
use alloc;
use alloc::Alloc as AllocTrait;
use super::direct_alloc;

use std::rc::Rc;
use std::cell::Cell;
use std::marker::PhantomData;

const MIN_ALIGN: u32 = 16;
const MAX_LEN: u32 = 4 * 1024 * 1024;

struct AllocState {
    block: *mut u8,
    limit: *mut u8,
    cursor: Cell<*mut u8>,
}

#[derive(Clone)]
pub struct Alloc<'a> {
    state: Rc<AllocState>,
    _a: PhantomData<Fn() -> &'a Alloc<'a>>,
}

impl<'a> Drop for Alloc<'a> {
    fn drop(&mut self) {
        println!("  bump_alloc::Alloc::drop: 0x{:x}", self as *mut Alloc as usize);
    }
}

impl Drop for AllocState {
    fn drop(&mut self) {
        println!("    bump_alloc::AllocState::drop: 0x{:x}", self as *mut _ as usize);
    }
}

impl<'a> Alloc<'a> {
    pub fn new(len: u32) -> Alloc<'a> {
        println!("  bump_alloc::Alloc::new bump len: {:?}", len);
        if len > MAX_LEN {
            panic!("cannot make bump_alloc len={}; max is {}",
                   len, MAX_LEN);
        }

        unsafe {
            let p = *direct_alloc::Alloc.alloc_array::<u8>(len as usize)
                .unwrap();
            Alloc {
                state: Rc::new(AllocState { block: p,
                                            limit: p.offset(len as isize),
                                            cursor: Cell::new(p) }),
                _a: PhantomData,
            }
        }
    }
}

fn roundup_size(size: i32) -> i32 {
    size + (MIN_ALIGN as i32) & !((MIN_ALIGN as i32)-1)
}

impl<'a> alloc::Alloc for Alloc<'a> {
    #[inline]
    unsafe fn alloc(&mut self, kind: alloc::Kind) -> alloc::Address {
        println!("  bump_alloc::Alloc::alloc bump kind: {:?}", kind);
        if kind.align() <= MIN_ALIGN as usize {
            let size = roundup_size((kind.size() + 4) as i32);
            if self.state.cursor.get() < self.state.limit.offset(-size as isize) {
                let p = self.state.cursor.get();
                let n = p.offset(size as isize);
                self.state.cursor.set(n);
                *(n.offset(-4) as *mut i32) = size;
                println!("  alloc bump kind: {:?} => {:p}", kind, p);
                return p;
            }
        }
        let p = direct_alloc::Alloc.alloc(kind); 
        println!("  alloc delg kind: {:?} => {:p}", kind, p);
        return p;
    }

    #[inline]
    unsafe fn dealloc(&mut self, ptr: alloc::Address, kind: alloc::Kind) {
        if kind.align() <= MIN_ALIGN as usize {
            println!("dealloc bump ptr {:p} kind: {:?}", ptr, kind);
            let size = roundup_size((kind.size() + 4) as i32);
            let next = ptr.offset(size as isize);
            let entry_size = next.offset(-4) as *mut i32;
            assert_eq!(size as i32, *entry_size);

            if next != self.state.cursor.get() {
                *entry_size = -size;
                return;
            }

            let start = self.state.block;
            let mut back = ptr;
            loop {
                if back == start { break }
                let prev_size = *(back.offset(-4) as *mut i32);
                if prev_size > 0 {
                    break;
                } else {
                    back = back.offset(prev_size as isize);
                }
            }
            self.state.cursor.set(back);
            return;
        } else {
            println!("dealloc delg ptr {:p} kind: {:?}", ptr, kind);
            return direct_alloc::Alloc.dealloc(ptr, kind);
        }
    }

    unsafe fn realloc(&mut self,
                      ptr: alloc::Address,
                      kind: alloc::Kind,
                      new_size: alloc::Size) -> alloc::Address {
        use alloc::SuperAlloc;
        SuperAlloc::realloc(self, ptr, kind, new_size)
    }
}

