use super::*;
use crate::{logging, utils::sleep_for_ms};
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::{
    api::{Api, DeleteParams, ListParams, Patch, PatchParams},
    core::{object::HasSpec, CustomResourceExt, NamespaceResourceScope, Resource},
    runtime::{
        wait::{await_condition, conditions},
        watcher, WatchStreamExt,
    },
    Client,
};
use std::fmt;
use std::marker::PhantomData;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// should be consistent with the `group` attribute on Sentinel Rule CRDs
pub const SENTINEL_RULE_GROUP: &str = "rust.datasource.sentinel.io";
pub const SENTINEL_RULE_VERSION: &str = "v1alpha1";

pub struct K8sDataSource<
    P: SentinelRule + PartialEq + DeserializeOwned + Clone,
    H: PropertyHandler<P>,
    R: CustomResourceExt
        + Resource<Scope = NamespaceResourceScope>
        + HasSpec<Spec = P>
        + Clone
        + DeserializeOwned
        + fmt::Debug
        + Send
        + 'static,
> where
    <R as Resource>::DynamicType: Default,
{
    ds: DataSourceBase<P, H>,
    /// Used as the field manager name
    manager: String,
    /// The k8s namespace in cluster
    namespace: String,
    /// Construct CRD name with `SENTINEL_RULE_GROUP` as suffix
    cr_name: String,
    client: Client,
    closed: AtomicBool,
    phantom: PhantomData<R>,
}

impl<
        P: SentinelRule + PartialEq + DeserializeOwned + Clone,
        H: PropertyHandler<P>,
        R: CustomResourceExt
            + Resource<Scope = NamespaceResourceScope>
            + HasSpec<Spec = P>
            + Clone
            + DeserializeOwned
            + fmt::Debug
            + Send
            + 'static,
    > K8sDataSource<P, H, R>
where
    <R as Resource>::DynamicType: Default,
{
    pub fn new(
        client: Client,
        property: String,
        namespace: String,
        cr_name: String,
        handlers: Vec<Arc<H>>,
    ) -> Self {
        let mut ds = DataSourceBase::default();
        for h in handlers {
            // incase of duplication, add it one by one, instead of adding all at once
            ds.add_property_handler(h);
        }
        K8sDataSource {
            ds,
            manager: property,
            namespace,
            cr_name: format!("{}.{}", cr_name, SENTINEL_RULE_GROUP),
            client,
            closed: AtomicBool::new(false),
            phantom: PhantomData,
        }
    }

    /// initialize the datasource and load initial rules
    /// start listener to listen on dynamic source
    /// return error if initialize failed.
    pub async fn initialize(&mut self) -> Result<()> {
        let crds: Api<CustomResourceDefinition> = Api::all(self.client.clone());
        // Apply the CRD so users can create Sentinel Rule instances in Kubernetes
        crds.patch(
            &self.cr_name,
            &PatchParams::apply(&self.manager),
            &Patch::Apply(R::crd()),
        )
        .await?;

        // Wait for the CRD to be ready
        await_condition(crds, &self.cr_name, conditions::is_crd_established()).await?;
        self.watch().await
    }

    /// Close the data source, stop watch the property key.
    pub async fn close(&self) -> Result<()> {
        self.closed.store(true, Ordering::SeqCst);
        let crds: Api<CustomResourceDefinition> = Api::all(self.client.clone());
        crds.delete(&self.cr_name, &DeleteParams::default()).await?;
        logging::info!(
            "[k8s] k8s data source has been closed. Remove the custom resource {:?}.",
            self.cr_name
        );
        Ok(())
    }

    /// Add watch for property from last_updated_revision updated after initializing
    async fn watch(&mut self) -> Result<()> {
        logging::info!(
            "[k8s] k8s data source is watching cr {} with manager {} in namespace {}",
            self.cr_name,
            self.manager,
            self.namespace
        );
        loop {
            // Watch for changes to foos in the configured namespace
            let rules: Api<R> = Api::namespaced(self.client.clone(), &self.namespace);
            let lp = ListParams::default();
            let mut apply_stream = watcher(rules, lp).applied_objects().boxed();
            while let Some(rule) = apply_stream.try_next().await? {
                self.ds.load(vec![Arc::new(rule.spec().clone())]).unwrap();
            }
            if self.closed.load(Ordering::SeqCst) {
                return Ok(());
            }
            sleep_for_ms(1000);
        }
    }
}

impl<
        P: SentinelRule + PartialEq + DeserializeOwned + Clone,
        H: PropertyHandler<P>,
        R: CustomResourceExt
            + Resource<Scope = NamespaceResourceScope>
            + HasSpec<Spec = P>
            + Clone
            + DeserializeOwned
            + fmt::Debug
            + Send
            + 'static,
    > DataSource<P, H> for K8sDataSource<P, H, R>
where
    <R as Resource>::DynamicType: Default,
{
    fn get_base(&mut self) -> &mut DataSourceBase<P, H> {
        &mut self.ds
    }
}
