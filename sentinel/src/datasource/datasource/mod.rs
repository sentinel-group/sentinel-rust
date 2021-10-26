/// The dynamic rule replacement.

#[cfg(feature = "ds_etcdv3")]
pub mod etcdv3;
#[cfg(feature = "ds_etcdv3")]
pub use etcdv3::*;

use super::*;
use async_trait::async_trait;

/// The generic interface to describe the datasource
/// Each DataSource instance listen to one property (sentinel rule).
#[async_trait]
pub trait DataSource<P: SentinelRule + PartialEq + DeserializeOwned, H: PropertyHandler<P>>:
    Send
{
    fn get_base(&mut self) -> &mut DataSourceBase<P, H>;
    /// Add specified property handler in current datasource
    fn add_property_handler(&mut self, h: Arc<H>) {
        self.get_base().add_property_handler(h);
    }
    /// Remove specified property handler in current datasource
    fn remove_property_handler(&mut self, h: Arc<H>) {
        self.get_base().remove_property_handler(h);
    }
    /// Read original data from the data source.
    /// return source bytes if succeed to read, if not, return error when reading
    async fn read_source(&mut self) -> Result<String>;
    /// initialize the datasource and load initial rules
    /// start listener to listen on dynamic source
    /// return error if initialize failed;
    /// once initialized, listener should recover all panic and error.
    async fn initialize(&mut self) -> Result<()>;
    /// Close the data source.
    async fn close(&self) -> Result<()>;
}

pub struct DataSourceBase<P: SentinelRule + PartialEq + DeserializeOwned, H: PropertyHandler<P>> {
    handlers: Vec<Arc<H>>,
    phantom: PhantomData<P>,
}

impl<P, H> Default for DataSourceBase<P, H>
where
    P: SentinelRule + PartialEq + DeserializeOwned,
    H: PropertyHandler<P>,
{
    fn default() -> Self {
        Self {
            handlers: Vec::new(),
            phantom: PhantomData,
        }
    }
}

impl<P, H> DataSourceBase<P, H>
where
    P: SentinelRule + PartialEq + DeserializeOwned,
    H: PropertyHandler<P>,
{
    pub fn update(&mut self, src: Option<&String>) -> Result<()> {
        let mut err = String::new();
        for h in &mut self.handlers {
            let h = Arc::get_mut(h).unwrap();
            let e = h.handle(src.clone());
            if let Err(e) = e {
                err.push_str(&format!("{:?}", e));
            }
        }
        if err.is_empty() {
            Ok(())
        } else {
            Err(Error::msg(err))
        }
    }

    // return idx if existed, else return None
    pub fn index_of_handler(&self, h: Arc<H>) -> Option<usize> {
        for (idx, handler) in self.handlers.iter().enumerate() {
            if Arc::ptr_eq(&handler, &h) {
                return Some(idx);
            }
        }
        return None;
    }

    pub fn add_property_handler(&mut self, h: Arc<H>) {
        if self.index_of_handler(Arc::clone(&h)).is_some() {
            return;
        }
        self.handlers.push(Arc::clone(&h));
    }

    pub fn remove_property_handler(&mut self, h: Arc<H>) {
        if let Some(idx) = self.index_of_handler(Arc::clone(&h)) {
            self.handlers.swap_remove(idx);
        }
    }
}
