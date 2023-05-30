/// The dynamic rule replacement.

#[cfg(feature = "ds_etcdv3")]
pub mod ds_etcdv3;
#[cfg(feature = "ds_etcdv3")]
pub use ds_etcdv3::*;

#[cfg(feature = "ds_consul")]
pub mod ds_consul;
#[cfg(feature = "ds_consul")]
pub use ds_consul::*;
cfg_k8s! {
    pub mod ds_k8s;
    pub use ds_k8s::*;
}

use super::*;

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
            let e = h.handle(src);
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
            if Arc::ptr_eq(handler, &h) {
                return Some(idx);
            }
        }
        None
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

    pub fn load(&mut self, rules: Vec<Arc<P>>) -> Result<bool> {
        let mut res = true;
        for h in &mut self.handlers {
            let h = Arc::get_mut(h).unwrap();
            res = res && h.load(rules.clone())?;
        }
        Ok(res)
    }
}
