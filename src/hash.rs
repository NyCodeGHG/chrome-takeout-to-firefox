use anyhow::Context;

pub fn hash(url: &str) -> anyhow::Result<u64> {
    let prefix = url.find(':').context("URL is missing the protocol.")?;
    let result = ((hash_simple(&url[0..prefix]) & 0x0000FFFF) << 32) + hash_simple(url);
    Ok(result)
}

const GOLDEN_RATIO: u32 = 0x9E3779B9;

fn hash_simple(text: &str) -> u64 {
    let mut hash = 0;
    for value in text.bytes() {
        hash = GOLDEN_RATIO.wrapping_mul(u32::rotate_left(hash, 5) ^ (value as u32));
    }
    hash as u64
}

#[cfg(test)]
mod tests {
    use super::hash;

    #[test]
    fn test_hash() {
        let pairs = &[
            ("https://vault.bitwarden.com/", 47358609224710),
            ("https://search.nixos.org/", 47360563686504),
            ("https://wiki.archlinux.org/", 47356434076161),
            ("javascript:(function()%7Bwindow.location%20%3D%20%60https%3A%2F%2Felk.zone%2F%24%7Bwindow.location.toString()%7D%60%3B%7D)()%3B", 61198099442140),
            ("https://www.mozilla.org/about/", 47357608426557),
        ];
        for (url, result) in pairs {
            assert_eq!(hash(url).unwrap(), *result);
        }
    }
}
