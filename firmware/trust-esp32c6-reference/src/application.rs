//! The installed `TRSTA001` application: read the container's header from the `stbc` partition,
//! copy only its total length into the heap, and admit it against the C6's descriptor before any
//! state exists (design §1.1 and §1.2, specification 34 §5.1).
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use trust_platform_esp32::{profile::DEVICE_DESCRIPTOR, Board};
use trust_runtime_services::installation::{admit, Admitted, MAGIC};

/// Offset of the total length in the container header (design §1.2).
const TOTAL_LENGTH_OFFSET: usize = 12;
/// The header prefix the reader needs: the magic and the total length.
const HEADER_BYTES: usize = 16;

/// Read the installed application and admit it. The container is copied into the heap, so a
/// length above the heap's room is refused with its numbers rather than exhausting the heap
/// (design §1.1, R1). The copied container is leaked: the program and its record live for the
/// rest of the boot.
///
/// # Errors
///
/// The refusal's text: the region cannot be read or holds no container, its header names a length
/// above the heap's room, or admission refused the deployment.
pub fn read_and_admit(board: &mut Board) -> Result<Admitted<'static>, String> {
    let mut header = [0u8; HEADER_BYTES];
    board.read_application(&mut header).map_err(|_| {
        String::from("no installed application: the application region cannot be read")
    })?;
    // The magic is checked first, so a partial write reads as no installed program (DEPLOY-03).
    if !header.starts_with(&MAGIC) {
        return Err(String::from(
            "no installed application: no TRSTA001 container in the region",
        ));
    }
    let total = u32::from_le_bytes([
        header[TOTAL_LENGTH_OFFSET],
        header[TOTAL_LENGTH_OFFSET + 1],
        header[TOTAL_LENGTH_OFFSET + 2],
        header[TOTAL_LENGTH_OFFSET + 3],
    ]) as usize;
    let room = crate::memory::HEAP.remaining();
    if total > room {
        return Err(format!(
            "the installed container needs {total} bytes, more than the heap's {room} bytes"
        ));
    }
    let mut container = vec![0u8; total];
    board.read_application(&mut container).map_err(|_| {
        String::from("no installed application: the application region cannot be read")
    })?;
    let container: &'static [u8] = Box::leak(container.into_boxed_slice());
    admit(container, &DEVICE_DESCRIPTOR).map_err(|error| error.to_string())
}
