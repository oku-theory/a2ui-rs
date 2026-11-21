//! What a host chooses for the basic functions: the locale and how URLs open.

use std::fmt::Debug;
use std::sync::Arc;

use crate::locale::{EnUs, Locale};

/// Opens a URL for `openUrl`, once its scheme has been checked.
pub trait UrlOpener: Send + Sync + Debug {
    /// Opens `url`, whose scheme is `http`, `https`, `mailto` or `tel`.
    ///
    /// The basic catalog expects no result: a failure is the host's to report.
    fn open(&self, url: &str);
}

/// A [`UrlOpener`] that opens nothing, for hosts without a browser.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct NoOpener;

impl UrlOpener for NoOpener {
    fn open(&self, _url: &str) {}
}

/// The host's choices for [`functions`](crate::functions): locale and URL opener.
///
/// The default is [`EnUs`] and [`NoOpener`].
#[derive(Debug, Clone)]
pub struct BasicOptions {
    /// The locale the formatting functions follow.
    pub locale: Arc<dyn Locale>,
    /// What `openUrl` hands an allowed URL to.
    pub url_opener: Arc<dyn UrlOpener>,
}

impl Default for BasicOptions {
    fn default() -> Self {
        Self {
            locale: Arc::new(EnUs),
            url_opener: Arc::new(NoOpener),
        }
    }
}
