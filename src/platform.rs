use std::env;

use crate::config::{OneOrManyPlatforms, Platform};

pub fn is_current_platform(platform_or_platforms: &OneOrManyPlatforms) -> bool {

    let current_platform = get_platform();

    match platform_or_platforms {
        OneOrManyPlatforms::One(platform) => platform.platform == current_platform,
        OneOrManyPlatforms::Many(platforms) => platforms.platforms.contains(&current_platform),
    }
}

fn get_platform() -> Platform {
    match env::consts::OS {
        "linux" => Platform::Linux,
        "macos" => Platform::MacOS,
        "windows" => Platform::Windows,
        platform => panic!("unknown platform: {}", platform)
    }
}
