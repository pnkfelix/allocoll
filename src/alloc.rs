use std::cmp;
use std::mem;
use std::ptr::{self, Unique};

use alloc_crate::heap;

pub type Size = usize;
pub type Capacity = usize;
pub type Alignment = usize;

pub unsafe trait Raw { }
unsafe impl Raw for .. { }
pub type Address = *mut u8;
pub struct Excess(Address, Capacity);

/// Category for a memory record.
///
/// An instance of `Kind` describes a particular layout of memory.
/// You build a `Kind` up as an input to give to an allocator.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Kind {
    size: Size,
    align: Alignment,
}

fn size_align<T>() -> (usize, usize) {
    (mem::size_of::<T>(), mem::align_of::<T>())
}

// Accessor methods
impl Kind {
    pub fn size(&self) -> usize { self.size }

    pub fn align(&self) -> usize { self.align }
}


// private methods
impl Kind {
    /// This constructor can only be used within the standard library,
    /// for e.g. the default standard allocator which knows how to
    /// deal with non-`Raw` types (in terms of registering them within
    /// the Gc when necessary).
    fn new_internal<T>() -> Kind {
        let (size, align) = size_align::<T>();
        Kind { size: size, align: align }
    }

    pub unsafe fn from_size_align(size: usize, align: usize) -> Kind {
        Kind { size: size, align: align }
    }

}

// public constructor methods
impl Kind {
    /// Creates a `Kind` describing the record for a single instance of `T`.
    pub fn new<T>() -> Kind {
        Kind::new_internal::<T>()
    }

    /// Creates a `Kind` describing the record for `self` followed by
    /// `next` with no additional padding between the two. Since no
    /// padding is inserted, the alignment of `next` is irrelevant,
    /// and is not incoporated *at all* into the resulting `Kind`.
    ///
    /// Returns `(k, offset)`, where `k` is kind of the concatenated
    /// record and `offset` is the start of the `next` embedded witnin
    /// the concatenated record (assuming that the record itself
    /// starts at offset 0).
    ///
    /// (The `offset` is always the same as `self.size()`; we use this
    ///  signature out of convenience in matching the signature of
    ///  `Kind::extend`.)
    pub fn extend_packed(self, next: Kind) -> (Kind, usize) {
        let new_size = self.size + next.size;
        (Kind { size: new_size, ..self }, self.size)
    }

    /// Creates a `Kind` describing the record that can hold a value
    /// of the same kind as `self`, but that also is aligned to
    /// alignment `align`.
    ///
    /// If `self` already meets the prescribed alignment, then returns
    /// `self`.
    ///
    /// Note that this method does not add any padding to the overall
    /// size, regardless of whether the returned kind has a different
    /// alignment. You should be able to get that effect by passing
    /// an appropriately aligned zero-sized type to `Kind::extend`.
    pub fn align_to(self, align: usize) -> Kind {
        if align > self.align {
            Kind { align: align, ..self }
        } else {
            self
        }
    }

    /// Returns the amount of padding we must insert after `self`
    /// to ensure that the following address will satisfy `align`.
    ///
    /// Note that for this to make sense, `align <= self.align`
    /// otherwise, the amount of inserted padding would need to depend
    /// on the particular starting address for the whole record.
    ///
    /// (Also, as usual, both alignments must be a power of two);
    fn pad_to(self, align: usize) -> usize {
        debug_assert!(align <= self.align);
        let len = self.size;
        let len_rounded_up = (len + align - 1) & !(align - 1);
        return len_rounded_up - len;
    }

    /// Creates a `Kind` describing the record for `self` followed by
    /// `next`, including any necessary padding to ensure that `next`
    /// will be properly aligned. Note that the result `Kind` will
    /// satisfy the alignment properties of both `self` and `next`.
    ///
    /// Returns `(k, offset)`, where `k` is kind of the concatenated
    /// record and `offset` is the start of the `next` embedded witnin
    /// the concatenated record (assuming that the record itself
    /// starts at offset 0).
    pub fn extend(self, next: Kind) -> (Kind, usize) {
        let new_align = cmp::max(self.align, next.align);
        let realigned = Kind { align: new_align, ..self };
        let pad = realigned.pad_to(new_align);
        let offset = self.size + pad;
        let new_size = offset + next.size;
        (Kind { size: new_size, align: new_align }, offset)
    }

    /// Creates a `Kind` describing the record for `n` instances of
    /// `self`, with a suitable amount of padding between each.
    pub fn array(self, n: usize) -> Kind {
        let padded_size = self.size + self.pad_to(self.align);
        Kind { size: padded_size * n, align: self.align }
    }

    /// Creates a `Kind` describing the record for `n` instances of
    /// `self`, with no padding between each.
    pub fn array_packed(self, n: usize) -> Kind {
        Kind { size: self.size * n, align: self.align }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct AllocError;

// See https://github.com/pnkfelix/rfcs/blob/fsk-allocator-rfc/active/0000-allocator.md
// for tons of documentation for the old API.
pub trait Alloc {
    /// Any activity done by the `oom` method must not allocate
    /// from `self` (otherwise you essentially infinite regress).
    unsafe fn oom(&mut self) -> ! { ::std::intrinsics::abort() }

    unsafe fn alloc(&mut self, kind: Kind) -> Address;
    unsafe fn dealloc(&mut self, ptr: Address, kind: Kind);

    unsafe fn usable_size(&self, kind: Kind) -> Capacity {
        SuperAlloc::usable_size(self, kind)
    }

    unsafe fn alloc_one<T:Raw>(&mut self) -> Result<Unique<T>, AllocError> {
        SuperAlloc::alloc_one(self)
    }

    unsafe fn dealloc_one<T:Raw>(&mut self, ptr: Unique<T>) {
        SuperAlloc::dealloc_one(self, ptr)
    }

    unsafe fn alloc_array<T:Raw>(&mut self, n: usize) -> Result<Unique<T>, AllocError> {
        SuperAlloc::alloc_array(self, n)
    }

    unsafe fn alloc_excess(&mut self, kind: Kind) -> Excess {
        SuperAlloc::alloc_excess(self, kind)
    }

    unsafe fn realloc(&mut self, ptr: Address, kind: Kind, new_size: Size) -> Address {
        SuperAlloc::realloc(self, ptr, kind, new_size)
    }

    unsafe fn realloc_excess(&mut self, ptr: Address, kind: Kind, new_size: Size) -> Excess {
        SuperAlloc::realloc_excess(self, ptr, kind, new_size)
    }
}

pub trait SuperAlloc {
    unsafe fn usable_size(&self, kind: Kind) -> Capacity;
    unsafe fn alloc_one<T:Raw>(&mut self) -> Result<Unique<T>, AllocError>;
    unsafe fn dealloc_one<T:Raw>(&mut self, mut ptr: Unique<T>);
    unsafe fn alloc_array<T:Raw>(&mut self, n: usize) -> Result<Unique<T>, AllocError>;
    unsafe fn alloc_excess(&mut self, kind: Kind) -> Excess;
    unsafe fn realloc(&mut self, ptr: Address, kind: Kind, new_size: Size) -> Address;
    unsafe fn realloc_excess(&mut self, ptr: Address, kind: Kind, new_size: Size) -> Excess;
}

impl<Self_:?Sized + Alloc> SuperAlloc for Self_ {
    unsafe fn usable_size(&self, kind: Kind) -> Capacity {
        kind.size
    }

    unsafe fn alloc_one<T:Raw>(&mut self) -> Result<Unique<T>, AllocError> {
        let p = self.alloc(Kind::new::<T>()) as *mut T;
        if !p.is_null() { Ok(Unique::new(p)) } else { Err(AllocError) }
    }

    unsafe fn dealloc_one<T:Raw>(&mut self, mut ptr: Unique<T>) {
        self.dealloc(ptr.get_mut() as *mut T as *mut u8, Kind::new::<T>());
    }

    unsafe fn alloc_array<T:Raw>(&mut self, n: usize) -> Result<Unique<T>, AllocError> {
        let p = self.alloc(Kind::new::<T>().array(n)) as *mut T;
        if !p.is_null() { Ok(Unique::new(p)) } else { Err(AllocError) }
    }

    unsafe fn alloc_excess(&mut self, kind: Kind) -> Excess {
        Excess(self.alloc(kind), self.usable_size(kind))
    }

    unsafe fn realloc(&mut self, ptr: Address, kind: Kind, new_size: Size) -> Address {
        if new_size <= self.usable_size(kind) {
            return ptr;
        } else {
            let new_ptr = self.alloc(Kind { size: new_size, ..kind });
            if !new_ptr.is_null() {
                ptr::copy(ptr as *const u8, new_ptr, cmp::min(kind.size, new_size));
                self.dealloc(ptr, kind);
            }
            return new_ptr;
        }
    }

    unsafe fn realloc_excess(&mut self, ptr: Address, kind: Kind, new_size: Size) -> Excess {
        Excess(self.realloc(ptr, kind, new_size),
               self.usable_size(Kind { size: new_size, ..kind }))
    }

}

#[derive(Copy, Clone, Debug)]
pub struct DefaultAlloc;

impl Default for DefaultAlloc {
    fn default() -> Self { DefaultAlloc }
}

impl Alloc for DefaultAlloc {
    unsafe fn alloc(&mut self, kind: Kind) -> Address {
        if kind.size == 0 {
            heap::EMPTY as *mut u8
        } else {
            heap::allocate(kind.size, kind.align)
        }
    }

    unsafe fn realloc(&mut self, ptr: Address, kind: Kind, new_size: Size) -> Address {
        heap::reallocate(ptr, kind.size, new_size, kind.align)
    }

    unsafe fn dealloc(&mut self, ptr: Address, kind: Kind) {
        heap::deallocate(ptr, kind.size, kind.align)
    }
}
