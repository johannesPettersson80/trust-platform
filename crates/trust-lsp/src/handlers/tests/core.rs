use super::*;
use std::path::Path;

fn normalize_path_for_assert(path: &Path) -> String {
    #[cfg(windows)]
    {
        path.to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches("//?/")
            .to_ascii_lowercase()
    }
    #[cfg(not(windows))]
    {
        path.to_string_lossy().into_owned()
    }
}

#[path = "core_part_01.rs"]
mod core_part_01;
#[path = "core_part_02.rs"]
mod core_part_02;
#[path = "core_part_03.rs"]
mod core_part_03;
#[path = "core_part_04.rs"]
mod core_part_04;
#[path = "core_part_05.rs"]
mod core_part_05;
#[path = "core_part_06.rs"]
mod core_part_06;
#[path = "core_part_07.rs"]
mod core_part_07;
#[path = "core_part_08.rs"]
mod core_part_08;
#[path = "core_part_09.rs"]
mod core_part_09;
#[path = "core_part_10.rs"]
mod core_part_10;
#[path = "core_part_11.rs"]
mod core_part_11;
#[path = "core_part_12.rs"]
mod core_part_12;
#[allow(unused_imports)]
use core_part_01::*;
#[allow(unused_imports)]
use core_part_02::*;
#[allow(unused_imports)]
use core_part_03::*;
#[allow(unused_imports)]
use core_part_04::*;
#[allow(unused_imports)]
use core_part_05::*;
#[allow(unused_imports)]
use core_part_06::*;
#[allow(unused_imports)]
use core_part_07::*;
#[allow(unused_imports)]
use core_part_08::*;
#[allow(unused_imports)]
use core_part_09::*;
#[allow(unused_imports)]
use core_part_10::*;
#[allow(unused_imports)]
use core_part_11::*;
#[allow(unused_imports)]
use core_part_12::*;
