use std::cmp::Ordering;
use std::fmt::{Display, Formatter};
use std::marker::PhantomData;
use std::str::FromStr;

use anyhow::anyhow;

/// The api version, the generic CR here to prevent from comparing `ApiVersion<CR1>` to `ApiVersion<CR2>`
#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum ApiVersion<CR> {
    /// alpha version e.g. v1alpha1 => Alpha(1, 1)
    Alpha(u32, u32, PhantomData<CR>),
    /// beta version e.g. v2beta3 => Beta(2, 3)
    Beta(u32, u32, PhantomData<CR>),
    /// stable version e.g v3 => Stable(3)
    Stable(u32, PhantomData<CR>),
}

impl<CR> ApiVersion<CR> {
    /// Create alpha version
    #[must_use]
    #[inline]
    pub fn alpha(main: u32, sub: u32) -> Self {
        Self::Alpha(main, sub, PhantomData)
    }

    /// Create beta version
    #[must_use]
    #[inline]
    pub fn beta(main: u32, sub: u32) -> Self {
        Self::Beta(main, sub, PhantomData)
    }

    /// Create beta version
    #[must_use]
    #[inline]
    pub fn stable(main: u32) -> Self {
        Self::Stable(main, PhantomData)
    }

    /// return the main version
    fn main_version(&self) -> u32 {
        match *self {
            ApiVersion::Stable(main, _)
            | ApiVersion::Beta(main, _, _)
            | ApiVersion::Alpha(main, _, _) => main,
        }
    }

    /// return the sub version
    fn sub_version(&self) -> u32 {
        match *self {
            ApiVersion::Stable(_, _) => 0,
            ApiVersion::Beta(_, sub, _) | ApiVersion::Alpha(_, sub, _) => sub,
        }
    }
}

impl<CR> FromStr for ApiVersion<CR> {
    type Err = anyhow::Error;

    #[inline]
    #[allow(clippy::indexing_slicing)] // it is obvious that a string start with 'v' has the index 0
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with('v') {
            return Err(anyhow!(
                "invalid api version format, version does not start with 'v'"
            ));
        }
        if s.contains("alpha") {
            let parts: Vec<_> = s[1..].split("alpha").collect();
            if parts.len() != 2 {
                return Err(anyhow!("invalid api version format"));
            }
            let main_ver = parts[0].parse()?;
            if parts[1].is_empty() {
                return Ok(Self::Alpha(main_ver, 0, PhantomData));
            }
            let sub_ver = parts[1].parse()?;
            return Ok(Self::Alpha(main_ver, sub_ver, PhantomData));
        }
        if s.contains("beta") {
            let parts: Vec<_> = s[1..].split("beta").collect();
            if parts.len() != 2 {
                return Err(anyhow!("invalid api version format"));
            }
            let main_ver = parts[0].parse()?;
            if parts[1].is_empty() {
                return Ok(Self::Beta(main_ver, 0, PhantomData));
            }
            let sub_ver = parts[1].parse()?;
            return Ok(Self::Beta(main_ver, sub_ver, PhantomData));
        }
        let main = s[1..].parse()?;
        Ok(Self::Stable(main, PhantomData))
    }
}

impl<CR> Display for ApiVersion<CR> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let version = match *self {
            ApiVersion::Alpha(main, sub, _) => {
                if sub > 0 {
                    format!("v{main}alpha{sub}")
                } else {
                    format!("v{main}alpha")
                }
            }
            ApiVersion::Beta(main, sub, _) => {
                if sub > 0 {
                    format!("v{main}beta{sub}")
                } else {
                    format!("v{main}beta")
                }
            }
            ApiVersion::Stable(main, _) => format!("v{main}"),
        };
        write!(f, "{version}")
    }
}

impl<CR> PartialEq<Self> for ApiVersion<CR> {
    #[inline]
    #[allow(clippy::pattern_type_mismatch)]
    fn eq(&self, other: &Self) -> bool {
        let kind = matches!(
            (self, other),
            (Self::Alpha(_, _, _), Self::Alpha(_, _, _))
                | (Self::Beta(_, _, _), Self::Beta(_, _, _))
                | (Self::Stable(_, _), Self::Stable(_, _))
        );
        kind && self.main_version() == other.main_version()
            && self.sub_version() == other.sub_version()
    }
}

impl<CR> PartialOrd for ApiVersion<CR> {
    #[inline]
    #[allow(clippy::pattern_type_mismatch)]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // comparing order
        // main version
        // alpha < beta < stable
        // sub version
        if self.main_version() != other.main_version() {
            return self.main_version().partial_cmp(&other.main_version());
        }
        match (self, other) {
            (Self::Beta(_, sub, _), Self::Beta(_, other_sub, _))
            | (Self::Alpha(_, sub, _), Self::Alpha(_, other_sub, _)) => sub.partial_cmp(other_sub),
            (Self::Alpha(_, _, _) | Self::Beta(_, _, _), Self::Stable(_, _))
            | (Self::Alpha(_, _, _), Self::Beta(_, _, _)) => Some(Ordering::Less),
            (Self::Stable(_, _) | Self::Beta(_, _, _), Self::Alpha(_, _, _))
            | (Self::Stable(_, _), Self::Beta(_, _, _)) => Some(Ordering::Greater),
            _ => Some(Ordering::Equal),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::migration::ApiVersion;
    use std::cmp::Ordering;
    use std::marker::PhantomData;

    #[test]
    #[allow(clippy::similar_names)]
    fn test_api_version() {
        type CR = ();

        let test_cases = [
            ("v1alpha", ApiVersion::Alpha(1, 0, PhantomData::<CR>)),
            ("v10alpha", ApiVersion::Alpha(10, 0, PhantomData)),
            ("v1beta", ApiVersion::Beta(1, 0, PhantomData)),
            ("v10beta", ApiVersion::Beta(10, 0, PhantomData)),
            ("v1alpha1", ApiVersion::Alpha(1, 1, PhantomData)),
            ("v10alpha1", ApiVersion::Alpha(10, 1, PhantomData)),
            ("v10alpha10", ApiVersion::Alpha(10, 10, PhantomData)),
            ("v1beta1", ApiVersion::Beta(1, 1, PhantomData)),
            ("v1beta10", ApiVersion::Beta(1, 10, PhantomData)),
            ("v10beta10", ApiVersion::Beta(10, 10, PhantomData)),
            ("v1", ApiVersion::Stable(1, PhantomData)),
            ("v10", ApiVersion::Stable(10, PhantomData)),
        ];
        for (raw, ver) in test_cases {
            let parsed: ApiVersion<_> = raw.parse().unwrap();
            assert_eq!(parsed, ver);
            assert_eq!(parsed.to_string(), raw);
        }
        let test_cases = [
            ("v1alpha", "v1alpha", Ordering::Equal),
            ("v1beta", "v1beta", Ordering::Equal),
            ("v1", "v1", Ordering::Equal),
            ("v1alpha", "v2alpha", Ordering::Less),
            ("v2alpha", "v1alpha", Ordering::Greater),
            ("v1beta", "v2beta", Ordering::Less),
            ("v2beta", "v1beta", Ordering::Greater),
            ("v1", "v2", Ordering::Less),
            ("v2", "v1", Ordering::Greater),
            ("v1alpha", "v1alpha1", Ordering::Less),
            ("v1alpha1", "v1alpha", Ordering::Greater),
            ("v1beta", "v1beta1", Ordering::Less),
            ("v1beta1", "v1beta", Ordering::Greater),
            ("v2alpha", "v1beta", Ordering::Greater),
            ("v1beta", "v2alpha", Ordering::Less),
            ("v2alpha", "v1", Ordering::Greater),
            ("v1", "v2alpha", Ordering::Less),
        ];
        for (lh, rh, cmp) in test_cases {
            assert_eq!(lh.cmp(rh), cmp);
        }
    }
}
