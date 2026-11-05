//! The commands the runtime follows after their first answer, for a confirmation or a remote
//! pulse's end (specification 39, §12.8 item 5): at most [`MAX_FOLLOWED`] at once, each holding a
//! slot until its follow ends, however it ends.

use std::sync::atomic::{AtomicUsize, Ordering};

/// The most commands followed at once for confirmation or a remote pulse's end.
const MAX_FOLLOWED: usize = 64;

/// How many commands are followed now.
static FOLLOWED: AtomicUsize = AtomicUsize::new(0);

/// A followed command's slot, given back when it is dropped: when its follow thread ends, also by a
/// panic, so that no command holds a slot for good.
pub(super) struct FollowSlot {
    followed: &'static AtomicUsize,
}

impl FollowSlot {
    /// A slot, when fewer than [`MAX_FOLLOWED`] commands are followed.
    pub(super) fn take() -> Option<Self> {
        Self::take_from(&FOLLOWED)
    }

    fn take_from(followed: &'static AtomicUsize) -> Option<Self> {
        followed
            .try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_FOLLOWED).then_some(count + 1)
            })
            .ok()
            .map(|_| Self { followed })
    }
}

impl Drop for FollowSlot {
    fn drop(&mut self) {
        self.followed.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    use super::{FollowSlot, MAX_FOLLOWED};

    // Catches checklist G43's leak: a follow thread that panics before it gives its slot back holds
    // it for good, so that once every slot is held no command is followed any more.
    #[test]
    fn a_follow_thread_that_panics_gives_its_slot_back() {
        static FOLLOWED: AtomicUsize = AtomicUsize::new(0);
        let slot = FollowSlot::take_from(&FOLLOWED).expect("a free slot");
        let follow = thread::spawn(move || {
            let _slot = slot;
            panic!("a follow thread fails");
        });
        assert!(follow.join().is_err(), "the follow thread panicked");
        assert_eq!(FOLLOWED.load(Ordering::Acquire), 0);
    }

    // Catches more commands followed at once than the bound, and a slot given back but not reused.
    #[test]
    fn followed_commands_are_bounded_and_a_given_back_slot_is_reused() {
        static FOLLOWED: AtomicUsize = AtomicUsize::new(0);
        let mut slots: Vec<FollowSlot> = (0..MAX_FOLLOWED)
            .map(|_| FollowSlot::take_from(&FOLLOWED).expect("a free slot"))
            .collect();
        assert!(
            FollowSlot::take_from(&FOLLOWED).is_none(),
            "every slot is held"
        );
        drop(slots.pop());
        assert!(
            FollowSlot::take_from(&FOLLOWED).is_some(),
            "a slot was given back"
        );
    }
}
