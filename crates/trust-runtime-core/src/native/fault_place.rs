//! Where a native program raised a fault (specification 44 §8.6; specification 34 MEM-05): the
//! site a runtime's fault record keeps, taken from the kernel's record, and the POU, file and line
//! that the image's tables give it when the runtime reports the fault (specification 11, the
//! fault entry).

use crate::fault::FaultSite;
use crate::memory::Generation;
use crate::native::image::{FileRecord, Image, StmtRecord};
use crate::native::memory::pou_symbol;

/// The site of the fault that the kernel recorded with the code `code` at the statement
/// `statement` (specification 44 §8.6), in the program of `image` and generation `generation`:
/// that statement, the POU that holds it and the generation. No fault (code 0), as after a fault
/// from outside the program such as a deadline's, and an image without a statement table, as a
/// release image is (specification 44 §13.2), give [`FaultSite::NONE`].
#[must_use]
pub fn fault_site(
    image: &Image<'_>,
    code: u32,
    statement: u32,
    generation: Generation,
) -> FaultSite {
    if code == 0 {
        return FaultSite::NONE;
    }
    image
        .table::<StmtRecord>()
        .ok()
        .and_then(|statements| statements.get(statement))
        .map_or(FaultSite::NONE, |record| FaultSite {
            pou: u32::from(record.pou),
            statement,
            generation,
        })
}

/// The place of a fault in the program's sources, as specification 11's fault entry names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaultPlace<'a> {
    /// The display name of the POU that holds the statement, as the debugger names it.
    pub pou: &'a str,
    /// The statement's source file, as the image's file table names it: its path relative to the
    /// project.
    pub file: &'a str,
    /// The statement's first line, counted from 1.
    pub line: u32,
}

/// The place of `site` in the program of `image`, which the caller has found to be the site's
/// generation: its POU's display name, the statement's file and its first line. None for
/// [`FaultSite::NONE`], or when a table of the image does not name the statement, its file or its
/// POU.
#[must_use]
pub fn fault_place<'a>(image: &Image<'a>, site: FaultSite) -> Option<FaultPlace<'a>> {
    if site == FaultSite::NONE {
        return None;
    }
    let statement = image.table::<StmtRecord>().ok()?.get(site.statement)?;
    let file = image
        .table::<FileRecord>()
        .ok()?
        .get(u32::from(statement.file))?;
    let (_, symbol) = pou_symbol(image, u32::from(statement.pou))?;
    Some(FaultPlace {
        pou: image.string(symbol.display)?,
        file: image.string(file.label)?,
        line: statement.start_line,
    })
}
