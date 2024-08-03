use std::env;
use mockall::automock;
use crate::config::{OneOrManyPlatforms, Platform};

pub fn current_platform_provider() -> Box<dyn PlatformProvider> {
    return Box::new(RealPlatformProvider{});
}

pub fn is_current_platform(platform_provider: &Box<dyn PlatformProvider>, platform_or_platforms: &OneOrManyPlatforms) -> bool {

    let current_platform = platform_provider.get_platform();

    match platform_or_platforms {
        OneOrManyPlatforms::One(platform) => platform.platform == current_platform,
        OneOrManyPlatforms::Many(platforms) => platforms.platforms.contains(&current_platform),
    }
}

#[automock]
pub trait PlatformProvider {
    fn get_platform(&self) -> Platform;
}

struct RealPlatformProvider;
impl PlatformProvider for RealPlatformProvider {
    fn get_platform(&self) -> Platform {
        match env::consts::OS {
            "linux" => Platform::Linux,
            "macos" => Platform::MacOS,
            "windows" => Platform::Windows,
            platform => panic!("unknown platform: {}", platform)
        }
    }
}