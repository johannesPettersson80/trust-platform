//! The cable between a chip's network and a client in these tests: two queues of Ethernet frames,
//! each end a smoltcp device.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use trust_platform_net::smoltcp::phy::{self, Device, DeviceCapabilities, Medium};
use trust_platform_net::smoltcp::time::Instant;

type Queue = Rc<RefCell<VecDeque<Vec<u8>>>>;

/// One end of the cable: the frames that arrive at it, and the other end's.
pub struct End {
    arriving: Queue,
    leaving: Queue,
}

pub fn cable() -> (End, End) {
    let one = Queue::default();
    let other = Queue::default();
    (
        End {
            arriving: Rc::clone(&one),
            leaving: Rc::clone(&other),
        },
        End {
            arriving: other,
            leaving: one,
        },
    )
}

impl Device for End {
    type RxToken<'a>
        = Arrived
    where
        Self: 'a;
    type TxToken<'a>
        = Leaving
    where
        Self: 'a;

    fn receive(&mut self, _: Instant) -> Option<(Arrived, Leaving)> {
        let frame = self.arriving.borrow_mut().pop_front()?;
        Some((Arrived(frame), Leaving(Rc::clone(&self.leaving))))
    }

    fn transmit(&mut self, _: Instant) -> Option<Leaving> {
        Some(Leaving(Rc::clone(&self.leaving)))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut capabilities = DeviceCapabilities::default();
        capabilities.medium = Medium::Ethernet;
        capabilities.max_transmission_unit = 1514;
        capabilities
    }
}

pub struct Arrived(Vec<u8>);

impl phy::RxToken for Arrived {
    fn consume<R, F: FnOnce(&[u8]) -> R>(self, read: F) -> R {
        read(&self.0)
    }
}

pub struct Leaving(Queue);

impl phy::TxToken for Leaving {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, write: F) -> R {
        let mut frame = vec![0; len];
        let result = write(&mut frame);
        self.0.borrow_mut().push_back(frame);
        result
    }
}
