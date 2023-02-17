#![cfg_attr(feature = "alloc_trait", feature(allocator_api))]

#[test]
#[cfg(feature = "alloc_trait")]
fn grow_in_place() {
    use std::{
        alloc::{Allocator, Layout},
        ptr::NonNull,
    };

    unsafe {
        let allocator = tikv_jemallocator::Jemalloc;

        // allocate 7 bytes which end up in the 8 byte size-class as long as
        // jemalloc's default size classes are used:
        let orig_sz = 7;
        let orig_l = Layout::from_size_align(orig_sz, 1).unwrap();
        let mut res = allocator.allocate(orig_l).unwrap();
        let ptr = NonNull::new(res.as_mut().as_mut_ptr()).unwrap();

        // try to grow it in place by 1 byte - it should grow without problems:
        let new_l = Layout::from_size_align(orig_sz + 1, 1).unwrap();
        let mut res = allocator.grow(ptr, orig_l, new_l).unwrap();
        let new_ptr = NonNull::new(res.as_mut().as_mut_ptr()).unwrap();
        assert_eq!(new_ptr, ptr);

        // trying to do it again fails because it would require moving the
        // allocation to a different size class which jemalloc's xallocx does not
        // do:
        let new_l2 = Layout::from_size_align(orig_sz + 2, 1).unwrap();
        let mut res = allocator.grow(new_ptr, new_l, new_l2).unwrap();
        let new_ptr2 = NonNull::new(res.as_mut().as_mut_ptr()).unwrap();
        assert_ne!(new_ptr2, new_ptr);

        allocator.deallocate(new_ptr2, new_l2)
    }
}

#[test]
#[cfg(feature = "alloc_trait")]
fn shrink_in_place() {
    use std::{
        alloc::{Allocator, Layout},
        ptr::NonNull,
    };

    unsafe {
        let allocator = tikv_jemallocator::Jemalloc;

        // allocate a "large" block of memory:
        let orig_sz = 10 * 4096;
        let orig_l = Layout::from_size_align(orig_sz, 1).unwrap();
        let mut res = allocator.allocate(orig_l).unwrap();
        let ptr = NonNull::new(res.as_mut().as_mut_ptr()).unwrap();

        let new_l = Layout::from_size_align(orig_sz - 1, 1).unwrap();
        let mut res = allocator.shrink(ptr, orig_l, new_l).unwrap();
        let new_ptr = NonNull::new(res.as_mut().as_mut_ptr()).unwrap();
        assert_eq!(new_ptr, ptr);

        // try to shrink it in place to 1 byte - if this succeeds,
        // the size-class of the new allocation should be different
        // than that of the original allocation:
        let new_l2 = Layout::from_size_align(1, 1).unwrap();
        let mut res = allocator.shrink(new_ptr, new_l, new_l2).unwrap();
        let new_ptr2 = NonNull::new(res.as_mut().as_mut_ptr()).unwrap();
        assert_ne!(new_ptr, new_ptr2);
        allocator.deallocate(new_ptr2, new_l2);
    }
}

#[test]
#[cfg(feature = "alloc_trait")]
fn basic() {
    use std::{
        alloc::{Allocator, Layout},
        cmp,
        ptr::{self, NonNull},
    };

    unsafe {
        let allocator = tikv_jemallocator::Jemalloc;

        let aligns = [1, 2, 4, 8, 16, 32];
        let mut sizes: Vec<usize> = (0..64).collect();
        sizes.extend([1023, 1024, 4095, 4096]);

        for origin_size in &sizes {
            for origin_align in &aligns {
                for new_size in &sizes {
                    for new_align in &aligns {
                        let tag = || {
                            format!(
                                "origin_size: {}, origin_align: {}, new_size: {}, new_align: {}",
                                origin_size, origin_align, new_size, new_align
                            )
                        };
                        let origin_l =
                            Layout::from_size_align(*origin_size, *origin_align).unwrap();
                        let mut res = allocator.allocate(origin_l).unwrap();
                        assert_eq!(res.as_ref().len(), *origin_size, "{}", tag());
                        assert_eq!(
                            res.as_ref().as_ptr().align_offset(*origin_align),
                            0,
                            "{}",
                            tag()
                        );
                        let origin_ptr = NonNull::new(res.as_mut().as_mut_ptr()).unwrap();

                        ptr::write_bytes(origin_ptr.as_ptr(), 0x42, *origin_size);

                        let new_l = Layout::from_size_align(*new_size, *new_align).unwrap();
                        let mut res = if new_size > origin_size {
                            allocator.grow(origin_ptr, origin_l, new_l).unwrap()
                        } else {
                            allocator.shrink(origin_ptr, origin_l, new_l).unwrap()
                        };

                        assert_eq!(res.as_ref().len(), *new_size, "{}", tag());
                        assert_eq!(
                            res.as_ref().as_ptr().align_offset(*new_align),
                            0,
                            "{}",
                            tag()
                        );
                        assert!(
                            res.as_ref()
                                .iter()
                                .take(cmp::min(*origin_size, *new_size))
                                .all(|x| *x == 0x42),
                            "{}",
                            tag()
                        );

                        let new_ptr = NonNull::new(res.as_mut().as_mut_ptr()).unwrap();
                        allocator.deallocate(new_ptr, new_l);

                        // test zeroed
                        let mut res = allocator.allocate_zeroed(origin_l).unwrap();
                        assert_eq!(res.as_ref().len(), *origin_size, "{}", tag());
                        assert_eq!(
                            res.as_ref().as_ptr().align_offset(*origin_align),
                            0,
                            "{}",
                            tag()
                        );
                        assert!(res.as_ref().iter().all(|&x| x == 0), "{}", tag());
                        let origin_ptr = NonNull::new(res.as_mut().as_mut_ptr()).unwrap();
                        ptr::write_bytes(origin_ptr.as_ptr(), 0x42, *origin_size);

                        let new_l = Layout::from_size_align(*new_size, *new_align).unwrap();
                        let mut res = if new_size > origin_size {
                            allocator.grow_zeroed(origin_ptr, origin_l, new_l).unwrap()
                        } else {
                            allocator.shrink(origin_ptr, origin_l, new_l).unwrap()
                        };
                        assert_eq!(res.as_ref().len(), *new_size, "{}", tag());
                        assert_eq!(
                            res.as_ref().as_ptr().align_offset(*new_align),
                            0,
                            "{}",
                            tag()
                        );
                        assert!(
                            res.as_ref()
                                .iter()
                                .take(cmp::min(*origin_size, *new_size))
                                .all(|x| *x == 0x42),
                            "{}",
                            tag()
                        );
                        assert!(
                            res.as_ref()
                                .iter()
                                .skip(cmp::min(*origin_size, *new_size))
                                .all(|x| *x == 0),
                            "{}",
                            tag()
                        );

                        let new_ptr = NonNull::new(res.as_mut().as_mut_ptr()).unwrap();
                        allocator.deallocate(new_ptr, new_l);
                    }
                }
            }
        }
    }
}
